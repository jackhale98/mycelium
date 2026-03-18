<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import type { Snippet } from 'svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';

	let { children }: { children: Snippet } = $props();

	onMount(() => {
		theme.applyTheme();

		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		const handler = () => {
			if (theme.mode === 'system') theme.applyTheme();
		};
		mq.addEventListener('change', handler);

		// Expose orgConfig to native iOS toolbar pickers
		(window as any).__myceliumOrgConfig = orgConfig;

		// Install native iOS keyboard toolbar
		setupNativeToolbar();

		return () => {
			mq.removeEventListener('change', handler);
			delete (window as any).__myceliumOrgConfig;
		};
	});

	async function setupNativeToolbar() {
		if (!/iPhone|iPad|iPod/i.test(navigator.userAgent)) return;
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			await invoke('plugin:folder-picker|setup_toolbar');
			console.log('[Mycelium] Native keyboard toolbar installed');
		} catch (e) {
			console.warn('[Mycelium] Native toolbar setup failed (non-fatal):', e);
		}
	}
</script>

<div class="h-screen w-screen overflow-hidden bg-surface-0 text-surface-900 dark:bg-surface-950 dark:text-surface-100">
	{@render children()}
</div>
