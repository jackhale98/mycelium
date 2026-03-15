<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import type { NodeRecord } from '$lib/types/node';

	let { node, highlighted = false }: { node: NodeRecord; highlighted?: boolean } = $props();
</script>

<button
	onclick={() => navigation.navigateToNode(node.id)}
	class="w-full rounded-lg px-3 py-2.5 text-left text-sm transition-colors"
	class:bg-mycelium-50={highlighted}
	class:dark:bg-mycelium-950={highlighted}
	class:hover:bg-surface-100={!highlighted}
	class:dark:hover:bg-surface-800={!highlighted}
>
	<div class="font-medium">{node.title ?? 'Untitled'}</div>
	{#if node.todo}
		<span class="mr-1 inline-block rounded bg-mycelium-100 px-1 py-0.5 text-xs font-medium text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300">
			{node.todo}
		</span>
	{/if}
	<div class="mt-0.5 truncate text-xs text-surface-700 dark:text-surface-300">
		{node.file}
	</div>
</button>
