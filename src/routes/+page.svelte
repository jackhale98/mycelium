<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { openVault, listFiles, listNodes } from '$lib/tauri/commands';
	import FolderBrowser from '$lib/components/common/FolderBrowser.svelte';

	let vaultPath = $state('');
	let isLoading = $state(false);
	let error = $state<string | null>(null);
	let autoOpening = $state(false);
	let showFolderBrowser = $state(false);

	onMount(() => {
		// Check for saved vault path and auto-open
		const saved = localStorage.getItem('mycelium-vault-path');
		if (saved && !vault.isOpen) {
			vaultPath = saved;
			autoOpen(saved);
		}
	});

	/** Restore iOS security-scoped bookmark access before opening vault */
	async function restoreIOSAccess(): Promise<void> {
		if (!isMobile()) return;
		try {
			const { invoke } = await import('@tauri-apps/api/core');
			const result = await invoke<{ path: string | null }>('plugin:folder-picker|restore_access');
			console.log('[Mycelium] Restored iOS access:', result?.path);
		} catch (e) {
			console.warn('[Mycelium] restore_access failed (non-fatal):', e);
		}
	}

	async function autoOpen(path: string) {
		autoOpening = true;
		try {
			// On iOS, restore security-scoped bookmark access before syncing
			await restoreIOSAccess();
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
			// Restore iOS security-scoped access before syncing
			await restoreIOSAccess();
			const syncResult = await openVault(vaultPath.trim());
			const files = await listFiles();
			const nodes = await listNodes();

			// Check if we actually indexed any files
			if (syncResult.total_files === 0 && isMobile()) {
				error = 'No .org files found. On iOS, the app may not have access to this folder. Try placing your vault in the Mycelium Documents folder (accessible via Files app → On My iPhone → Mycelium).';
				isLoading = false;
				return;
			}

			// Surface any walk errors for debugging
			if (syncResult.walk_errors?.length > 0) {
				console.warn('[Mycelium] Walk errors during sync:', syncResult.walk_errors);
			}

			vault.setVault(vaultPath.trim(), files, nodes, syncResult);
			localStorage.setItem('mycelium-vault-path', vaultPath.trim());
			window.location.href = '/vault';
		} catch (e) {
			error = String(e);
		} finally {
			isLoading = false;
		}
	}

	function isMobile(): boolean {
		if (typeof navigator === 'undefined') return false;
		return /iPhone|iPad|iPod|Android/i.test(navigator.userAgent);
	}

	async function handlePickFolder() {
		if (isMobile()) {
			// iOS/Android: use our custom folder picker plugin
			// This presents the native UIDocumentPickerViewController with UTType.folder
			try {
				const { invoke } = await import('@tauri-apps/api/core');
				const result = await invoke<{ path: string | null }>('plugin:folder-picker|pick_folder');
				if (result?.path) {
					vaultPath = result.path;
					return;
				}
			} catch (e) {
				console.warn('Folder picker plugin failed:', e);
			}

			// Fallback: file picker (limited to single file access)
			try {
				const { open } = await import('@tauri-apps/plugin-dialog');
				const file = await open({ filters: [{ name: 'Org files', extensions: ['org'] }], multiple: false });
				if (file) {
					let p = file as string;
					if (p.startsWith('file://')) p = decodeURIComponent(p.substring(7));
					const slash = p.lastIndexOf('/');
					if (slash > 0) vaultPath = p.substring(0, slash);
				}
			} catch { showFolderBrowser = true; }
			return;
		}

		// Desktop: native folder picker via dialog plugin
		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const selected = await open({ directory: true, multiple: false });
			if (selected) {
				let p = selected as string;
				if (p.startsWith('file://')) p = decodeURIComponent(p.substring(7));
				vaultPath = p;
			}
		} catch { showFolderBrowser = true; }
	}

	function handleFolderSelected(path: string) {
		vaultPath = path;
		showFolderBrowser = false;
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
					{isLoading ? 'Opening...' : 'Open Vault'}
				</button>

				{#if error}
					<p class="rounded-lg bg-red-50 p-3 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">
						{error}
					</p>
				{/if}
			</div>


			<!-- Info -->
			<p class="text-center text-xs text-surface-700 dark:text-surface-300">
				On iOS, place your org vault in Files → On My iPhone → Mycelium, then tap Browse to select a file from it.
				Sync with iCloud, Syncthing, or Working Copy.
			</p>
		</div>
	</div>
{/if}

<FolderBrowser open={showFolderBrowser} onselect={handleFolderSelected} onclose={() => (showFolderBrowser = false)} />
