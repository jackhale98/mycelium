export interface FileRecord {
	file: string;
	title: string | null;
	hash: string;
	mtime: string;
}

/** An `:ID:` declared by more than one file — the last file indexed wins in the database. */
export interface IdCollision {
	id: string;
	existing_file: string;
	new_file: string;
	title: string | null;
}

export interface SyncResult {
	total_files: number;
	indexed: number;
	skipped: number;
	removed: number;
	walk_errors?: string[];
	broken_links?: number;
	id_collisions?: IdCollision[];
}
