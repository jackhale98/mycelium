<script lang="ts">
	import { onMount } from 'svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import { getAgenda } from '$lib/tauri/commands';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { NodeRecord } from '$lib/types/node';

	let items = $state<NodeRecord[]>([]);
	let error = $state<string | null>(null);
	let filter = $state<'all' | 'todo' | 'scheduled' | 'deadline'>('all');

	const filtered = $derived(
		items.filter(n => {
			if (filter === 'todo') return n.todo && n.todo !== 'DONE';
			if (filter === 'scheduled') return n.scheduled;
			if (filter === 'deadline') return n.deadline;
			return true;
		})
	);

	const doneCount = $derived(items.filter(n => n.todo === 'DONE').length);
	const activeCount = $derived(items.filter(n => n.todo && n.todo !== 'DONE').length);

	onMount(async () => {
		try { items = await getAgenda(); }
		catch (e) { error = String(e); }
	});

	function extractDate(raw: string | null): string {
		if (!raw) return '';
		const m = raw.match(/(\d{4}-\d{2}-\d{2})/);
		return m ? m[1] : raw;
	}

	function isOverdue(deadline: string | null): boolean {
		if (!deadline) return false;
		const d = extractDate(deadline);
		if (!d) return false;
		return d < new Date().toISOString().slice(0, 10);
	}
</script>

<div class="flex h-full flex-col">
	<header class="flex h-14 shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: var(--safe-area-top)">
		<button onclick={() => navigation.navigateToVault()} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>
		<h1 class="flex-1 text-lg font-semibold">Agenda</h1>
		<span class="text-xs text-surface-700 dark:text-surface-300">{activeCount} active, {doneCount} done</span>
	</header>

	<!-- Filter tabs -->
	<div class="flex shrink-0 gap-1 border-b border-surface-200 px-4 py-2 dark:border-surface-700">
		{#each [
			{ id: 'all' as const, label: 'All' },
			{ id: 'todo' as const, label: 'Active' },
			{ id: 'deadline' as const, label: 'Deadlines' },
			{ id: 'scheduled' as const, label: 'Scheduled' },
		] as tab}
			<button
				onclick={() => (filter = tab.id)}
				class="rounded-lg px-3 py-1.5 text-xs font-medium transition-colors {filter === tab.id
					? 'bg-mycelium-100 text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300'
					: 'text-surface-700 hover:bg-surface-100 dark:text-surface-300 dark:hover:bg-surface-800'}"
			>{tab.label}</button>
		{/each}
	</div>

	{#if error}<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>{/if}

	<div class="flex-1 overflow-y-auto">
		{#if filtered.length > 0}
			<ul class="divide-y divide-surface-100 dark:divide-surface-800">
				{#each filtered as item}
					<li>
						<button
							onclick={() => navigation.navigateToNode(item.id)}
							class="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-surface-50 dark:hover:bg-surface-800/50"
						>
							<!-- TODO badge -->
							<div class="mt-0.5 shrink-0">
								{#if item.todo === 'DONE'}
									<span class="inline-block rounded px-1.5 py-0.5 text-[10px] font-bold" style="color:#16a34a;background:#f0fdf4">DONE</span>
								{:else if item.todo}
									<span class="inline-block rounded px-1.5 py-0.5 text-[10px] font-bold" style="color:#dc2626;background:#fef2f2">{item.todo}</span>
								{/if}
							</div>
							<!-- Content -->
							<div class="min-w-0 flex-1">
								<div class="truncate font-medium">{item.title ?? 'Untitled'}</div>
								<div class="mt-1 flex flex-wrap gap-2 text-xs">
									{#if item.deadline}
										<span class="{isOverdue(item.deadline) ? 'text-red-600 font-semibold dark:text-red-400' : 'text-surface-700 dark:text-surface-300'}">
											Deadline: {extractDate(item.deadline)}
											{#if isOverdue(item.deadline)} (overdue){/if}
										</span>
									{/if}
									{#if item.scheduled}
										<span class="text-surface-700 dark:text-surface-300">
											Scheduled: {extractDate(item.scheduled)}
										</span>
									{/if}
									{#if item.priority}
										<span style="color:#ea580c;font-weight:600">[#{item.priority}]</span>
									{/if}
								</div>
							</div>
							<svg class="mt-1 h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" /></svg>
						</button>
					</li>
				{/each}
			</ul>
		{:else}
			<div class="p-8 text-center text-sm text-surface-700 dark:text-surface-300">
				{#if filter === 'all'}No tasks found. Add TODO keywords to your org headings.
				{:else}No matching items.{/if}
			</div>
		{/if}
	</div>

	<MobileNav />
</div>
