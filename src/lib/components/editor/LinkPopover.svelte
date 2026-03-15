<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';

	let {
		nodeId,
		title,
		x = 0,
		y = 0,
		visible = false,
	}: {
		nodeId: string;
		title: string;
		x?: number;
		y?: number;
		visible?: boolean;
	} = $props();

	function navigate() {
		navigation.navigateToNode(nodeId);
	}
</script>

{#if visible}
	<div
		class="fixed z-50 rounded-lg border border-surface-200 bg-surface-0 p-3 shadow-lg dark:border-surface-700 dark:bg-surface-900"
		style="left: {x}px; top: {y}px;"
	>
		<button
			onclick={navigate}
			class="text-sm font-medium text-mycelium-600 hover:underline dark:text-mycelium-400"
		>
			{title}
		</button>
	</div>
{/if}
