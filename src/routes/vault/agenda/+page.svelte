<script lang="ts">
	import { onMount } from 'svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';
	import { getAgenda, readFileMeta, saveFile, listNodes, localDate, localTime } from '$lib/tauri/commands';
	import { onDbUpdated } from '$lib/tauri/events';
	import { vault } from '$lib/stores/vault.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { HeadlineRecord } from '$lib/types/node';
	import {
		agendaReason, applyRepeaterOnDone, compareTasks, getTodoKeyword, isDoneKeyword,
		isHeadlineLine, overdueItems as findOverdue, repeatKeyword, setClosed,
		setPlanningDate, setPriority as setHeadlinePriority, setTodoKeyword, removePlanning,
		timestampDate,
	} from '$lib/org';
	import type { KeywordConfig, PlanningKind } from '$lib/org';

	let items = $state<HeadlineRecord[]>([]);
	let error = $state<string | null>(null);
	let tab = $state<'agenda' | 'tasks'>('agenda');
	let taskSearch = $state('');
	let taskFilter = $state<string>('all');
	let changingId = $state<string | null>(null);
	// Recomputed on load, on refresh and when the day rolls over, so a device left
	// open overnight does not keep calling yesterday "Today".
	let today = $state(localDate());

	async function refresh() {
		try {
			items = await getAgenda();
			today = localDate();
			error = null;
		} catch (e) { error = String(e); }
	}

	onMount(() => {
		refresh();
		// Keep in step with external edits and the file watcher.
		const unlisten = onDbUpdated(() => { refresh(); });
		const onVisible = () => { if (document.visibilityState === 'visible') refresh(); };
		document.addEventListener('visibilitychange', onVisible);
		const dayTimer = setInterval(() => { today = localDate(); }, 60_000);
		return () => {
			document.removeEventListener('visibilitychange', onVisible);
			clearInterval(dayTimer);
			void Promise.resolve(unlisten).then((off) => { if (typeof off === 'function') off(); });
		};
	});

	// ── Helpers ──────────────────────────────────────────────────

	function keywordConfig(): KeywordConfig {
		return { todoKeywords: orgConfig.todoKeywords, doneKeywords: orgConfig.doneKeywords };
	}

	function priorityConfig() {
		return { priorities: orgConfig.priorities };
	}

	function nowForOrg() {
		return { date: localDate(), time: localTime() };
	}

	function extractDate(raw: string | null): string {
		return timestampDate(raw) ?? '';
	}

	function extractTime(raw: string | null): string {
		const m = raw?.match(/(\d{1,2}:\d{2})/);
		return m ? m[1] : '';
	}

	function dateTimeValue(raw: string | null): string {
		const date = extractDate(raw);
		if (!date) return '';
		const time = extractTime(raw);
		return time ? `${date}T${time}` : date;
	}

	/** Navigate to the node for an agenda item */
	function navigateToItem(item: HeadlineRecord) {
		if (item.node_id) {
			navigation.navigateToNode(item.node_id);
			return;
		}
		const fileNode = vault.nodes.find(n => n.file === item.file);
		if (fileNode) navigation.navigateToNode(fileNode.id);
	}

	function isPastDue(n: HeadlineRecord): boolean {
		const dl = extractDate(n.deadline);
		return !!dl && dl < today && !isDone(n);
	}

	function isDone(n: HeadlineRecord): boolean {
		return isDoneKeyword(n.todo ?? null, keywordConfig());
	}

	// ── Weekly agenda ───────────────────────────────────────────

	function weekDays(from: string): { date: string; label: string; isToday: boolean }[] {
		const out: { date: string; label: string; isToday: boolean }[] = [];
		const dayNames = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
		const base = new Date(`${from}T12:00:00`);
		for (let i = 0; i < 7; i++) {
			const d = new Date(base); d.setDate(d.getDate() + i);
			const ds = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
			const label = i === 0 ? 'Today' : i === 1 ? 'Tomorrow' : `${dayNames[d.getDay()]} ${d.getMonth()+1}/${d.getDate()}`;
			out.push({ date: ds, label, isToday: i === 0 });
		}
		return out;
	}

	interface DayEntry { node: HeadlineRecord; reason: string; time: string; label: string | null }

	function itemsForDate(date: string): DayEntry[] {
		const done = orgConfig.doneKeywords;
		const result: DayEntry[] = [];
		for (const n of items) {
			const reason = agendaReason(n, date, today, done);
			if (!reason) continue;
			const source = reason.includes('deadline') ? n.deadline : n.scheduled;
			result.push({
				node: n,
				reason,
				time: extractTime(source),
				label: reason === 'overdue-scheduled'
					? `Sched. ${daysBetween(extractDate(n.scheduled), date)}x`
					: reason === 'upcoming-deadline'
						? `In ${daysBetween(date, extractDate(n.deadline))} d.`
						: null,
			});
		}
		result.sort((a, b) => {
			if (a.time && !b.time) return -1;
			if (!a.time && b.time) return 1;
			if (a.time && b.time) return a.time.localeCompare(b.time);
			return compareTasks(a.node, b.node, priorityConfig());
		});
		return result;
	}

	function daysBetween(from: string, to: string): number {
		if (!from || !to) return 0;
		const ms = new Date(`${to}T12:00:00`).getTime() - new Date(`${from}T12:00:00`).getTime();
		return Math.max(0, Math.round(ms / 86_400_000));
	}

	const overdueItems = $derived(findOverdue(items, today, orgConfig.doneKeywords));

	// ── Tasks tab ───────────────────────────────────────────────

	const filteredTasks = $derived(
		items.filter(n => {
			if (taskFilter !== 'all' && n.todo !== taskFilter) return false;
			if (taskSearch.trim()) return n.title?.toLowerCase().includes(taskSearch.toLowerCase()) ?? false;
			return true;
		}).sort((a, b) => compareTasks(a, b, priorityConfig()))
	);

	// ── Inline editing ──────────────────────────────────────────

	/**
	 * Rewrite one headline in its file. The headline is located by line number and
	 * verified to still be a headline, so a stale index can never rewrite prose.
	 */
	async function editHeadline(
		node: HeadlineRecord,
		fn: (lines: string[], idx: number) => string[]
	) {
		changingId = node.node_id ?? `${node.file}:${node.line}`;
		try {
			const file = await readFileMeta(node.file);
			const lines = file.content.split('\n');
			const idx = node.line;
			if (idx < 0 || idx >= lines.length || !isHeadlineLine(lines[idx])) {
				error = 'That task moved since the agenda was loaded. Refreshing.';
				await refresh();
				return;
			}
			const next = fn(lines, idx);
			if (next.join('\n') !== file.content) {
				await saveFile(node.file, next.join('\n'), file.hash);
			}
			await refresh();
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) {
			const message = String(e);
			error = message.includes('CONFLICT:')
				? 'That file changed on disk. The agenda has been refreshed — try again.'
				: message;
			if (message.includes('CONFLICT:')) await refresh();
		}
		finally { changingId = null; }
	}

	/**
	 * Move a task to a new state. Completing a repeating task shifts its planning
	 * dates to the next occurrence and keeps it open, as org does; completing a
	 * normal task stamps CLOSED.
	 */
	async function setState(node: HeadlineRecord, state: string | null) {
		await editHeadline(node, (lines, idx) => {
			const config = keywordConfig();
			const wasDone = isDoneKeyword(getTodoKeyword(lines[idx], config), config);
			const becomesDone = isDoneKeyword(state, config);
			let next = [...lines];

			if (becomesDone && !wasDone) {
				const repeat = applyRepeaterOnDone(next, idx, nowForOrg());
				if (repeat.repeated) {
					next = repeat.lines;
					next[idx] = setTodoKeyword(next[idx], repeatKeyword(config), config);
					return next;
				}
				next[idx] = setTodoKeyword(next[idx], state, config);
				return setClosed(next, idx, nowForOrg());
			}

			next[idx] = setTodoKeyword(next[idx], state, config);
			return wasDone && !becomesDone ? setClosed(next, idx, null) : next;
		});
	}

	async function setPriority(node: HeadlineRecord, priority: string | null) {
		await editHeadline(node, (lines, idx) => {
			const next = [...lines];
			next[idx] = setHeadlinePriority(next[idx], priority, keywordConfig());
			return next;
		});
	}

	/** Set or clear a planning date, preserving that entry's own repeater cookies. */
	async function setDate(node: HeadlineRecord, type: PlanningKind, datetime: string | null) {
		await editHeadline(node, (lines, idx) =>
			datetime ? setPlanningDate(lines, idx, type, datetime) : removePlanning(lines, idx, type)
		);
	}
