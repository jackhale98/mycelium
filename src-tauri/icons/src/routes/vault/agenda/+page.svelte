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

	function extractTime(raw: string | null): string {
		if (!raw) return '';
		const m = raw.match(/(\d{1,2}:\d{2})/);
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

	function itemsForDate(date: string): { node: NodeRecord; reason: 'deadline' | 'scheduled' }[] {
		const result: { node: NodeRecord; reason: 'deadline' | 'scheduled'; time: string }[] = [];
		for (const n of items) {
			if (isDone(n)) continue;
			if (extractDate(n.deadline) === date) result.push({ node: n, reason: 'deadline', time: extractTime(n.deadline) });
			else if (extractDate(n.scheduled) === date) result.push({ node: n, reason: 'scheduled', time: extractTime(n.scheduled) });
		}
		// Sort by time (items with time first, then by time string, then by priority)
		result.sort((a, b) => {
			if (a.time && !b.time) return -1;
			if (!a.time && b.time) return 1;
			if (a.time && b.time) return a.time.localeCompare(b.time);
			return (a.node.priority ?? 'Z').localeCompare(b.node.priority ?? 'Z');
		});
		return result;
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

	/** Set or update a DEADLINE/SCHEDULED date+time. Preserves any repeater syntax. */
	async function setDate(node: NodeRecord, type: 'DEADLINE' | 'SCHEDULED', datetime: string | null) {
		changingId = node.id;
		try {
			const content = await readFile(node.file);
			const lines = content.split('\n');

			let hlIdx = -1;
			for (let i = 0; i < lines.length; i++) {
				if (!/^\*+\s/.test(lines[i])) continue;
				const nearby = lines.slice(i, Math.min(i + 8, lines.length)).join('\n');
				if (nearby.includes(`:ID: ${node.id}`)) { hlIdx = i; break; }
			}
			if (hlIdx === -1) return;

			let planIdx = -1;
			let insertAfter = hlIdx;
			let existingRepeater = '';
			for (let j = hlIdx + 1; j < lines.length && j < hlIdx + 6; j++) {
				const t = lines[j].trim();
				if (t.startsWith(`${type}:`) || t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:')) {
					if (t.startsWith(`${type}:`)) {
						planIdx = j;
						// Extract existing repeater and warning to preserve them
						const repMatch = t.match(/(\+\+?|\.?\+)\d+[hdwmy]/);
						if (repMatch) existingRepeater = ' ' + repMatch[0];
						const warnMatch = t.match(/-\d+[hdwmy]/);
						if (warnMatch) existingRepeater += ' ' + warnMatch[0];
					}
					insertAfter = j;
				} else if (t === ':PROPERTIES:') {
					while (j < lines.length && lines[j].trim() !== ':END:') j++;
					insertAfter = j;
				} else break;
			}

			if (datetime) {
				// datetime can be "2026-03-20" or "2026-03-20T14:00"
				const [datePart, timePart] = datetime.includes('T') ? datetime.split('T') : [datetime, ''];
				const dayNames = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
				const d = new Date(datePart + 'T12:00:00');
				let ts = `<${datePart} ${dayNames[d.getDay()]}`;
				if (timePart) ts += ` ${timePart}`;
				ts += `${existingRepeater}>`;
				const newLine = `${type}: ${ts}`;

				if (planIdx >= 0) {
					lines[planIdx] = lines[planIdx].replace(new RegExp(`${type}:\\s*<[^>]*>`), `${type}: ${ts}`);
				} else {
					lines.splice(insertAfter + 1, 0, newLine);
				}
			} else if (planIdx >= 0) {
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
							{#each dayItems as di}
								<div class="flex items-center gap-1.5">
									<span style="font-size:9px;font-weight:700;padding:1px 4px;border-radius:3px;{di.reason === 'deadline' ? 'color:#dc2626;background:#fef2f2' : 'color:#2563eb;background:#eff6ff'}">{di.reason === 'deadline' ? 'DL' : 'SC'}</span>
									{#if di.time}<span style="font-size:10px;color:#6b7280;font-variant-numeric:tabular-nums">{di.time}</span>{/if}
									<div class="flex-1">{@render taskRow(di.node)}</div>
								</div>
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
	{@const dlDate = extractDate(item.deadline)}
	{@const dlTime = extractTime(item.deadline)}
	{@const scDate = extractDate(item.scheduled)}
	{@const scTime = extractTime(item.scheduled)}
	<div
		style="position:relative;overflow:hidden;border-radius:8px"
		ontouchstart={(e) => {
			const el = (e.currentTarget as HTMLElement);
			const inner = el.querySelector('[data-inner]') as HTMLElement;
			const actions = el.querySelector('[data-actions]') as HTMLElement;
			if (!inner || !actions) return;
			const startX = e.touches[0].clientX;
			let dx = 0;
			const onMove = (ev: TouchEvent) => {
				dx = startX - ev.touches[0].clientX;
				const clamped = Math.max(-160, Math.min(0, -dx));
				inner.style.transform = `translateX(${clamped}px)`;
				actions.style.opacity = String(Math.min(1, Math.abs(clamped) / 80));
			};
			const onEnd = () => {
				document.removeEventListener('touchmove', onMove);
				document.removeEventListener('touchend', onEnd);
				const open = dx > 60;
				inner.style.transition = 'transform 0.25s cubic-bezier(0.25,0.46,0.45,0.94)';
				actions.style.transition = 'opacity 0.25s ease';
				inner.style.transform = open ? 'translateX(-160px)' : 'translateX(0)';
				actions.style.opacity = open ? '1' : '0';
				setTimeout(() => { inner.style.transition = ''; actions.style.transition = ''; }, 250);
			};
			document.addEventListener('touchmove', onMove, { passive: true });
			document.addEventListener('touchend', onEnd);
		}}
	>
		<!-- Action buttons (fade in as user swipes) -->
		<div data-actions style="position:absolute;right:0;top:0;bottom:0;display:flex;opacity:0">
			<label style="width:80px;display:flex;flex-direction:column;align-items:center;justify-content:center;background:#dc2626;color:white;font-size:12px;font-weight:600;cursor:pointer;gap:2px">
				Deadline
				<input type="datetime-local" value={dlDate && dlTime ? `${dlDate}T${dlTime}` : dlDate} onchange={(e) => setDate(item, 'DEADLINE', (e.target as HTMLInputElement).value || null)} style="position:absolute;opacity:0;width:0;height:0" />
				<span style="font-size:10px;opacity:0.8">{dlDate ? (dlTime ? `${dlDate} ${dlTime}` : dlDate) : 'set'}</span>
			</label>
			<label style="width:80px;display:flex;flex-direction:column;align-items:center;justify-content:center;background:#2563eb;color:white;font-size:12px;font-weight:600;cursor:pointer;gap:2px">
				Schedule
				<input type="datetime-local" value={scDate && scTime ? `${scDate}T${scTime}` : scDate} onchange={(e) => setDate(item, 'SCHEDULED', (e.target as HTMLInputElement).value || null)} style="position:absolute;opacity:0;width:0;height:0" />
				<span style="font-size:10px;opacity:0.8">{scDate ? (scTime ? `${scDate} ${scTime}` : scDate) : 'set'}</span>
			</label>
		</div>

		<!-- Main row -->
		<div data-inner class="bg-surface-0 dark:bg-surface-950" style="position:relative;display:flex;align-items:center;gap:8px;padding:8px 4px;will-change:transform;{changingId === item.id ? 'opacity:0.5;' : ''}">
			<select
				value={item.todo ?? ''}
				onchange={(e) => setState(item, (e.target as HTMLSelectElement).value || null)}
				disabled={changingId === item.id}
				style="height:28px;flex-shrink:0;border-radius:4px;border:0;padding:0 16px 0 4px;font-size:10px;font-weight:700;color:{isDone(item) ? '#16a34a' : item.todo ? '#dc2626' : '#6b7280'};background:{isDone(item) ? '#f0fdf4' : item.todo ? '#fef2f2' : 'transparent'}"
			>
				<option value="">None</option>
				{#each orgConfig.todoKeywords as kw}<option value={kw}>{kw}</option>{/each}
				{#each orgConfig.doneKeywords as kw}<option value={kw}>{kw}</option>{/each}
			</select>

			<button onclick={() => navigation.navigateToNode(item.id)} style="min-width:0;flex:1;text-align:left">
				<div style="font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;{isDone(item) ? 'text-decoration:line-through;opacity:0.6' : 'font-weight:500'}">{item.title ?? 'Untitled'}</div>
				{#if dlDate || scDate}
					<div style="display:flex;gap:6px;font-size:10px;margin-top:2px;flex-wrap:wrap">
						{#if dlDate}
							<span style="display:inline-flex;align-items:center;gap:2px;padding:1px 4px;border-radius:3px;background:{isOverdue(item) ? '#fef2f2' : '#fff7ed'};color:{isOverdue(item) ? '#dc2626' : '#ea580c'}">
								<span style="font-weight:700">DL</span> {dlDate}{#if dlTime} {dlTime}{/if}
							</span>
						{/if}
						{#if scDate}
							<span style="display:inline-flex;align-items:center;gap:2px;padding:1px 4px;border-radius:3px;background:#eff6ff;color:#2563eb">
								<span style="font-weight:700">SC</span> {scDate}{#if scTime} {scTime}{/if}
							</span>
						{/if}
					</div>
				{/if}
			</button>

			<select
				value={item.priority ?? ''}
				onchange={(e) => setPriority(item, (e.target as HTMLSelectElement).value || null)}
				disabled={changingId === item.id}
				style="height:28px;flex-shrink:0;border-radius:4px;border:0;padding:0 12px 0 4px;font-size:10px;font-weight:700;color:#ea580c;{item.priority ? 'background:#fff7ed' : 'background:transparent'}"
			>
				<option value="">—</option>
				{#each orgConfig.priorities as p}<option value={p}>#{p}</option>{/each}
			</select>
		</div>
	</div>
{/snippet}
