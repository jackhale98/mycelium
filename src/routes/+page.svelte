<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { openVault, listFiles, listNodes } from '$lib/tauri/commands';

	let vaultPath = $state('');
	let isLoading = $state(false);
	let error = $state<string | null>(null);
	let autoOpening = $state(false);

	onMount(() => {
		// Check for saved vault path and auto-open
		const saved = localStorage.getItem('mycelium-vault-path');
		if (saved && !vault.isOpen) {
			vaultPath = saved;
			autoOpen(saved);
		}
	});

	async function autoOpen(path: string) {
		autoOpening = true;
		try {
			const syncResult = await openVault(path);
			const files = await listFiles();
			const nodes = await listNodes();
			vault.setVault(path, files, nodes, syncResult);
			localStorage.setItem('mycelium-vault-path', path);
			window.location.href = '/vault';
		} catch {
			// Saved path no longer valid — show the picker
			autoOpening = false;
		}
	}

	async function handleOpenVault() {
		if (!vaultPath.trim()) return;
		isLoading = true;
		error = null;
		try {
			const syncResult = await openVault(vaultPath.trim());
			const files = await listFiles();
			const nodes = await listNodes();
			vault.setVault(vaultPath.trim(), files, nodes, syncResult);
			localStorage.setItem('mycelium-vault-path', vaultPath.trim());
			window.location.href = '/vault';
		} catch (e) {
			error = String(e);
		} finally {
			isLoading = false;
		}
	}

	async function handlePickFolder() {
		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const selected = await open({ directory: true, multiple: false });
			if (selected) {
				// On iOS, the path may be a file:// URL — strip the scheme
				let path = selected as string;
				if (path.startsWith('file://')) {
					path = decodeURIComponent(path.replace('file://', ''));
				}
				// On iOS, if we got a temp Inbox path, show an error
				if (path.includes('/tmp/') || path.includes('-Inbox')) {
					error = 'On iOS, please use a folder from Files app (iCloud Drive, On My iPhone, or a synced folder). Temporary inbox paths cannot be used as a vault.';
					return;
				}
				vaultPath = path;
			}
		} catch (e) {
			// Folder picker not available — try to get the app's documents directory
			try {
				const { documentDir } = await import('@tauri-apps/api/path');
				const docPath = await documentDir();
				vaultPath = docPath;
			} catch {
				// Browser mode or path API not available
			}
		}
	}
</script>

{#if autoOpening}
	<div class="flex h-full flex-col items-center justify-center p-6">
		<img src="/logo.svg" alt="Mycelium" class="mb-4 h-16 w-16 rounded-2xl" />
		<p class="text-sm text-surface-700 dark:text-surface-300">Opening vault...</p>
	</div>
{:else}
	<div class="flex h-full flex-col items-center justify-center p-6">
		<div class="w-full max-w-md space-y-8">
			<!-- Logo & Title -->
			<div class="text-center">
				<img src="/logo.svg" alt="Mycelium" class="mx-auto mb-4 h-20 w-20 rounded-2xl" />
				<h1 class="text-3xl font-bold tracking-tight">Mycelium</h1>
				<p class="mt-2 text-surface-700 dark:text-surface-300">
					Open-source Org Roam knowledge base
				</p>
			</div>

			<!-- Vault Picker -->
			<div class="space-y-4">
				<div class="flex gap-2">
					<input
						type="text"
						bind:value={vaultPath}
						placeholder="Path to your org-roam vault..."
						class="flex-1 rounded-lg border border-surface-200 bg-surface-50 px-4 py-3 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-900"
						onkeydown={(e) => e.key === 'Enter' && handleOpenVault()}
					/>
					<button
						onclick={handlePickFolder}
						class="rounded-lg border border-surface-200 px-4 py-3 text-sm font-medium hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800"
					>
						Browse
					</button>
				</div>

				<button
					onclick={handleOpenVault}
					disabled={isLoading || !vaultPath.trim()}
					class="w-full rounded-lg bg-mycelium-600 px-4 py-3 text-sm font-semibold text-white transition-colors hover:bg-mycelium-700 disabled:opacity-50"
				>
					{isLoading ? 'Opening vault...' : 'Open Vault'}
				</button>

				{#if error}
					<p class="rounded-lg bg-red-50 p-3 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">
						{error}
					</p>
				{/if}
			</div>


			<!-- Info -->
			<p class="text-center text-xs text-surface-700 dark:text-surface-300">
				Select a directory containing your .org files.
			</p>
			<p class="text-center text-[10px] text-surface-700/60 dark:text-surface-300/60">
				On iOS, use a folder from iCloud Drive or On My iPhone in the Files app. Sync your vault with iCloud, Syncthing, or Working Copy.
			</p>
		</div>
	</div>
{/if}
