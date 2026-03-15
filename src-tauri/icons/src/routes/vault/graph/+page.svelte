<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { getGraphData } from '$lib/tauri/commands';
	import GraphView from '$lib/components/graph/GraphView.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { GraphData } from '$lib/types/node';

	let graphData = $state<GraphData | null>(null);
	let error = $state<string | null>(null);
	let graphView: GraphView;
	let showOrphans = $state(false);

	const orphanNodes = $derived(
		graphData
			? graphData.nodes.filter(n => n.link_count === 0)
			: []
	);

	$effect(() => { loadGraph(); });

	async function loadGraph() {
		try { graphData = await getGraphData(); }
		catch (e) { error = String(e); }
	}

	function openRandom() {
		const nodes = vault.nodes;
		if (nodes.length === 0) return;
		const random = nodes[Math.floor(Math.random() * nodes.length)];
		navigation.navigateToNode(random.id);
	}
</script>

<div class="flex h-full flex-col">
	<header class="flex h-14 shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: var(--safe-area-top)">
		<button onclick={() => navigation.navigateToVault()} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>
		<h1 class="flex-1 text-lg font-semibold">Graph</h1>
		{#if graphData}
			<span class="text-xs text-surface-700 dark:text-surface-300">{graphData.nodes.length} nodes</span>
		{/if}
	</header>

	{#if error}<div class="bg-red-50 p-3 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>{/if}

	<div class="relative flex-1">
		{#if graphData}
			<GraphView data={graphData} bind:this={graphView} />

			<!-- Controls -->
			<div class="absolute bottom-4 right-4 flex flex-col gap-1.5">
				<button onclick={openRandom} class="rounded-lg bg-surface-0 p-2.5 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700" title="Open random node">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 12c0-1.232-.046-2.453-.138-3.662a4.006 4.006 0 00-3.7-3.7 48.678 48.678 0 00-7.324 0 4.006 4.006 0 00-3.7 3.7c-.017.22-.032.441-.046.662M19.5 12l3-3m-3 3l-3-3m-12 3c0 1.232.046 2.453.138 3.662a4.006 4.006 0 003.7 3.7 48.656 48.656 0 007.324 0 4.006 4.006 0 003.7-3.7c.017-.22.032-.441.046-.662M4.5 12l3 3m-3-3l-3 3" /></svg>
				</button>
				<button onclick={() => (showOrphans = !showOrphans)} class="rounded-lg p-2.5 shadow-md {showOrphans ? 'bg-mycelium-600 text-white' : 'bg-surface-0 hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700'}" title="Orphan nodes ({orphanNodes.length})">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15 12H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
				</button>
				<div class="h-px bg-surface-200 dark:bg-surface-700"></div>
				<button onclick={() => graphView?.zoomIn()} class="rounded-lg bg-surface-0 p-2.5 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700" title="Zoom in">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" /></svg>
				</button>
				<button onclick={() => graphView?.zoomOut()} class="rounded-lg bg-surface-0 p-2.5 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700" title="Zoom out">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 12h-15" /></svg>
				</button>
				<button onclick={() => graphView?.resetZoom()} class="rounded-lg bg-surface-0 p-2.5 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700" title="Reset zoom">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" /></svg>
				</button>
			</div>

			<!-- Orphan nodes panel -->
			{#if showOrphans}
				<div class="absolute bottom-4 left-4 max-h-[60vh] w-72 overflow-y-auto rounded-lg border border-surface-200 bg-surface-0 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<div class="sticky top-0 flex items-center justify-between border-b border-surface-200 bg-surface-0 px-3 py-2 dark:border-surface-700 dark:bg-surface-900">
						<span class="text-sm font-semibold">Orphan Nodes ({orphanNodes.length})</span>
						<button onclick={() => (showOrphans = false)} class="rounded p-1 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Close">
							<svg class="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
						</button>
					</div>
					{#if orphanNodes.length > 0}
						<ul class="py-1">
							{#each orphanNodes as node}
								<li>
									<button onclick={() => navigation.navigateToNode(node.id)} class="w-full px-3 py-2 text-left text-sm hover:bg-surface-100 dark:hover:bg-surface-800">
										{node.title ?? node.id}
									</button>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="p-3 text-center text-sm text-surface-700 dark:text-surface-300">No orphan nodes</p>
					{/if}
				</div>
			{/if}
		{:else}
			<div class="flex h-full items-center justify-center"><p class="text-surface-700 dark:text-surface-300">Loading graph...</p></div>
		{/if}
	</div>

	<MobileNav />
</div>
