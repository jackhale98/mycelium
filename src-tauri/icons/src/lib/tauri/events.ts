import { vault } from '$lib/stores/vault.svelte';
import { listFiles, listNodes } from './commands';

type UnlistenFn = () => void;

export async function onDbUpdated(callback?: () => void): Promise<UnlistenFn> {
	try {
		const { listen } = await import('@tauri-apps/api/event');
		return listen('db-updated', async () => {
			try {
				const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
				vault.updateFiles(files);
				vault.updateNodes(nodes);
			} catch {
				// Vault may have been closed
			}
			callback?.();
		});
	} catch {
		// Not running in Tauri — return no-op unlisten
		return () => {};
	}
}

export async function onVaultChanged(callback: () => void): Promise<UnlistenFn> {
	try {
		const { listen } = await import('@tauri-apps/api/event');
		return listen('vault-changed', () => callback());
	} catch {
		return () => {};
	}
}
