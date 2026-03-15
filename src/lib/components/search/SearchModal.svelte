<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { searchNodes } from '$lib/tauri/commands';
	import type { NodeRecord } from '$lib/types/node';

	let query = $state('');
	let results = $state<NodeRecord[]>([]);
	let selectedIndex = $state(0);
	let searchTimeout: ReturnType<typeof setTimeout>;

	function handleInput() {
		clearTimeout(searchTimeout);
		if (!query.trim()) {
			results = [];
			return;
		}
		searchTimeout = setTimeout(doSearch, 150);
	}

	async function doSearch() {
		if (!query.trim()) return;
		try {
			results = await searchNodes(query.trim());
			selectedIndex = 0;
		} catch {
			results = [];
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = Math.min(selectedIndex + 1, results.length - 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = Math.max(selectedIndex - 1, 0);
		} else if (e.key === 'Enter' && results[selectedIndex]) {
			e.preventDefault();
			selectResult(results[selectedIndex]);
		} else if (e.key === 'Escape') {
			navigation.toggleSearch();
		}
	}

	function selectResult(node: NodeRecord) {
		navigation.toggleSearch();
		navigation.navigateToNode(node.id);
	}
</script>

{#if navigation.searchOpen}
	<!-- Backdrop -->
	<button
		class="fixed inset-0 z-40 bg-black/50"
		onclick={() => navigation.toggleSearch()}
		aria-label="Close search"
	></button>

	<!-- Modal -->
	<div class="fixed inset-x-4 top-[10%] z-50 mx-auto max-w-lg rounded-xl border border-surface-200 bg-surface-0 shadow-2xl dark:border-surface-700 dark:bg-surface-900">
		<input
			type="text"
			bind:value={query}
			oninput={handleInput}
			onkeydown={handleKeydown}
			placeholder="Search nodes..."
			class="w-full rounded-t-xl border-b border-surface-200 bg-transparent px-4 py-3 text-sm focus:outline-none dark:border-surface-700"
			autofocus
		/>

		{#if results.length > 0}
			<ul class="max-h-64 overflow-y-auto p-1">
				{#each results as node, i}
					<li>
						<button
							onclick={() => selectResult(node)}
							class="w-full rounded-lg px-3 py-2.5 text-left text-sm"
							class:bg-mycelium-50={i === selectedIndex}
							class:dark:bg-mycelium-950={i === selectedIndex}
						>
							<div class="font-medium">{node.title ?? 'Untitled'}</div>
							<div class="mt-0.5 text-xs text-surface-700 dark:text-surface-300">
								{node.file}
							</div>
						</button>
					</li>
				{/each}
			</ul>
		{:else if query.trim()}
			<div class="p-4 text-center text-sm text-surface-700 dark:text-surface-300">
				No results
			</div>
		{/if}
	</div>
{/if}
