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
		isHeadlineLine, isOverdue, keywordCategoryClass, overdueItems as findOverdue,
		repeatKeyword, setClosed, setPlanningDate, setPriority as setHeadlinePriority,
		setTodoKeyword, removePlanning, timestampDate,
	} from '$lib/org';
	import type { PlanningKind } from '$lib/org';

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

	function priorityConfig() {
		return { priorities: orgConfig.priorities };
	}

	function nowForOrg() {
		return { date: localDate(), time: localTime() };
	}

	/** Stable identity for a headline, for keyed lists and per-row UI state. */
	function rowKey(item: HeadlineRecord): string {
		return item.node_id ?? `${item.file}:${item.line}`;
	}

	function extractDate(raw: string | null): string {
		return timestampDate(raw) ?? '';
	}

	function extractTime(raw: string | null): string {
		const m = raw?.match(/(\d{1,2}:\d{2})/);
		return m ? m[1] : '';
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

	/** A tap anywhere dismisses an open swipe before it does anything else. */
	function activateRow(item: HeadlineRecord) {
		if (openRow !== null) { openRow = null; return; }
		navigateToItem(item);
	}

	function isPastDue(n: HeadlineRecord): boolean {
		const dl = extractDate(n.deadline);
		return !!dl && dl < today && !isDone(n);
	}

	function isDone(n: HeadlineRecord): boolean {
		return isDoneKeyword(n.todo ?? null, orgConfig.keywordConfig);
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
			// Late work is listed in the Overdue block above. Without this it was
			// drawn a second time under Today, doubling every overdue task.
			if (date === today && isOverdue(n, today, done)) continue;
			const source = reason.includes('deadline') ? n.deadline : n.scheduled;
			result.push({
				node: n,
				reason,
				time: extractTime(source),
				label: reason === 'upcoming-deadline'
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

	// ── Swipe-to-edit ───────────────────────────────────────────
	// The row tracks its offset in state rather than writing transforms onto the
	// DOM: an imperative transform survived the keyed diff on refresh and stranded
	// an open row's offset on whatever task took its place.

	/** Travel before the gesture commits to an axis. */
	const AXIS_LOCK = 10;

	let openRow = $state<string | null>(null);
	let dragRow = $state<string | null>(null);
	let dragOffset = $state(0);
	let dragWidth = $state(0);

	let gesture:
		| { key: string; x: number; y: number; width: number; base: number; axis: 'h' | 'v' | null }
		| null = null;

	function onRowTouchStart(event: TouchEvent, key: string) {
		const touch = event.touches[0];
		if (!touch) return;
		// The action panel is sized in vw/rem, so its width is read from layout
		// rather than hardcoded — it differs per screen and after a rotation.
		const actions = (event.currentTarget as HTMLElement).querySelector('[data-actions]');
		const width = actions instanceof HTMLElement ? actions.offsetWidth : 0;
		if (width === 0) return;
		gesture = {
			key,
			x: touch.clientX,
			y: touch.clientY,
			width,
			base: openRow === key ? -width : 0,
			axis: null,
		};
		dragWidth = width;
	}

	function onRowTouchMove(event: TouchEvent) {
		if (!gesture) return;
		const touch = event.touches[0];
		if (!touch) return;
		const dx = touch.clientX - gesture.x;
		const dy = touch.clientY - gesture.y;

		if (gesture.axis === null) {
			if (Math.abs(dx) < AXIS_LOCK && Math.abs(dy) < AXIS_LOCK) return;
			// Ties go to the list: it is scrolled far more often than a row is swiped.
			gesture.axis = Math.abs(dx) > Math.abs(dy) ? 'h' : 'v';
			if (gesture.axis === 'h') dragRow = gesture.key;
		}
		if (gesture.axis !== 'h') return;

		dragOffset = Math.max(-gesture.width, Math.min(0, gesture.base + dx));
	}

	function onRowTouchEnd() {
		if (!gesture) return;
		if (gesture.axis === 'h') {
			openRow = dragOffset < -gesture.width / 2 ? gesture.key : null;
		}
		gesture = null;
		dragRow = null;
		dragOffset = 0;
	}

	/** How far the actions are revealed, 0–1, for fading them in. */
	function revealed(key: string): number {
		if (dragRow === key) return dragWidth > 0 ? Math.min(1, Math.abs(dragOffset) / dragWidth) : 0;
		return openRow === key ? 1 : 0;
	}

	// ── Inline editing ──────────────────────────────────────────

	/**
	 * Rewrite one headline in its file. The headline is located by line number and
	 * verified to still be a headline, so a stale index can never rewrite prose.
	 */
	async function editHeadline(
		node: HeadlineRecord,
		fn: (lines: string[], idx: number) => string[]
	) {
		changingId = rowKey(node);
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
		finally {
			changingId = null;
			openRow = null;
		}
	}

	/**
	 * Move a task to a new state. Completing a repeating task shifts its planning
	 * dates to the next occurrence and keeps it open, as org does; completing a
	 * normal task stamps CLOSED.
	 */
	async function setState(node: HeadlineRecord, state: string | null) {
		await editHeadline(node, (lines, idx) => {
			const config = orgConfig.keywordConfig;
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
			next[idx] = setHeadlinePriority(next[idx], priority, orgConfig.keywordConfig);
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
	<header class="agenda-gutter flex shrink-0 items-center gap-2 border-b border-surface-200 dark:border-surface-700" style="padding-top: calc(var(--safe-area-top) + 0.5rem); padding-bottom: 0.5rem; min-height: var(--tap);">
		<button onclick={() => navigation.navigateToVault()} class="flex shrink-0 items-center justify-center rounded-lg hover:bg-surface-100 dark:hover:bg-surface-800" style="min-width:var(--tap);min-height:var(--tap)" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>
		<h1 class="flex-1 text-lg font-semibold">Agenda</h1>
		<span class="text-fluid-sm text-surface-600 dark:text-surface-400">{items.length} items</span>
	</header>

	<!-- Tabs -->
	<div class="flex shrink-0 border-b border-surface-200 dark:border-surface-700">
		<button onclick={() => (tab = 'agenda')} style="min-height:var(--tap)" class="flex-1 text-fluid-sm font-medium transition-colors {tab === 'agenda' ? 'border-b-2 border-mycelium-600 text-mycelium-700 dark:text-mycelium-300' : 'text-surface-700 dark:text-surface-300'}">Week</button>
		<button onclick={() => (tab = 'tasks')} style="min-height:var(--tap)" class="flex-1 text-fluid-sm font-medium transition-colors {tab === 'tasks' ? 'border-b-2 border-mycelium-600 text-mycelium-700 dark:text-mycelium-300' : 'text-surface-700 dark:text-surface-300'}">All Tasks</button>
	</div>

	{#if error}<div class="agenda-gutter bg-red-50 py-2 text-fluid-sm text-red-700 dark:bg-red-950 dark:text-red-300">{error}</div>{/if}

	<div class="flex-1 overflow-y-auto">
		<div class="agenda-width">
		{#if tab === 'agenda'}
			<!-- Weekly agenda -->
			<div>
				{#if overdueItems.length > 0}
					<div class="overdue-band agenda-gutter border-b border-surface-200 py-2 dark:border-surface-800">
						<h3 class="overdue-band-title text-fluid-xs mb-1.5 font-bold uppercase tracking-wide">Overdue ({overdueItems.length})</h3>
						{#each overdueItems as item (rowKey(item))}
							{@render taskRow(item)}
						{/each}
					</div>
				{/if}

				{#each weekDays(today) as day (day.date)}
					{@const dayItems = itemsForDate(day.date)}
					<div class="agenda-gutter border-b border-surface-100 py-2 dark:border-surface-800">
						<h3 class="text-fluid-xs mb-1 font-bold uppercase tracking-wide {day.isToday ? 'text-mycelium-700 dark:text-mycelium-400' : 'text-surface-600 dark:text-surface-400'}">
							{day.label} <span class="font-normal opacity-70">{day.date}</span>
						</h3>
						{#if dayItems.length > 0}
							{#each dayItems as di (rowKey(di.node))}
								{#if di.label}
									<div class="flex items-center gap-1.5">
										<span class="state-chip state-deadline shrink-0">{di.label}</span>
										<div class="min-w-0 flex-1">{@render taskRow(di.node)}</div>
									</div>
								{:else}
									{@render taskRow(di.node)}
								{/if}
							{/each}
						{:else}
							<p class="text-fluid-xs py-0.5 text-surface-500 dark:text-surface-500">—</p>
						{/if}
					</div>
				{/each}
			</div>

		{:else}
			<!-- All tasks with search/filter -->
			<div class="agenda-gutter border-b border-surface-200 py-2 dark:border-surface-700">
				<div class="flex gap-2">
					<div class="relative flex-1">
						<svg class="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-surface-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" /></svg>
						<input type="text" bind:value={taskSearch} placeholder="Filter tasks..." aria-label="Filter tasks by title" class="text-fluid-sm w-full rounded-lg border border-surface-200 bg-surface-50 pl-8 pr-3 dark:border-surface-700 dark:bg-surface-900" style="min-height:var(--tap)" />
					</div>
					<select bind:value={taskFilter} aria-label="Filter by state" class="text-fluid-xs rounded-lg border border-surface-200 bg-surface-50 px-2 dark:border-surface-700 dark:bg-surface-900" style="min-height:var(--tap)">
						<option value="all">All</option>
						{@render keywordOptions()}
					</select>
				</div>
			</div>

			<div class="divide-y divide-surface-100 dark:divide-surface-800">
				{#each filteredTasks as item (rowKey(item))}
					<div class="agenda-gutter py-1">
						{@render taskRow(item)}
					</div>
				{/each}
			</div>
			{#if filteredTasks.length === 0}
				<p class="text-fluid-sm p-8 text-center text-surface-600 dark:text-surface-400">No matching tasks</p>
			{/if}
		{/if}
		</div>
	</div>

	<MobileNav />
</div>

<!-- Keyword choices, grouped so the three categories are visible where states are picked. -->
{#snippet keywordOptions()}
	{#if orgConfig.todoKeywords.length > 0}
		<optgroup label="Active">
			{#each orgConfig.todoKeywords as kw}<option value={kw}>{kw}</option>{/each}
		</optgroup>
	{/if}
	{#if orgConfig.waitingKeywords.length > 0}
		<optgroup label="Waiting">
			{#each orgConfig.waitingKeywords as kw}<option value={kw}>{kw}</option>{/each}
		</optgroup>
	{/if}
	{#if orgConfig.doneKeywords.length > 0}
		<optgroup label="Done">
			{#each orgConfig.doneKeywords as kw}<option value={kw}>{kw}</option>{/each}
		</optgroup>
	{/if}
{/snippet}

{#snippet taskRow(item: HeadlineRecord)}
	{@const key = rowKey(item)}
	{@const busy = changingId === key}
	{@const isOpen = openRow === key}
	{@const dlDate = extractDate(item.deadline)}
	{@const dlTime = extractTime(item.deadline)}
	{@const scDate = extractDate(item.scheduled)}
	{@const scTime = extractTime(item.scheduled)}
	<div
		class="swipe-row"
		style="position:relative;overflow:hidden;border-radius:0.5rem"
		ontouchstart={(e) => onRowTouchStart(e, key)}
		ontouchmove={onRowTouchMove}
		ontouchend={onRowTouchEnd}
		ontouchcancel={onRowTouchEnd}
	>
		<!-- Planning actions, revealed by swiping the row left -->
		<div
			data-actions
			inert={!isOpen}
			style="position:absolute;right:0;top:0;bottom:0;display:flex;opacity:{revealed(key)};transition:{dragRow === key ? 'none' : 'opacity 0.25s ease'}"
		>
			<label class="swipe-action swipe-action--deadline">
				Deadline
				<input type="datetime-local" tabindex={isOpen ? 0 : -1} aria-label="Deadline for {item.title ?? 'untitled task'}" value={dlDate && dlTime ? `${dlDate}T${dlTime}` : dlDate} onchange={(e) => setDate(item, 'DEADLINE', (e.target as HTMLInputElement).value || null)} />
				<span>{dlDate ? (dlTime ? `${dlDate} ${dlTime}` : dlDate) : 'set'}</span>
			</label>
			<label class="swipe-action swipe-action--sched">
				Schedule
				<input type="datetime-local" tabindex={isOpen ? 0 : -1} aria-label="Scheduled date for {item.title ?? 'untitled task'}" value={scDate && scTime ? `${scDate}T${scTime}` : scDate} onchange={(e) => setDate(item, 'SCHEDULED', (e.target as HTMLInputElement).value || null)} />
				<span>{scDate ? (scTime ? `${scDate} ${scTime}` : scDate) : 'set'}</span>
			</label>
		</div>

		<!-- Main row -->
		<div
			class="swipe-inner bg-surface-0 dark:bg-surface-950 {dragRow === key ? '' : 'swipe-inner--settling'} {openRow === key && dragRow !== key ? 'swipe-inner--open' : ''}"
			style="position:relative;display:flex;align-items:center;gap:0.5rem;padding:0.25rem;min-height:var(--row-min);will-change:transform;{dragRow === key ? `transform:translateX(${dragOffset}px);` : ''}{busy ? 'opacity:0.5;' : ''}"
		>
			<!-- State. The chip stays small; the select over it fills the tap target. -->
			<span class="tap-wrap">
				<span class="state-chip {keywordCategoryClass(item.todo, orgConfig.categoryConfig)}">{item.todo ?? '—'}</span>
				<select
					class="tap-target"
					aria-label="State for {item.title ?? 'untitled task'}"
					value={item.todo ?? ''}
					onchange={(e) => setState(item, (e.target as HTMLSelectElement).value || null)}
					disabled={busy}
				>
					<option value="">None</option>
					{@render keywordOptions()}
				</select>
			</span>

			<button onclick={() => activateRow(item)} style="min-width:0;flex:1;text-align:left;align-self:stretch;padding:0.375rem 0">
				<div class="text-fluid-md" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;{isDone(item) ? 'text-decoration:line-through;opacity:0.6' : 'font-weight:500'}">{item.title ?? 'Untitled'}</div>
				{#if dlDate || scDate}
					<div style="display:flex;gap:0.375rem;margin-top:0.2rem;flex-wrap:wrap">
						{#if dlDate}
							<span class="state-chip {isPastDue(item) ? 'state-overdue' : 'state-deadline'}">
								<span style="font-weight:700">DL</span> {dlDate}{#if dlTime} {dlTime}{/if}
							</span>
						{/if}
						{#if scDate}
							<span class="state-chip state-sched">
								<span style="font-weight:700">SC</span> {scDate}{#if scTime} {scTime}{/if}
							</span>
						{/if}
					</div>
				{/if}
			</button>

			<!-- Priority, same chip-over-target arrangement -->
			<span class="tap-wrap">
				<span class="state-chip {item.priority ? 'state-priority' : 'state-none'}">{item.priority ? `#${item.priority}` : '—'}</span>
				<select
					class="tap-target"
					aria-label="Priority for {item.title ?? 'untitled task'}"
					value={item.priority ?? ''}
					onchange={(e) => setPriority(item, (e.target as HTMLSelectElement).value || null)}
					disabled={busy}
				>
					<option value="">None</option>
					{#each orgConfig.priorities as p}<option value={p}>#{p}</option>{/each}
				</select>
			</span>
		</div>
	</div>
{/snippet}
