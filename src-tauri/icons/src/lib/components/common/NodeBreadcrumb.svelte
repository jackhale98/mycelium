<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';

	let { olp = [], title = '' }: { olp?: string[]; title?: string } = $props();
</script>

<nav class="flex items-center gap-1 text-xs text-surface-700 dark:text-surface-300">
	<button onclick={() => navigation.navigateToVault()} class="hover:text-mycelium-600">
		Vault
	</button>
	{#each olp as segment}
		<span>/</span>
		<span>{segment}</span>
	{/each}
	{#if title}
		<span>/</span>
		<span class="font-medium text-surface-900 dark:text-surface-100">{title}</span>
	{/if}
</nav>
