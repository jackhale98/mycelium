import type { NodeRecord, BacklinkRecord, ForwardLink, GraphData, SearchResult, TagCount, HeadlineRecord } from '$lib/types/node';
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

/// Local-time formatters. The backend never derives dates itself, so anything
/// day-sensitive (daily notes, quick capture, org-roam filename prefixes) must be
/// formatted here from the user's own clock. `toISOString()` is UTC and would put
/// users west of UTC into tomorrow's note after their local afternoon.

function pad(n: number, width = 2): string {
	return String(n).padStart(width, '0');
}

/** The local calendar date as `YYYY-MM-DD`. Never UTC. */
export function localDate(now: Date = new Date()): string {
	return `${pad(now.getFullYear(), 4)}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** The local wall-clock time as `HH:MM`. Never UTC. */
export function localTime(now: Date = new Date()): string {
	return `${pad(now.getHours())}:${pad(now.getMinutes())}`;
}

/** The local date and time as the org-roam filename prefix `YYYYMMDDHHmmss`. Never UTC. */
export function localTimestamp(now: Date = new Date()): string {
	return (
		`${pad(now.getFullYear(), 4)}${pad(now.getMonth() + 1)}${pad(now.getDate())}` +
		`${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`
	);
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

export async function setTodoKeywords(keywords: string[]): Promise<void> {
	return invoke('set_todo_keywords', { keywords });
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
export interface FileMeta {
	content: string;
	hash: string;
}

export async function readFile(filePath: string): Promise<string> {
	return invoke('read_file', { filePath });
}

/** Read a file together with the hash to pass back to {@link saveFile}. */
export async function readFileMeta(filePath: string): Promise<FileMeta> {
	return invoke('read_file_meta', { filePath });
}

/**
 * Write a file and return the hash of the newly written content.
 * Pass the hash last read for that file as `expectedHash` to get a `CONFLICT:`
 * error instead of clobbering an edit made outside the app.
 */
export async function saveFile(
	filePath: string,
	content: string,
	expectedHash?: string
): Promise<string> {
	return invoke('save_file', { filePath, content, expectedHash });
}

/** `timestamp` is the local `YYYYMMDDHHmmss` from {@link localTimestamp}. */
export async function createFile(title: string, timestamp: string): Promise<string> {
	return invoke('create_file', { title, timestamp });
}

// Graph commands
export async function getGraphData(): Promise<GraphData> {
	return invoke('get_graph_data');
}

/** A slice of the graph plus the whole-vault totals, so the UI can say what it left out. */
export interface BoundedGraphData extends GraphData {
	total_nodes: number;
	total_links: number;
	returned_nodes: number;
	returned_links: number;
	truncated: boolean;
}

/** The `limit` most-connected nodes, with links already restricted to that set. */
export async function getGraphDataLimited(limit: number): Promise<BoundedGraphData> {
	return invoke('get_graph_data_limited', { limit });
}

// Daily notes commands
/**
 * `date` is the local `YYYY-MM-DD` from {@link localDate}. `timestamp` is the local
 * `YYYYMMDDHHmmss` used for the filename prefix when the note has to be created;
 * omit it to default to midnight on `date`.
 */
export async function getOrCreateDaily(date: string, timestamp?: string): Promise<NodeRecord> {
	return invoke('get_or_create_daily', { date, timestamp });
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
export async function getAgenda(): Promise<HeadlineRecord[]> {
	return invoke('get_agenda');
}

// Unlinked mentions
export async function getUnlinkedMentions(nodeId: string): Promise<SearchResult[]> {
	return invoke('get_unlinked_mentions', { nodeId });
}

// Quick capture
/**
 * Append `text` to the daily note for `localDate`, stamped with `localTime`.
 * Both come from {@link localDate} / {@link localTime} so the capture lands in the
 * day the user is actually in.
 */
export async function quickCapture(text: string, date: string, time: string): Promise<string> {
	return invoke('quick_capture', { text, localDate: date, localTime: time });
}

// Folder picker
export async function getDocumentsPath(): Promise<string> {
	return invoke('get_documents_path');
}

export interface DirEntry {
	name: string;
	path: string;
	is_dir: boolean;
	has_org_files: boolean;
}

export async function listSubdirectories(path: string): Promise<DirEntry[]> {
	return invoke('list_subdirectories', { path });
}
