class EditorStore {
	filePath = $state<string | null>(null);
	nodeId = $state<string | null>(null);
	content = $state('');
	originalContent = $state('');
	isSaving = $state(false);
	/** Hash of the content last read from or written to disk, for conflict detection. */
	savedHash = $state<string | null>(null);
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

	/** Write pending edits through the editor page's save handler, if one is set. */
	async flush(): Promise<void> {
		if (!this.saveHook || !this.isDirty || !this.filePath) return;
		try {
			await this.saveHook();
		} catch {
			// The caller is navigating away; the page surfaces the error itself.
		}
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
