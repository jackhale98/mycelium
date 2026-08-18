import { vault } from '$lib/stores/vault.svelte';
import { listFiles, listNodes } from './commands';

type UnlistenFn = () => void;

/**
 * Refresh the vault lists whenever the backend reports a database change.
 * `onError` receives the failure message when the refresh itself fails, so the
 * page can tell the user its lists are stale instead of showing nothing.
 */
export async function onDbUpdated(
	callback?: () => void,
	onError?: (message: string) => void
): Promise<UnlistenFn> {
	try {
		const { listen } = await import('@tauri-apps/api/event');
		return listen('db-updated', async () => {
			try {
				const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
				vault.updateFiles(files);
				vault.updateNodes(nodes);
			} catch (e) {
				if (vault.isOpen) onError?.(String(e));
			}
			callback?.();
		});
	} catch (e) {
		// Not running in Tauri — return no-op unlisten
		console.warn('[Mycelium] db-updated listener unavailable:', e);
		return () => {};
	}
}
