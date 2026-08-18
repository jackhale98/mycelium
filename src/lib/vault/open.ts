/// Shared vault-opening path used by the picker and by the cold-relaunch restore
/// in /vault. Keeps the ordering guarantee that custom TODO keywords reach the
/// Rust parser before any indexing runs.

import { vault } from '$lib/stores/vault.svelte';
import { orgConfig } from '$lib/stores/orgconfig.svelte';
import { openVault, listFiles, listNodes } from '$lib/tauri/commands';
import type { SyncResult } from '$lib/types/vault';

export const VAULT_PATH_KEY = 'mycelium-vault-path';

export function isMobile(): boolean {
	if (typeof navigator === 'undefined') return false;
	return /iPhone|iPad|iPod|Android/i.test(navigator.userAgent);
}

export function isAndroid(): boolean {
	if (typeof navigator === 'undefined') return false;
	return /Android/i.test(navigator.userAgent);
}

export function savedVaultPath(): string | null {
	if (typeof localStorage === 'undefined') return null;
	return localStorage.getItem(VAULT_PATH_KEY);
}

/** Restore iOS security-scoped bookmark access before opening a vault. */
export async function restoreIOSAccess(): Promise<void> {
	if (!isMobile()) return;
	try {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke<{ path: string | null }>('plugin:folder-picker|restore_access');
	} catch (e) {
		console.warn('[Mycelium] restore_access failed (non-fatal):', e);
	}
}

export interface OpenVaultOutcome {
	sync: SyncResult;
	/** Set when the parser kept its default keywords, so headlines may index wrong. */
	keywordError: string | null;
}

/**
 * Open a vault and load it into the store. Pushes the configured TODO keywords to
 * the parser before indexing runs; if that push fails the vault still opens, but
 * the failure is reported so the caller can warn that a rebuild will be needed.
 */
export async function openVaultAt(path: string): Promise<OpenVaultOutcome> {
	await restoreIOSAccess();
	let keywordError: string | null = null;
	try {
		await orgConfig.syncToBackend();
	} catch (e) {
		keywordError = String(e);
	}
	const sync = await openVault(path);
	const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
	vault.setVault(path, files, nodes, sync);
	if (typeof localStorage !== 'undefined') localStorage.setItem(VAULT_PATH_KEY, path);
	return { sync, keywordError };
}
