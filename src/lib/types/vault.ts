export interface FileRecord {
	file: string;
	title: string | null;
	hash: string;
	mtime: string;
}

export interface SyncResult {
	total_files: number;
	indexed: number;
	skipped: number;
	removed: number;
	walk_errors?: string[];
	broken_links?: number;
}
