import type { FileRecord, SyncResult } from '$lib/types/vault';
import type { NodeRecord } from '$lib/types/node';

class VaultStore {
	path = $state<string | null>(null);
	files = $state<FileRecord[]>([]);
	nodes = $state<NodeRecord[]>([]);
	isLoading = $state(false);
	lastSync = $state<SyncResult | null>(null);

	constructor() {
		// Restore from sessionStorage on page load
		if (typeof sessionStorage !== 'undefined') {
			try {
				const saved = sessionStorage.getItem('mycelium-vault');
				if (saved) {
					const data = JSON.parse(saved);
					this.path = data.path;
					this.files = data.files ?? [];
					this.nodes = data.nodes ?? [];
					this.lastSync = data.lastSync;
				}
			} catch {
				// ignore
			}
		}
	}

	get isOpen() {
		return this.path !== null;
	}

	get nodeCount() {
		return this.nodes.length;
	}

	get fileCount() {
		return this.files.length;
	}

	setVault(path: string, files: FileRecord[], nodes: NodeRecord[], syncResult: SyncResult) {
		this.path = path;
		this.files = files;
		this.nodes = nodes;
		this.lastSync = syncResult;
		this.persist();
	}

	updateFiles(files: FileRecord[]) {
		this.files = files;
		this.persist();
	}

	updateNodes(nodes: NodeRecord[]) {
		this.nodes = nodes;
		this.persist();
	}

	close() {
		this.path = null;
		this.files = [];
		this.nodes = [];
		this.lastSync = null;
		if (typeof sessionStorage !== 'undefined') {
			sessionStorage.removeItem('mycelium-vault');
		}
	}

	private persist() {
		if (typeof sessionStorage !== 'undefined') {
			try {
				sessionStorage.setItem(
					'mycelium-vault',
					JSON.stringify({
						path: this.path,
						files: this.files,
						nodes: this.nodes,
						lastSync: this.lastSync,
					})
				);
			} catch {
				// Storage full or unavailable
			}
		}
	}
}

export const vault = new VaultStore();
