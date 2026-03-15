<script lang="ts">
	import { onMount } from 'svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';
	import { getAgenda, readFile, saveFile, listNodes } from '$lib/tauri/commands';
	import { vault } from '$lib/stores/vault.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { NodeRecord } from '$lib/types/node';

	let items = $state<NodeRecord[]>([]);
	let error = $state<string | null>(null);
	let tab = $state<'agenda' | 'tasks'>('agenda');
	let taskSearch = $state('');
	let taskFilter = $state<string>('all');
	let changingId = $state<string | null>(null);

	onMount(async () => {
		try { items = await getAgenda(); }
		catch (e) { error = String(e); }
	});

	// ── Helpers ──────────────────────────────────────────────────

	function fmtDate(d: Date): string {
		return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
	}

	function extractDate(raw: string | null): string {
		if (!raw) return '';
		const m = raw.match(/(\d{4}-\d{2}-\d{2})/);
		return m ? m[1] : '';
	}

	const today = fmtDate(new Date());

	function isOverdue(n: NodeRecord): boolean {
		const dl = extractDate(n.deadline);
		return !!dl && dl < today && !orgConfig.doneKeywords.includes(n.todo ?? '');
	}

	function isDone(n: NodeRecord): boolean {
		return orgConfig.doneKeywords.includes(n.todo ?? '');
	}

	// ── Weekly agenda ───────────────────────────────────────────

	function weekDays(): { date: string; label: string; isToday: boolean }[] {
		const out: { date: string; label: string; isToday: boolean }[] = [];
		const now = new Date();
		for (let i = 0; i < 7; i++) {
			const d = new Date(now); d.setDate(d.getDate() + i);
			const ds = fmtDate(d);
			const dayNames = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
			const label = i === 0 ? 'Today' : i === 1 ? 'Tomorrow' : `${dayNames[d.getDay()]} ${d.getMonth()+1}/${d.getDate()}`;
			out.push({ date: ds, label, isToday: i === 0 });
		}
		return out;
	}

	function itemsForDate(date: string): NodeRecord[] {
		return items.filter(n => {
			if (isDone(n)) return false;
			return extractDate(n.deadline) === date || extractDate(n.scheduled) === date;
		});
	}

	const overdueItems = $derived(items.filter(n => isOverdue(n)));

	// ── Tasks tab ───────────────────────────────────────────────

	const filteredTasks = $derived(
		items.filter(n => {
			if (taskFilter !== 'all' && n.todo !== taskFilter) return false;
			if (taskSearch.trim()) return n.title?.toLowerCase().includes(taskSearch.toLowerCase()) ?? false;
			return true;
		})
	);

	// ── Inline state change ─────────────────────────────────────

	async function setState(node: NodeRecord, state: string | null) {
		await modifyHeadline(node, (stars, kw, rest) => {
			return state ? `${stars}${state} ${rest}` : `${stars}${rest}`;
		});
	}

	async function setPriority(node: NodeRecord, priority: string | null) {
		await modifyHeadline(node, (stars, kw, rest) => {
			const prefix = kw ? `${stars}${kw} ` : stars;
			const stripped = rest.replace(/^\[#[A-Z]\]\s*/, '');
			return priority ? `${prefix}[#${priority}] ${stripped}` : `${prefix}${stripped}`;
		});
	}

	async function setDate(node: NodeRecord, type: 'DEADLINE' | 'SCHEDULED', date: string | null) {
		changingId = node.id;
		try {
			const content = await readFile(node.file);
			const lines = content.split('\n');

			// Find the headline for this node
			let hlIdx = -1;
			for (let i = 0; i < lines.length; i++) {
				if (!/^\*+\s/.test(lines[i])) continue;
				const nearby = lines.slice(i, Math.min(i + 8, lines.length)).join('\n');
				if (nearby.includes(`:ID: ${node.id}`)) { hlIdx = i; break; }
			}
			if (hlIdx === -1) return;

			// Find existing planning line or insertion point
			let planIdx = -1;
			let insertAfter = hlIdx;
			for (let j = hlIdx + 1; j < lines.length && j < hlIdx + 6; j++) {
				const t = lines[j].trim();
				if (t.startsWith(`${type}:`) || t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:')) {
					if (t.startsWith(`${type}:`)) planIdx = j;
					insertAfter = j;
				} else if (t === ':PROPERTIES:') {
					while (j < lines.length && lines[j].trim() !== ':END:') j++;
					insertAfter = j;
				} else break;
			}

			if (date) {
				const dayNames = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
				const d = new Date(date + 'T12:00:00');
				const ts = `<${date} ${dayNames[d.getDay()]}>`;
				const newLine = `${type}: ${ts}`;

				if (planIdx >= 0) {
					// Replace existing
					lines[planIdx] = lines[planIdx].replace(new RegExp(`${type}:\\s*<[^>]*>`), `${type}: ${ts}`);
				} else {
					lines.splice(insertAfter + 1, 0, newLine);
				}
			} else if (planIdx >= 0) {
				// Remove the planning keyword from the line
				lines[planIdx] = lines[planIdx].replace(new RegExp(`\\s*${type}:\\s*<[^>]*>`), '').trim();
				if (!lines[planIdx]) lines.splice(planIdx, 1);
			}

			await saveFile(node.file, lines.join('\n'));
			items = await getAgenda();
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) { error = String(e); }
		finally { changingId = null; }
	}

	async function modifyHeadline(node: NodeRecord, fn: (stars: string, kw: string | null, rest: string) => string) {
		changingId = node.id;
		try {
			const content = await readFile(node.file);
			const lines = content.split('\n');
			const kwPattern = orgConfig.allKeywords.join('|');

			for (let i = 0; i < lines.length; i++) {
				const m = lines[i].match(/^(\*+\s+)/);
				if (!m) continue;
				// Match by ID in nearby property drawer
				const nearby = lines.slice(i, Math.min(i + 8, lines.length)).join('\n');
				if (!nearby.includes(`:ID: ${node.id}`)) continue;

				const stars = m[1];
				let rest = lines[i].slice(stars.length);
				let kw: string | null = null;
				const kwMatch = rest.match(new RegExp(`^(${kwPattern})\\s+`));
				if (kwMatch) { kw = kwMatch[1]; rest = rest.slice(kwMatch[0].length); }
				lines[i] = fn(stars, kw, rest);
				break;
			}

			await saveFile(node.file, lines.join('\n'));
			items = await getAgenda();
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) { error = String(e); }
		finally { changingId = null; }
	}
</script>

<div class="flex h-full flex-col">
	<header class="flex h-14 shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: var(--safe-area-top)">
		<button onclick={() => navigation.navigateToVault()} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>
		<h1 class="flex-1 text-lg font-semibold">Agenda</h1>
		<span class="text-xs text-surface-700 dark:text-surface-300">{items.length} items</span>
	</header>

	<!-- Tabs -->
	<div class="flex shrink-0 border-b border-surface-200 dark:border-surface-700">
		<button onclick={() => (tab = 'agenda')} class="flex-1 py-2.5 text-center text-sm font-medium transition-colors {tab === 'agenda' ? 'border-b-2 border-mycelium-600 text-mycelium-700 dark:text-mycelium-300' : 'text-surface-700 dark:text-surface-300'}">Week</button>
		<button onclick={() => (tab = 'tasks')} class="flex-1 py-2.5 text-center text-sm font-medium transition-colors {tab === 'tasks' ? 'border-b-2 border-mycelium-600 text-mycelium-700 dark:text-mycelium-300' : 'text-surface-700 dark:text-surface-300'}">All Tasks</button>
	</div>

	{#if error}<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>{/if}

	<div class="flex-1 overflow-y-auto">
		{#if tab === 'agenda'}
			<!-- Weekly agenda -->
			<div>
				{#if overdueItems.length > 0}
					<div class="border-b border-red-200 px-4 py-2 dark:border-red-900" style="background:#fef2f2">
						<h3 class="mb-1.5 text-[11px] font-bold uppercase tracking-wide" style="color:#dc2626">Overdue ({overdueItems.length})</h3>
						{#each overdueItems as item}
							{@render taskRow(item)}
						{/each}
					</div>
				{/if}

				{#each weekDays() as day}
					{@const dayItems = itemsForDate(day.date)}
					<div class="border-b border-surface-100 px-4 py-2 dark:border-surface-800">
						<h3 class="mb-1 text-[11px] font-bold uppercase tracking-wide {day.isToday ? 'text-mycelium-700 dark:text-mycelium-400' : 'text-surface-700 dark:text-surface-300'}">
							{day.label} <span class="font-normal opacity-60">{day.date}</span>
						</h3>
						{#if dayItems.length > 0}
							{#each dayItems as item}
								{@render taskRow(item)}
							{/each}
						{:else}
							<p class="py-0.5 text-[11px] text-surface-700/40 dark:text-surface-300/40">—</p>
						{/if}
					</div>
				{/each}
			</div>

		{:else}
			<!-- All tasks with search/filter -->
			<div class="border-b border-surface-200 px-4 py-2 dark:border-surface-700">
				<div class="flex gap-2">
					<div class="relative flex-1">
						<svg class="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" /></svg>
						<input type="text" bind:value={taskSearch} placeholder="Filter tasks..." class="w-full rounded-lg border border-surface-200 bg-surface-50 py-2 pl-8 pr-3 text-sm dark:border-surface-700 dark:bg-surface-900" />
					</div>
					<select bind:value={taskFilter} class="rounded-lg border border-surface-200 bg-surface-50 px-2 py-2 text-xs dark:border-surface-700 dark:bg-surface-900">
						<option value="all">All</option>
						{#each orgConfig.todoKeywords as kw}<option value={kw}>{kw}</option>{/each}
						{#each orgConfig.doneKeywords as kw}<option value={kw}>{kw}</option>{/each}
					</select>
				</div>
			</div>

			<div class="divide-y divide-surface-100 dark:divide-surface-800">
				{#each filteredTasks as item}
					<div class="px-4 py-1">
						{@render taskRow(item)}
					</div>
				{/each}
			</div>
			{#if filteredTasks.length === 0}
				<p class="p-8 text-center text-sm text-surface-700 dark:text-surface-300">No matching tasks</p>
			{/if}
		{/if}
	</div>

	<MobileNav />
</div>

{#snippet taskRow(item: NodeRecord)}
	<div class="py-2 {changingId === item.id ? 'opacity-50' : ''}">
		<!-- Row 1: state, title, priority -->
		<div class="flex items-center gap-2">
			<select
				value={item.todo ?? ''}
				onchange={(e) => setState(item, (e.target as HTMLSelectElement).value || null)}
				disabled={changingId === item.id}
				class="h-7 shrink-0 rounded border-0 py-0 pl-1 pr-5 text-[10px] font-bold"
				style="color:{isDone(item) ? '#16a34a' : item.todo ? '#dc2626' : '#6b7280'};background:{isDone(item) ? '#f0fdf4' : item.todo ? '#fef2f2' : 'transparent'}"
			>
				<option value="">None</option>
				{#each orgConfig.todoKeywords as kw}<option value={kw}>{kw}</option>{/each}
				{#each orgConfig.doneKeywords as kw}<option value={kw}>{kw}</option>{/each}
			</select>

			<button onclick={() => navigation.navigateToNode(item.id)} class="min-w-0 flex-1 text-left">
				<div class="truncate text-sm {isDone(item) ? 'line-through opacity-60' : 'font-medium'}">{item.title ?? 'Untitled'}</div>
			</button>

			<select
				value={item.priority ?? ''}
				onchange={(e) => setPriority(item, (e.target as HTMLSelectElement).value || null)}
				disabled={changingId === item.id}
				class="h-7 shrink-0 rounded border-0 py-0 pl-1 pr-4 text-[10px] font-bold"
				style="color:#ea580c;{item.priority ? 'background:#fff7ed' : 'background:transparent'}"
			>
				<option value="">—</option>
				{#each orgConfig.priorities as p}<option value={p}>#{p}</option>{/each}
			</select>
		</div>

		<!-- Row 2: compact date editors -->
		<div class="mt-1 flex items-center gap-3 pl-1 text-[11px]">
			<label class="flex items-center gap-1 cursor-pointer">
				<span class="font-semibold" style="color:#dc2626">DL</span>
				<input
					type="date"
					value={extractDate(item.deadline)}
					onchange={(e) => setDate(item, 'DEADLINE', (e.target as HTMLInputElement).value || null)}
					class="rounded border px-1.5 py-0.5 text-[11px]"
					style="border-color:#e2e8f0;color:{isOverdue(item) ? '#dc2626' : '#374151'};background:transparent;max-width:130px"
				/>
			</label>
			<label class="flex items-center gap-1 cursor-pointer">
				<span class="font-semibold" style="color:#2563eb">SC</span>
				<input
					type="date"
					value={extractDate(item.scheduled)}
					onchange={(e) => setDate(item, 'SCHEDULED', (e.target as HTMLInputElement).value || null)}
					class="rounded border px-1.5 py-0.5 text-[11px]"
					style="border-color:#e2e8f0;color:#374151;background:transparent;max-width:130px"
				/>
			</label>
		</div>
	</div>
{/snippet}
