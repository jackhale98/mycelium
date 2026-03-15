import type { NodeRecord, BacklinkRecord, ForwardLink, GraphData, SearchResult, TagCount } from '$lib/types/node';
import type { FileRecord, SyncResult } from '$lib/types/vault';
import { mockHandlers } from './mock';

function isTauri(): boolean {
	try {
		return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;
	} catch {
		return false;
	}
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (isTauri()) {
		const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
		return tauriInvoke(cmd, args) as Promise<T>;
	}
	// Browser preview: use mock handlers
	const handler = mockHandlers[cmd];
	if (handler) {
		return handler(args ?? {}) as T;
	}
	throw new Error(`No mock handler for command: ${cmd}`);
}

// Vault commands
export async function openVault(path: string): Promise<SyncResult> {
	return invoke('open_vault', { path });
}

export async function listFiles(): Promise<FileRecord[]> {
	return invoke('list_files');
}

export async function syncVault(): Promise<SyncResult> {
	return invoke('sync_vault');
}

export async function rebuildDatabase(): Promise<SyncResult> {
	return invoke('rebuild_database');
}

export async function checkVaultChanges(): Promise<boolean> {
	return invoke('check_vault_changes');
}

// Node commands
export async function getNode(id: string): Promise<NodeRecord | null> {
	return invoke('get_node', { id });
}

export async function listNodes(): Promise<NodeRecord[]> {
	return invoke('list_nodes');
}

export async function getBacklinks(nodeId: string): Promise<BacklinkRecord[]> {
	return invoke('get_backlinks', { nodeId });
}

export async function searchNodes(query: string): Promise<NodeRecord[]> {
	return invoke('search_nodes', { query });
}

export async function searchFull(query: string): Promise<SearchResult[]> {
	return invoke('search_full', { query });
}

// Editor commands
export async function readFile(filePath: string): Promise<string> {
	return invoke('read_file', { filePath });
}

export async function saveFile(filePath: string, content: string): Promise<void> {
	return invoke('save_file', { filePath, content });
}

export async function createFile(title: string): Promise<string> {
	return invoke('create_file', { title });
}

// Graph commands
export async function getGraphData(): Promise<GraphData> {
	return invoke('get_graph_data');
}

// Daily notes commands
export async function getOrCreateDaily(date: string): Promise<NodeRecord> {
	return invoke('get_or_create_daily', { date });
}

export async function listDailyNotes(): Promise<NodeRecord[]> {
	return invoke('list_daily_notes');
}

// Tag commands
export async function getAllTags(): Promise<TagCount[]> {
	return invoke('get_all_tags');
}

export async function getNodesByTag(tag: string): Promise<NodeRecord[]> {
	return invoke('get_nodes_by_tag', { tag });
}

// Forward links
export async function getForwardLinks(nodeId: string): Promise<ForwardLink[]> {
	return invoke('get_forward_links', { nodeId });
}

// Export
export async function exportMarkdown(filePath: string): Promise<string> {
	return invoke('export_markdown', { filePath });
}

export async function exportHtml(filePath: string): Promise<string> {
	return invoke('export_html', { filePath });
}

// Node refactoring
export async function renameNode(nodeId: string, newTitle: string): Promise<void> {
	return invoke('rename_node', { nodeId, newTitle });
}

// Image import
export async function importImage(sourcePath: string): Promise<string> {
	return invoke('import_image', { sourcePath });
}

// Agenda
export async function getAgenda(): Promise<NodeRecord[]> {
	return invoke('get_agenda');
}

// Unlinked mentions
export async function getUnlinkedMentions(nodeId: string): Promise<SearchResult[]> {
	return invoke('get_unlinked_mentions', { nodeId });
}

// Quick capture
export async function quickCapture(text: string): Promise<string> {
	return invoke('quick_capture', { text });
}
