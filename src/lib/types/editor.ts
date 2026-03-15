export interface EditorState {
	filePath: string | null;
	content: string;
	isDirty: boolean;
	isSaving: boolean;
}