</script>

<div class="flex h-full flex-col">
	<header class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: calc(env(safe-area-inset-top, 0px) + 8px); padding-bottom: 8px; min-height: 48px;">
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

				{#each weekDays(today) as day}
					{@const dayItems = itemsForDate(day.date)}
					<div class="border-b border-surface-100 px-4 py-2 dark:border-surface-800">
						<h3 class="mb-1 text-[11px] font-bold uppercase tracking-wide {day.isToday ? 'text-mycelium-700 dark:text-mycelium-400' : 'text-surface-700 dark:text-surface-300'}">
							{day.label} <span class="font-normal opacity-60">{day.date}</span>
						</h3>
						{#if dayItems.length > 0}
							{#each dayItems as di}
								{#if di.label}
									<div class="flex items-start gap-1.5">
										<span class="mt-1.5 shrink-0 rounded px-1 text-[10px] font-medium {di.reason === 'overdue-scheduled' ? 'bg-red-50 text-red-600 dark:bg-red-950 dark:text-red-400' : 'bg-orange-50 text-orange-600 dark:bg-orange-950 dark:text-orange-400'}">{di.label}</span>
										<div class="min-w-0 flex-1">{@render taskRow(di.node)}</div>
									</div>
								{:else}
									{@render taskRow(di.node)}
								{/if}
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

{#snippet taskRow(item: HeadlineRecord)}
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
		<div data-inner class="bg-surface-0 dark:bg-surface-950" style="position:relative;display:flex;align-items:center;gap:8px;padding:8px 4px;will-change:transform;{changingId === (item.node_id ?? `${item.file}:${item.line}`) ? 'opacity:0.5;' : ''}">
			<select
				value={item.todo ?? ''}
				onchange={(e) => setState(item, (e.target as HTMLSelectElement).value || null)}
				disabled={changingId === (item.node_id ?? `${item.file}:${item.line}`)}
				style="height:28px;flex-shrink:0;border-radius:4px;border:0;padding:0 16px 0 4px;font-size:10px;font-weight:700;color:{isDone(item) ? '#16a34a' : item.todo ? '#dc2626' : '#6b7280'};background:{isDone(item) ? '#f0fdf4' : item.todo ? '#fef2f2' : 'transparent'}"
			>
				<option value="">None</option>
				{#each orgConfig.todoKeywords as kw}<option value={kw}>{kw}</option>{/each}
				{#each orgConfig.doneKeywords as kw}<option value={kw}>{kw}</option>{/each}
			</select>

			<button onclick={() => navigateToItem(item)} style="min-width:0;flex:1;text-align:left">
				<div style="font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;{isDone(item) ? 'text-decoration:line-through;opacity:0.6' : 'font-weight:500'}">{item.title ?? 'Untitled'}</div>
				{#if dlDate || scDate}
					<div style="display:flex;gap:6px;font-size:10px;margin-top:2px;flex-wrap:wrap">
						{#if dlDate}
							<span style="display:inline-flex;align-items:center;gap:2px;padding:1px 4px;border-radius:3px;background:{isPastDue(item) ? '#fef2f2' : '#fff7ed'};color:{isPastDue(item) ? '#dc2626' : '#ea580c'}">
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
				disabled={changingId === (item.node_id ?? `${item.file}:${item.line}`)}
				style="height:28px;flex-shrink:0;border-radius:4px;border:0;padding:0 12px 0 4px;font-size:10px;font-weight:700;color:#ea580c;{item.priority ? 'background:#fff7ed' : 'background:transparent'}"
			>
				<option value="">—</option>
				{#each orgConfig.priorities as p}<option value={p}>#{p}</option>{/each}
			</select>
		</div>
	</div>
{/snippet}
