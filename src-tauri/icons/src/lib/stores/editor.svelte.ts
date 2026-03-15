class EditorStore {
	filePath = $state<string | null>(null);
	nodeId = $state<string | null>(null);
	content = $state('');
	originalContent = $state('');
	isSaving = $state(false);

	get isDirty() {
		return this.content !== this.originalContent;
	}

	get hasFile() {
		return this.filePath !== null;
	}

	openFile(filePath: string, content: string, nodeId?: string) {
		this.filePath = filePath;
		this.content = content;
		this.originalContent = content;
		this.nodeId = nodeId ?? null;
	}

	updateContent(content: string) {
		this.content = content;
	}

	markSaved() {
		this.originalContent = this.content;
		this.isSaving = false;
	}

	close() {
		this.filePath = null;
		this.nodeId = null;
		this.content = '';
		this.originalContent = '';
	}
}

export const editor = new EditorStore();
