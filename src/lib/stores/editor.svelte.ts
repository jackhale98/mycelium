class EditorStore {
	filePath = $state<string | null>(null);
	nodeId = $state<string | null>(null);
	content = $state('');
	originalContent = $state('');
	isSaving = $state(false);
	/** Hash of the content last read from or written to disk, for conflict detection. */
	savedHash = $state<string | null>(null);
	/**
	 * Bumped every time content is loaded from disk, including a reload of the
	 * file already open, so the editor view knows to show it.
	 */
	revision = $state(0);
	/** Set by the editor page so navigation can flush pending edits before leaving. */
	saveHook: (() => Promise<void>) | null = null;

	get isDirty() {
		return this.content !== this.originalContent;
	}

	get hasFile() {
		return this.filePath !== null;
	}

	openFile(filePath: string, content: string, nodeId?: string, hash?: string | null) {
		this.filePath = filePath;
		this.content = content;
		this.originalContent = content;
		this.nodeId = nodeId ?? null;
		this.savedHash = hash ?? null;
		this.revision += 1;
	}

	updateContent(content: string) {
		this.content = content;
	}

	/**
	 * Mark the text that was actually written as the new baseline.
	 *
	 * Keystrokes typed while the save was in flight are not part of `saved`, so
	 * the buffer stays dirty and they are picked up by the next save instead of
	 * being silently dropped.
	 */
	markSaved(saved: string, hash?: string | null) {
		this.originalContent = saved;
		if (hash !== undefined) this.savedHash = hash;
		this.isSaving = false;
	}

	/**
	 * Write pending edits through the editor page's save handler, if one is set.
	 *
	 * Resolves `true` when nothing is left unsaved, `false` when edits are still
	 * only in memory — a conflict waiting on the user, or a write that failed.
	 * Navigation must not leave on `false`: every way out of a note discards the
	 * buffer, and the page is already showing why the save did not land.
	 */
	async flush(): Promise<boolean> {
		// Without a hook the editor page is gone and nothing here can save; holding
		// navigation would only strand the user on whatever page they are on.
		if (!this.saveHook || !this.isDirty || !this.filePath) return true;
		try {
			await this.saveHook();
		} catch {
			// The page surfaces the error itself.
		}
		return !this.isDirty;
	}

	close() {
		this.filePath = null;
		this.nodeId = null;
		this.content = '';
		this.originalContent = '';
		this.savedHash = null;
		this.saveHook = null;
	}
}

export const editor = new EditorStore();
