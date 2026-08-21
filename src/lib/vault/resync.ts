import { checkVaultChanges, listFiles, listNodes, syncVault } from '$lib/tauri/commands';
import { vault } from '$lib/stores/vault.svelte';
import type { SyncResult } from '$lib/types/vault';

let inFlight: Promise<SyncResult | null> | null = null;

async function run(): Promise<SyncResult | null> {
	if (!vault.isOpen) return null;
	if (!(await checkVaultChanges())) return null;

	const result = await syncVault();
	const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
	vault.updateFiles(files);
	vault.updateNodes(nodes);
	return result;
}

/**
 * Re-index the vault if anything changed on disk, and refresh the stores.
 *
 * Anything can edit the vault while the app is backgrounded — a git client
 * syncing the folder, another device, a desktop Emacs. The file watcher covers
 * that while the app is running, but it cannot see changes made while the app
 * was suspended, and on iOS it may not be running at all.
 *
 * Concurrent callers share one run: the layout fires this on every foreground
 * and the vault list fires it on focus, and those overlap.
 *
 * Resolves to the sync result, or `null` when no vault is open or nothing
 * changed. Rejects only if the sync itself failed — callers decide how loud to
 * be about that.
 */
export function resyncIfChanged(): Promise<SyncResult | null> {
	if (!inFlight) {
		inFlight = run().finally(() => {
			inFlight = null;
		});
	}
	return inFlight;
}
