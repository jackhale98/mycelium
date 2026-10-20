<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import type { Snippet } from 'svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';
	import { resyncIfChanged } from '$lib/vault/resync';
	import { installBackHandler } from '$lib/vault/back';
	import { installEditorFocusReporting, isEditorTarget } from '$lib/native/editorFocus';
	import { installEditingClass, installKeyboardViewport } from '$lib/native/keyboardViewport';

	let { children }: { children: Snippet } = $props();

	onMount(() => {
		theme.applyTheme();

		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		const handler = () => {
			if (theme.mode === 'system') theme.applyTheme();
		};
		mq.addEventListener('change', handler);

		// Expose orgConfig and vault tags to native toolbar pickers (iOS + Android)
		(window as any).__myceliumOrgConfig = orgConfig;
		loadVaultTags();

		// Install native keyboard toolbar (iOS + Android), shown only while the
		// note editor has focus
		const removeFocusReporting = setupNativeToolbar();
		const removeKeyboardViewport = installKeyboardViewport();
		const removeEditingClass = installEditingClass(isEditorTarget);

		// Answer the Android system back press. Harmless elsewhere: nothing calls
		// the hook on platforms that have no system back.
		const removeBackHandler = installBackHandler();

		// Pick up edits made while the app was in the background — a git client
		// syncing the folder, another device, a desktop Emacs. `visibilitychange`
		// is the signal iOS actually delivers when returning from another app;
		// `focus` covers desktop window switching, and the two de-duplicate.
		const onForeground = () => {
			if (document.visibilityState !== 'visible') return;
			resyncIfChanged().catch((e) => {
				console.warn('[Mycelium] background re-sync failed:', e);
			});
		};
		document.addEventListener('visibilitychange', onForeground);
		window.addEventListener('focus', onForeground);

		return () => {
			removeFocusReporting();
			removeKeyboardViewport();
			removeEditingClass();
			removeBackHandler();
			document.removeEventListener('visibilitychange', onForeground);
			window.removeEventListener('focus', onForeground);
			mq.removeEventListener('change', handler);
			delete (window as any).__myceliumOrgConfig;
			delete (window as any).__myceliumVaultTags;
		};
	});

	async function loadVaultTags() {
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const tags = await invoke('get_all_tags');
			(window as any).__myceliumVaultTags = tags;
		} catch { /* not fatal — tags just won't appear in picker */ }
	}

	function setupNativeToolbar(): () => void {
		if (!/iPhone|iPad|iPod|Android/i.test(navigator.userAgent)) return () => {};
		// Listen before installing, so a field focused while the toolbar is
		// being set up is still reported.
		const remove = installEditorFocusReporting();
		void installNativeToolbar();
		return remove;
	}

	async function installNativeToolbar() {
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			await invoke('plugin:folder-picker|setup_toolbar');
			console.log('[Mycelium] Native keyboard toolbar installed');
		} catch (e) {
			console.warn('[Mycelium] Native toolbar setup failed (non-fatal):', e);
		}
	}
</script>

<div style="height: var(--app-height, 100vh)" class="w-screen overflow-hidden bg-surface-0 text-surface-900 dark:bg-surface-950 dark:text-surface-100">
	{@render children()}
</div>
