<script lang="ts">
	import { onMount } from 'svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';

	let container: HTMLDivElement;
	let fallbackMode = $state(false);
	let view: any;
	let currentFilePath: string | null = null;

	onMount(() => {
		initEditor();
		return () => { view?.destroy(); };
	});

	async function initEditor() {
		try {
			const cmView = await import('@codemirror/view');
			const cmState = await import('@codemirror/state');
			const cmCommands = await import('@codemirror/commands');
			const cmLanguage = await import('@codemirror/language');
			const cmSearch = await import('@codemirror/search');

			let extensions: any[] = [
				cmView.lineNumbers(),
				cmView.highlightActiveLineGutter(),
				cmCommands.history(),
				cmView.drawSelection(),
				cmView.highlightActiveLine(),
				cmLanguage.bracketMatching(),
				cmSearch.highlightSelectionMatches(),
			];

			// Try loading org-mode extensions (non-fatal if they fail)
			try {
				const { orgHighlighting } = await import('$lib/codemirror/lang-org/highlighting');
				extensions.push(...orgHighlighting());
			} catch (e) { console.warn('Org highlighting unavailable:', e); }

			try {
				const { orgFolding } = await import('$lib/codemirror/extensions/folding');
				extensions.push(...orgFolding());
			} catch (e) { console.warn('Org folding unavailable:', e); }

			try {
				const { orgLivePreview } = await import('$lib/codemirror/extensions/live-preview');
				extensions.push(orgLivePreview());
			} catch (e) { console.warn('Live preview unavailable:', e); }

			try {
				const { orgLinkClickExtension } = await import('$lib/codemirror/extensions/link-click');
				extensions.push(orgLinkClickExtension((id: string) => navigation.navigateToNode(id)));
			} catch (e) { console.warn('Link click unavailable:', e); }

			try {
				const { orgCheckboxToggle } = await import('$lib/codemirror/extensions/checkbox-toggle');
				extensions.push(orgCheckboxToggle());
			} catch (e) { console.warn('Checkbox toggle unavailable:', e); }

			try {
				const { orgLinkAutocomplete } = await import('$lib/codemirror/extensions/link-autocomplete');
				extensions.push(orgLinkAutocomplete({ getNodes: () => vault.nodes }));
			} catch (e) { console.warn('Link autocomplete unavailable:', e); }

			extensions.push(
				cmView.keymap.of([
					...cmCommands.defaultKeymap,
					...cmCommands.historyKeymap,
					...cmSearch.searchKeymap,
					{
						key: 'Mod-s',
						run: () => {
							document.dispatchEvent(new CustomEvent('org-editor-save'));
							return true;
						},
					},
				]),
				cmView.EditorView.updateListener.of((update: any) => {
					if (update.docChanged) {
						editor.updateContent(update.state.doc.toString());
					}
				}),
				cmView.EditorView.theme(
				document.documentElement.classList.contains('dark')
					? {
						'&': { fontSize: '14px', fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace', backgroundColor: '#0f172a', color: '#e2e8f0' },
						'.cm-content': { padding: '12px 0', minHeight: '400px', caretColor: '#4ade80' },
						'.cm-line': { padding: '1px 12px' },
						'.cm-gutters': { backgroundColor: '#1e293b', borderRight: '1px solid #334155', color: '#64748b' },
						'.cm-activeLineGutter': { backgroundColor: '#1e293b', color: '#94a3b8' },
						'.cm-activeLine': { backgroundColor: '#1e293b' },
						'&.cm-focused .cm-cursor': { borderLeftColor: '#4ade80', borderLeftWidth: '2px' },
						'&.cm-focused .cm-selectionBackground, ::selection': { backgroundColor: '#052e16 !important' },
						'.cm-foldGutter .cm-gutterElement': { padding: '0 4px' },
						'.cm-tooltip-autocomplete': { border: '1px solid #334155', borderRadius: '8px', boxShadow: '0 4px 12px rgba(0,0,0,0.3)', fontSize: '13px', backgroundColor: '#1e293b', color: '#e2e8f0' },
						'.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: '#052e16', color: '#4ade80' },
					}
					: {
						'&': { fontSize: '14px', fontFamily: 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace', backgroundColor: '#ffffff', color: '#1e293b' },
						'.cm-content': { padding: '12px 0', minHeight: '400px', caretColor: '#16a34a' },
						'.cm-line': { padding: '1px 12px' },
						'.cm-gutters': { backgroundColor: '#f8fafc', borderRight: '1px solid #e2e8f0', color: '#94a3b8' },
						'.cm-activeLineGutter': { backgroundColor: '#f1f5f9', color: '#64748b' },
						'.cm-activeLine': { backgroundColor: '#f8fafc' },
						'&.cm-focused .cm-cursor': { borderLeftColor: '#16a34a', borderLeftWidth: '2px' },
						'&.cm-focused .cm-selectionBackground, ::selection': { backgroundColor: '#dcfce7 !important' },
						'.cm-foldGutter .cm-gutterElement': { padding: '0 4px' },
						'.cm-tooltip-autocomplete': { border: '1px solid #e2e8f0', borderRadius: '8px', boxShadow: '0 4px 12px rgba(0,0,0,0.1)', fontSize: '13px', backgroundColor: '#ffffff', color: '#1e293b' },
						'.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: '#dcfce7', color: '#15803d' },
					}
			),
				cmView.EditorView.lineWrapping,
			);

			const startState = cmState.EditorState.create({
				doc: editor.content,
				extensions,
			});

			view = new cmView.EditorView({
				state: startState,
				parent: container,
			});
			currentFilePath = editor.filePath;

		} catch (e) {
			console.error('CodeMirror failed to initialize, using fallback textarea:', e);
			fallbackMode = true;
		}
	}

	// Reload content when a different file is opened
	$effect(() => {
		if (view && editor.filePath !== currentFilePath) {
			view.dispatch({
				changes: { from: 0, to: view.state.doc.length, insert: editor.content },
			});
			currentFilePath = editor.filePath;
		}
	});

	export function insertAtCursor(text: string) {
		if (!view) return;
		view.focus();
		const { from } = view.state.selection.main;
		view.dispatch({ changes: { from, insert: text } });
	}

	export function wrapSelection(before: string, after: string) {
		if (!view) return;
		view.focus();
		const { from, to } = view.state.selection.main;
		const selected = view.state.sliceDoc(from, to);
		view.dispatch({
			changes: { from, to, insert: `${before}${selected}${after}` },
			selection: { anchor: from + before.length, head: to + before.length },
		});
	}

	export function insertLinePrefix(prefix: string) {
		if (!view) return;
		view.focus();
		const { from } = view.state.selection.main;
		const line = view.state.doc.lineAt(from);
		view.dispatch({ changes: { from: line.from, insert: prefix } });
	}

	export function insertHeadingWithId(level: number = 2) {
		if (!view) return;
		view.focus();
		const { from } = view.state.selection.main;
		const line = view.state.doc.lineAt(from);
		const id = crypto.randomUUID();
		const stars = '*'.repeat(level);
		const text = `\n${stars} \n:PROPERTIES:\n:ID: ${id}\n:END:\n`;
		view.dispatch({
			changes: { from: line.to, insert: text },
			selection: { anchor: line.to + 1 + level + 1 },
		});
	}

	/** Insert a plain heading (no :ID:) at the same level as the nearest heading */
	export function insertHeading() {
		if (!view) return;
		view.focus();
		const { from } = view.state.selection.main;
		const line = view.state.doc.lineAt(from);
		// Find the nearest heading above to detect level
		let level = 2;
		for (let i = line.number; i >= 1; i--) {
			const l = view.state.doc.line(i).text;
			const m = l.match(/^(\*+)\s/);
			if (m) { level = m[1].length; break; }
		}
		const stars = '*'.repeat(level);
		const text = `\n${stars} `;
		view.dispatch({
			changes: { from: line.to, insert: text },
			selection: { anchor: line.to + text.length },
		});
	}

	/** Get the current cursor position (byte offset in document) */
	export function getCursorPos(): number {
		if (!view) return 0;
		return view.state.selection.main.from;
	}

	/** Replace the entire document content (used when modifying outside CM) */
	export function replaceContent(newContent: string) {
		if (!view) return;
		view.dispatch({
			changes: { from: 0, to: view.state.doc.length, insert: newContent },
		});
	}

	function handleFallbackInput(e: Event) {
		editor.updateContent((e.target as HTMLTextAreaElement).value);
	}
</script>

{#if fallbackMode}
	<textarea
		value={editor.content}
		oninput={handleFallbackInput}
		class="min-h-[400px] w-full rounded-lg border p-4 font-mono text-sm leading-relaxed focus:outline-none"
		style={document.documentElement.classList.contains('dark') ? 'background:#0f172a;color:#e2e8f0;border-color:#334155' : 'background:#ffffff;color:#1e293b;border-color:#e2e8f0'}
		spellcheck="false"
	></textarea>
{:else}
	<div
		bind:this={container}
		class="org-editor min-h-[400px] rounded-lg border"
		style={document.documentElement.classList.contains('dark') ? 'background:#0f172a;border-color:#334155' : 'background:#ffffff;border-color:#e2e8f0'}
	></div>
{/if}
