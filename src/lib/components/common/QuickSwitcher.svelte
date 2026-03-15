<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import type { NodeRecord } from '$lib/types/node';

	let {
		open = false,
		mode = 'navigate' as 'navigate' | 'insert-link',
		onclose,
		oninsert,
	}: {
		open?: boolean;
		mode?: 'navigate' | 'insert-link';
		onclose?: () => void;
		oninsert?: (node: NodeRecord) => void;
	} = $props();

	let query = $state('');
	let selectedIndex = $state(0);

	const filtered = $derived(() => {
		if (!query.trim()) return vault.nodes.slice(0, 15);
		const q = query.toLowerCase();
		return vault.nodes
			.filter(
				(n) =>
					n.title?.toLowerCase().includes(q) ||
					n.id.toLowerCase().includes(q) ||
					n.file.toLowerCase().includes(q)
			)
			.slice(0, 15);
	});

	function handleKeydown(e: KeyboardEvent) {
		const items = filtered();
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = Math.min(selectedIndex + 1, items.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = Math.max(selectedIndex - 1, 0);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const item = items[selectedIndex];
			if (item) selectNode(item);
		} else if (e.key === 'Escape') {
			onclose?.();
		}
	}

	function selectNode(node: NodeRecord) {
		if (mode === 'insert-link') {
			oninsert?.(node);
		} else {
			navigation.navigateToNode(node.id);
		}
		query = '';
		selectedIndex = 0;
		onclose?.();
	}

	$effect(() => {
		if (open) {
			query = '';
			selectedIndex = 0;
		}
	});
</script>

{#if open}
	<!-- Backdrop -->
	<button
		class="fixed inset-0 z-50 bg-black/50"
		onclick={() => onclose?.()}
		aria-label="Close"
	></button>

	<!-- Modal -->
	<div class="fixed inset-x-4 top-[12%] z-50 mx-auto max-w-lg overflow-hidden rounded-xl border border-surface-200 bg-surface-0 shadow-2xl dark:border-surface-700 dark:bg-surface-900">
		<!-- Search input -->
		<div class="flex items-center gap-3 border-b border-surface-200 px-4 dark:border-surface-700">
			{#if mode === 'insert-link'}
				<svg class="h-4 w-4 shrink-0 text-mycelium-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 011.242 7.244l-4.5 4.5a4.5 4.5 0 01-6.364-6.364l1.757-1.757m9.86-1.125a4.5 4.5 0 00-1.242-7.244l-4.5-4.5a4.5 4.5 0 00-6.364 6.364L4.757 8.688" />
				</svg>
			{:else}
				<svg class="h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
				</svg>
			{/if}
			<input
				type="text"
				bind:value={query}
				onkeydown={handleKeydown}
				placeholder={mode === 'insert-link' ? 'Search nodes to link...' : 'Jump to node...'}
				class="w-full bg-transparent py-3.5 text-sm focus:outline-none"
				autofocus
			/>
			<kbd class="hidden shrink-0 rounded border border-surface-200 px-1.5 py-0.5 text-[10px] text-surface-700 dark:border-surface-700 dark:text-surface-300 sm:block">
				ESC
			</kbd>
		</div>

		<!-- Results -->
		<ul class="max-h-72 overflow-y-auto p-1">
			{#each filtered() as node, i}
				<li>
					<button
						onclick={() => selectNode(node)}
						class="flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition-colors {i === selectedIndex
							? 'bg-mycelium-50 dark:bg-mycelium-950'
							: 'hover:bg-surface-100 dark:hover:bg-surface-800'}"
					>
						<!-- Icon based on type -->
						{#if node.todo}
							<span class="shrink-0 text-xs font-bold text-red-500">{node.todo}</span>
						{:else if node.level === 0}
							<svg class="h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
								<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
							</svg>
						{:else}
							<span class="shrink-0 text-xs text-surface-700 dark:text-surface-300">{'*'.repeat(node.level)}</span>
						{/if}
						<div class="min-w-0 flex-1">
							<div class="truncate font-medium">{node.title ?? 'Untitled'}</div>
							<div class="truncate text-[10px] text-surface-700/60 dark:text-surface-300/60">
								{node.file.split('/').pop()}
							</div>
						</div>
						{#if mode === 'insert-link'}
							<span class="shrink-0 text-[10px] text-mycelium-600 dark:text-mycelium-400">insert</span>
						{/if}
					</button>
				</li>
			{/each}
			{#if filtered().length === 0}
				<li class="px-3 py-4 text-center text-sm text-surface-700 dark:text-surface-300">
					No matching nodes
				</li>
			{/if}
		</ul>

		<!-- Footer hint -->
		<div class="border-t border-surface-200 px-4 py-2 dark:border-surface-700">
			<p class="text-[10px] text-surface-700 dark:text-surface-300">
				<kbd class="rounded border border-surface-200 px-1 dark:border-surface-700">&uarr;&darr;</kbd> navigate
				<kbd class="ml-2 rounded border border-surface-200 px-1 dark:border-surface-700">&crarr;</kbd> {mode === 'insert-link' ? 'insert link' : 'open'}
			</p>
		</div>
	</div>
{/if}
