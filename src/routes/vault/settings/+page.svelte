<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { theme, type ThemeMode } from '$lib/stores/theme.svelte';
	import { syncVault, rebuildDatabase, listFiles, listNodes } from '$lib/tauri/commands';
	import MobileNav from '$lib/components/common/MobileNav.svelte';

	let isSyncing = $state(false);
	let isRebuilding = $state(false);
	let syncMessage = $state<string | null>(null);

	async function handleResync() {
		isSyncing = true;
		syncMessage = null;
		try {
			const result = await syncVault();
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);
			syncMessage = `Synced: ${result.indexed} indexed, ${result.skipped} unchanged, ${result.removed} removed`;
		} catch (e) {
			syncMessage = `Error: ${e}`;
		} finally {
			isSyncing = false;
		}
	}

	async function handleRebuild() {
		if (!confirm('This will drop all indexed data and rebuild the database from scratch. Continue?')) return;
		isRebuilding = true;
		syncMessage = null;
		try {
			const result = await rebuildDatabase();
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);
			syncMessage = `Rebuilt: ${result.indexed} files indexed from scratch`;
		} catch (e) {
			syncMessage = `Error: ${e}`;
		} finally {
			isRebuilding = false;
		}
	}

	function handleCloseVault() {
		vault.close();
		navigation.navigateHome();
	}

	const themeOptions: { value: ThemeMode; label: string; icon: string }[] = [
		{ value: 'light', label: 'Light', icon: '\u2600' },
		{ value: 'dark', label: 'Dark', icon: '\u263E' },
		{ value: 'system', label: 'System', icon: '\u2699' },
	];
</script>

<div class="flex h-full flex-col">
	<header
		class="flex h-14 shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700"
		style="padding-top: var(--safe-area-top)"
	>
		<button
			onclick={() => navigation.navigateToVault()}
			class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800"
			aria-label="Back"
		>
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
				<path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" />
			</svg>
		</button>
		<h1 class="text-lg font-semibold">Settings</h1>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		<div class="mx-auto max-w-lg space-y-6">
			<!-- Vault Info -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Vault
				</h2>
				<div class="space-y-2 text-sm">
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Path</span>
						<span class="max-w-[200px] truncate font-mono text-xs">{vault.path ?? 'None'}</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Files</span>
						<span class="font-semibold">{vault.fileCount}</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Nodes</span>
						<span class="font-semibold">{vault.nodeCount}</span>
					</div>
					{#if vault.lastSync}
						<div class="flex justify-between">
							<span class="text-surface-700 dark:text-surface-300">Last indexed</span>
							<span>{vault.lastSync.indexed} files</span>
						</div>
					{/if}
				</div>

				<div class="mt-4 space-y-2">
					<div class="flex gap-2">
						<button
							onclick={handleResync}
							disabled={isSyncing || isRebuilding}
							class="flex-1 rounded-lg border border-surface-200 px-3 py-2 text-sm font-medium hover:bg-surface-100 disabled:opacity-50 dark:border-surface-700 dark:hover:bg-surface-800"
						>
							{isSyncing ? 'Syncing...' : 'Re-sync Vault'}
						</button>
						<button
							onclick={handleCloseVault}
							class="rounded-lg border border-red-200 px-3 py-2 text-sm font-medium text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-400 dark:hover:bg-red-950"
						>
							Close Vault
						</button>
					</div>
					<button
						onclick={handleRebuild}
						disabled={isSyncing || isRebuilding}
						class="w-full rounded-lg border border-amber-200 px-3 py-2 text-sm font-medium text-amber-700 hover:bg-amber-50 disabled:opacity-50 dark:border-amber-800 dark:text-amber-400 dark:hover:bg-amber-950"
					>
						{isRebuilding ? 'Rebuilding...' : 'Rebuild Database'}
					</button>
					<p class="text-[11px] text-surface-700 dark:text-surface-300">
						Re-sync checks for changes. Rebuild drops all indexed data and re-indexes every file from scratch.
					</p>
				</div>

				{#if syncMessage}
					<p class="mt-2 text-xs text-surface-700 dark:text-surface-300">{syncMessage}</p>
				{/if}
			</section>

			<!-- Theme -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Appearance
				</h2>
				<div class="flex gap-2">
					{#each themeOptions as opt}
						<button
							onclick={() => theme.setMode(opt.value)}
							class="flex flex-1 flex-col items-center gap-1 rounded-lg border px-3 py-3 text-sm transition-colors {theme.mode === opt.value
								? 'border-mycelium-500 bg-mycelium-50 text-mycelium-700 dark:bg-mycelium-950 dark:text-mycelium-300'
								: 'border-surface-200 hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800'}"
						>
							<span class="text-lg">{opt.icon}</span>
							<span class="text-xs font-medium">{opt.label}</span>
						</button>
					{/each}
				</div>
			</section>

			<!-- About -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					About
				</h2>
				<div class="space-y-1 text-sm">
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Mycelium</span>
						<span class="font-mono text-xs">v0.1.0</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">License</span>
						<span class="text-xs">Apache 2.0</span>
					</div>
				</div>
				<p class="mt-3 text-xs text-surface-700 dark:text-surface-300">
					Open-source Org Roam mobile knowledge base. Built with Tauri, Svelte, and Rust.
				</p>
			</section>
		</div>
	</div>

	<MobileNav />
</div>
