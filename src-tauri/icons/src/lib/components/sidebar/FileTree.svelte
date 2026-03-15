<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';

	// Find nodes for each file
	function nodesForFile(filePath: string) {
		return vault.nodes.filter((n) => n.file === filePath);
	}
</script>

<ul class="space-y-0.5">
	{#each vault.files as file}
		{@const nodes = nodesForFile(file.file)}
		<li>
			<div class="rounded-lg px-2 py-1.5 text-sm">
				<div class="truncate font-medium text-surface-700 dark:text-surface-300" title={file.file}>
					{file.title ?? file.file.split('/').pop() ?? file.file}
				</div>
				{#if nodes.length > 0}
					<ul class="ml-3 mt-1 space-y-0.5">
						{#each nodes as node}
							<li>
								<button
									onclick={() => navigation.navigateToNode(node.id)}
									class="w-full truncate rounded px-2 py-1 text-left text-xs hover:bg-surface-200 dark:hover:bg-surface-700"
								>
									{node.title ?? 'Untitled'}
								</button>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		</li>
	{/each}
</ul>
