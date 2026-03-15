<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import type { Snippet } from 'svelte';
	import { theme } from '$lib/stores/theme.svelte';

	let { children }: { children: Snippet } = $props();

	onMount(() => {
		theme.applyTheme();

		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		const handler = () => {
			if (theme.mode === 'system') theme.applyTheme();
		};
		mq.addEventListener('change', handler);
		return () => mq.removeEventListener('change', handler);
	});
</script>

<div class="h-screen w-screen overflow-hidden bg-surface-0 text-surface-900 dark:bg-surface-950 dark:text-surface-100">
	{@render children()}
</div>
