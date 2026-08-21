<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { isAndroid, isMobile, openVaultAt, savedVaultPath } from '$lib/vault/open';
	import FolderBrowser from '$lib/components/common/FolderBrowser.svelte';
	import type { IdCollision } from '$lib/types/vault';

	let vaultPath = $state('');
	let isLoading = $state(false);
	let error = $state<string | null>(null);
	let warning = $state<string | null>(null);
	let autoOpening = $state(false);
	let showFolderBrowser = $state(false);

	onMount(() => {
		// Check for saved vault path and auto-open
		const saved = savedVaultPath();
		if (saved && !vault.isOpen) {
			vaultPath = saved;
			autoOpen(saved);
		}
	});

	async function autoOpen(path: string) {
		autoOpening = true;
		error = null;
		try {
			const { sync, keywordError } = await openVaultAt(path);
			if (sync.id_collisions?.length) {
				console.warn('[Mycelium] Duplicate :ID: values found during sync:', sync.id_collisions);
			}
			if (keywordError) {
				warning = keywordSyncWarning(keywordError);
				autoOpening = false;
				return;
			}
			continueToVault();
		} catch (e) {
			// Saved path no longer valid — show the picker, and say why
			error = `Could not reopen ${path}: ${e}`;
			autoOpening = false;
		}
	}

	async function handleOpenVault() {
		if (!vaultPath.trim()) return;
		isLoading = true;
		error = null;
		warning = null;
		try {
			const path = vaultPath.trim();
			const { sync, keywordError } = await openVaultAt(path);

			// Check if we actually indexed any files
			if (sync.total_files === 0 && isMobile()) {
				error = 'No .org files found. On iOS, the app may not have access to this folder. Try placing your vault in the Mycelium Documents folder (accessible via Files app → On My iPhone → Mycelium).';
				isLoading = false;
				return;
			}

			// Surface any walk errors for debugging
			if ((sync.walk_errors?.length ?? 0) > 0) {
				console.warn('[Mycelium] Walk errors during sync:', sync.walk_errors);
			}
			if (sync.broken_links && sync.broken_links > 0) {
				console.warn(`[Mycelium] ${sync.broken_links} broken link(s) removed (source node no longer exists)`);
			}

			const collisions = sync.id_collisions ?? [];
			if (collisions.length > 0) {
				console.warn('[Mycelium] Duplicate :ID: values found during sync:', collisions);
			}

			const warnings: string[] = [];
			if (keywordError) warnings.push(keywordSyncWarning(keywordError));
			if (collisions.length > 0) warnings.push(idCollisionWarning(collisions));
			if (warnings.length > 0) {
				warning = warnings.join('\n\n');
				return;
			}
			continueToVault();
		} catch (e) {
			error = String(e);
		} finally {
			isLoading = false;
		}
	}

	function continueToVault() {
		window.location.href = '/vault';
	}

	function keywordSyncWarning(detail: string): string {
		return `Your custom TODO keywords could not be applied before indexing (${detail}). Rebuild the database from Settings once the app is working.`;
	}

	function idCollisionWarning(collisions: IdCollision[]): string {
		const first = collisions[0];
		const a = first.existing_file.split('/').pop() ?? first.existing_file;
		const b = first.new_file.split('/').pop() ?? first.new_file;
		const extra = collisions.length - 1;
		const rest = extra > 0 ? ` ${extra} other ID${extra === 1 ? ' is' : 's are'} shared too.` : '';
		return `${a} and ${b} both use the ID ${first.id}, so only one of them is showing. org-roam needs every ID to be unique — open one and give it a new :ID:.${rest}`;
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

			// Android returns a Storage Access Framework tree rather than a path, and
			// the branch above has already taken it. Reaching here means the user
			// dismissed the picker, so there is nothing to report.
			if (isAndroid()) return;

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

				{#if warning}
					<div class="rounded-lg bg-amber-50 p-3 dark:bg-amber-950">
						<p class="whitespace-pre-line text-sm text-amber-800 dark:text-amber-300">{warning}</p>
						<button
							onclick={continueToVault}
							class="mt-2 rounded-lg border border-amber-300 px-3 py-1.5 text-xs font-medium text-amber-800 hover:bg-amber-100 dark:border-amber-800 dark:text-amber-300 dark:hover:bg-amber-900"
						>
							Continue to vault
						</button>
					</div>
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
