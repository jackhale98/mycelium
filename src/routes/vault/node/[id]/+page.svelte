<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import {
		getNode, getBacklinks, getForwardLinks, getUnlinkedMentions,
		readFileMeta, saveFile, listNodes, createFile, localTimestamp, localDate, localTime,
		renameNode, importImage,
	} from '$lib/tauri/commands';
	import RenderedView from '$lib/components/editor/RenderedView.svelte';
	import OrgEditor from '$lib/components/editor/OrgEditor.svelte';
	import EditorToolbar from '$lib/components/editor/EditorToolbar.svelte';
	import BacklinkPanel from '$lib/components/sidebar/BacklinkPanel.svelte';
	import OutlinePanel from '$lib/components/sidebar/OutlinePanel.svelte';
	import QuickSwitcher from '$lib/components/common/QuickSwitcher.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { NodeRecord, BacklinkRecord, ForwardLink, SearchResult } from '$lib/types/node';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';
	import { prefs } from '$lib/stores/prefs.svelte';
	import {
		applyRepeaterOnDone, getPlanning, getTodoKeyword, isDoneKeyword, nextTodoKeyword,
		setClosed, setPlanningDate, setPriority, setTodoKeyword, removePlanning,
		toggleFiletag, getFiletags as readFiletags, repeatKeyword,
	} from '$lib/org';
	import type { KeywordConfig, PlanningKind } from '$lib/org';

	let node = $state<NodeRecord | null>(null);
	let backlinks = $state<BacklinkRecord[]>([]);
	let forwardLinks = $state<ForwardLink[]>([]);
	let unlinkedMentions = $state<SearchResult[]>([]);
	let error = $state<string | null>(null);
	let autoSaveTimer: ReturnType<typeof setTimeout>;
	let showLinkSwitcher = $state(false);
	let showMenu = $state(false);
	let editorComponent: OrgEditor;
	let nodeId = $state<string | null>(null);
	let showSource = $state(false);
	let showRename = $state(false);
	let renameTitle = $state('');
	let showLinks = $state(false);
	let showFind = $state(false);
	let findQuery = $state('');
	let findCount = $state(0);
	let findCurrent = $state(0);
	let contentEl: HTMLDivElement;
	let conflict = $state<string | null>(null);

	// Auto-save debounce. Skipped in manual mode, which exists so a vault under
	// version control is not rewritten every 1.5s while you type. The flushes on
	// navigation and on `pagehide` below still run in both modes: the OS can end
	// the process at any moment, and losing what someone typed is worse than a
	// write they did not ask for.
	$effect(() => {
		const _ = editor.content;
		if (!prefs.autoSaves) return;
		if (editor.isDirty && editor.filePath) {
			clearTimeout(autoSaveTimer);
			autoSaveTimer = setTimeout(() => handleSave(), 1500);
		}
	});

	onMount(() => {
		const pathParts = window.location.pathname.split('/');
		const id = pathParts[pathParts.length - 1];
		if (id) { nodeId = id; loadNode(id); }

		const onSave = () => handleSave();
		document.addEventListener('org-editor-save', onSave);
		editor.saveHook = handleSave;
		// iOS/Android can kill a backgrounded app outright, so flush on the way out.
		const onHide = () => { if (editor.isDirty) { clearTimeout(autoSaveTimer); void handleSave(); } };
		const onVisibility = () => { if (document.visibilityState === 'hidden') onHide(); };
		window.addEventListener('pagehide', onHide);
		document.addEventListener('visibilitychange', onVisibility);
		const onKey = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === 'e') { e.preventDefault(); showSource = !showSource; if (showFind) closeFind(); }
			if ((e.metaKey || e.ctrlKey) && e.key === 'k' && showSource) { e.preventDefault(); showLinkSwitcher = true; }
			if ((e.metaKey || e.ctrlKey) && e.key === 'f' && !showSource) { e.preventDefault(); showFind = true; }
		};
		document.addEventListener('keydown', onKey);

		// Register native iOS keyboard toolbar bridge
		// The native toolbar calls these functions via evaluateJavaScript
		(window as any).__myceliumToolbar = {
			link: () => onLink(),
			heading: () => onHeading(),
			headingLevel: (lvl: number) => { editorComponent?.insertHeadingAt(lvl); },
			makeNode: () => makeHeadingIntoNode(),
			todo: () => cycleTodo(),
			todoSet: (kw: string | null) => onTodo(kw),
			priority: () => onPriority('A'),
			prioritySet: (p: string | null) => onPriority(p),
			deadline: () => onDeadline(),
			deadlineSet: (ts: string | null) => setDeadline(ts),
			scheduled: () => onScheduled(),
			scheduledSet: (ts: string | null) => setScheduled(ts),
			bold: () => onBold(),
			italic: () => onItalic(),
			underline: () => onUnderline(),
			strike: () => onStrike(),
			code: () => onCode(),
			verbatim: () => onVerbatim(),
			list: () => onList(),
			checkbox: () => onCheckbox(),
			table: () => onTable(3, 3),
			tableSize: (rows: number, cols: number) => onTable(rows, cols),
			srcblock: () => onSrcBlock(),
			quote: () => onQuote(),
			timestamp: () => onTimestamp(),
			tag: () => onTag(),
			tagSet: (tag: string) => toggleTag(tag),
			/** Return current filetags as JSON for native picker */
			getFiletags: (): string => JSON.stringify(readFiletags(splitLines(editor.content))),
			/** Return existing date string for pre-selection in native date picker */
			getExisting: (type: string): string => {
				const src = splitLines(editor.content);
				const idx = findNearestHeadlineIdx(src);
				if (idx === -1) return '';
				const planning = getPlanning(src, idx);
				const ts = type === 'deadline' ? planning.DEADLINE : planning.SCHEDULED;
				return ts?.date ?? '';
			},
		};

		return () => {
			document.removeEventListener('org-editor-save', onSave);
			document.removeEventListener('keydown', onKey);
			window.removeEventListener('pagehide', onHide);
			document.removeEventListener('visibilitychange', onVisibility);
			if (editor.saveHook === handleSave) editor.saveHook = null;
			delete (window as any).__myceliumToolbar;
			clearTimeout(autoSaveTimer);
		};
	});

	async function loadNode(id: string) {
		error = null;
		try {
			node = await getNode(id);
			if (!node) { error = 'Node not found'; return; }
			const file = await readFileMeta(node.file);
			editor.openFile(node.file, file.content, id, file.hash);
			conflict = null;
			const [bl, fl, um] = await Promise.all([getBacklinks(id), getForwardLinks(id), getUnlinkedMentions(id)]);
			backlinks = bl; forwardLinks = fl; unlinkedMentions = um;
		} catch (e) { error = String(e); }
	}

	async function handleSave() {
		if (!editor.filePath || !editor.isDirty || conflict) return;
		// Snapshot what we send: anything typed while the write is in flight must
		// stay dirty rather than being marked saved.
		const snapshot = editor.content;
		editor.isSaving = true;
		try {
			const hash = await saveFile(editor.filePath, snapshot, editor.savedHash ?? undefined);
			editor.markSaved(snapshot, hash);
			if (nodeId) {
				const [bl, fl] = await Promise.all([getBacklinks(nodeId), getForwardLinks(nodeId)]);
				backlinks = bl; forwardLinks = fl;
			}
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) {
			editor.isSaving = false;
			const message = String(e);
			if (message.includes('CONFLICT:')) {
				clearTimeout(autoSaveTimer);
				conflict = message.replace(/^.*CONFLICT:\s*/, '');
			} else {
				error = message;
			}
		}
	}

	/** Discard our buffer and take what is on disk now. */
	async function reloadFromDisk() {
		if (!nodeId) return;
		conflict = null;
		await loadNode(nodeId);
	}

	/** Keep our buffer and overwrite whatever landed on disk. */
	async function overwriteOnDisk() {
		if (!editor.filePath) return;
		const snapshot = editor.content;
		conflict = null;
		editor.isSaving = true;
		try {
			const hash = await saveFile(editor.filePath, snapshot);
			editor.markSaved(snapshot, hash);
			if (nodeId) await loadLinks(nodeId);
		} catch (e) { error = String(e); editor.isSaving = false; }
	}

	async function loadLinks(id: string) {
		const [bl, fl] = await Promise.all([getBacklinks(id), getForwardLinks(id)]);
		backlinks = bl; forwardLinks = fl;
	}

	async function handleBack() {
		clearTimeout(autoSaveTimer);
		await navigation.goBack();
	}

	function handleInsertLink(n: NodeRecord) {
		editorComponent?.insertAtCursor(`[[id:${n.id}][${n.title ?? n.id}]]`);
	}

	function startRename() {
		showMenu = false;
		renameTitle = node?.title ?? '';
		showRename = true;
	}
	async function handleRename() {
		if (!nodeId || !renameTitle.trim()) return;
		// rename_node rewrites the file from disk, so unsaved edits must land first
		// and the pending autosave must not fire against the renamed file.
		clearTimeout(autoSaveTimer);
		await handleSave();
		if (conflict || editor.isDirty) { error = 'Save your changes before renaming.'; return; }
		try {
			await renameNode(nodeId, renameTitle.trim());
			showRename = false;
			// Reload the node
			await loadNode(nodeId);
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) { error = String(e); }
	}

	/** Add :ID: properties to the nearest heading, turning it into an org-roam node */
	function makeHeadingIntoNode() {
		modifyContent(content => {
			const lines = content.split('\n');
			const idx = findNearestHeadlineIdx(lines);
			if (idx === -1) return content;
			// Check if it already has :PROPERTIES: with :ID: below it
			let insertAt = idx;
			for (let j = idx + 1; j < lines.length && j <= idx + 10; j++) {
				const t = lines[j].trim();
				if (t === ':PROPERTIES:') {
					// Check if :ID: already exists in the drawer
					for (let k = j + 1; k < lines.length; k++) {
						if (lines[k].trim() === ':END:') break;
						if (lines[k].trim().startsWith(':ID:')) return content; // already a node
					}
					// Add :ID: inside existing properties
					const id = crypto.randomUUID();
					lines.splice(j + 1, 0, `:ID: ${id}`);
					return lines.join('\n');
				}
				if (t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:')) {
					insertAt = j;
				} else break;
			}
			const id = crypto.randomUUID();
			lines.splice(insertAt + 1, 0, ':PROPERTIES:', `:ID: ${id}`, ':END:');
			return lines.join('\n');
		});
	}

	// ── Tags ──────────────────────────────────────────────────

	function onTag() {
		// Fallback for web: prompt for tag name
		const tag = prompt('Enter tag name:');
		if (tag?.trim()) toggleTag(tag.trim());
	}

	/** Toggle a tag in the #+FILETAGS: line */
	function toggleTag(tag: string) {
		modifyContent(content => joinLines(toggleFiletag(splitLines(content), tag), content));
	}

	// ── Find in page ──────────────────────────────────────────

	function doFind() {
		if (!contentEl || !findQuery.trim()) {
			clearHighlights();
			findCount = 0;
			findCurrent = 0;
			return;
		}
		clearHighlights();
		const query = findQuery.toLowerCase();
		const walker = document.createTreeWalker(contentEl, NodeFilter.SHOW_TEXT);
		const matches: { node: Text; start: number }[] = [];
		while (walker.nextNode()) {
			const textNode = walker.currentNode as Text;
			const text = textNode.textContent?.toLowerCase() ?? '';
			let idx = text.indexOf(query);
			while (idx !== -1) {
				matches.push({ node: textNode, start: idx });
				idx = text.indexOf(query, idx + 1);
			}
		}
		// Highlight matches by wrapping in <mark>
		// Process in reverse to preserve offsets
		for (let i = matches.length - 1; i >= 0; i--) {
			const { node, start } = matches[i];
			const range = document.createRange();
			range.setStart(node, start);
			range.setEnd(node, start + query.length);
			const mark = document.createElement('mark');
			mark.className = 'mycelium-find-match';
			mark.dataset.matchIdx = String(i);
			mark.style.cssText = 'background:#fde68a;color:#92400e;border-radius:2px;padding:0 1px;';
			range.surroundContents(mark);
		}
		findCount = matches.length;
		findCurrent = matches.length > 0 ? 1 : 0;
		scrollToMatch(0);
	}

	function scrollToMatch(idx: number) {
		if (!contentEl) return;
		const marks = contentEl.querySelectorAll('mark.mycelium-find-match');
		marks.forEach((m, i) => {
			(m as HTMLElement).style.background = i === idx ? '#f59e0b' : '#fde68a';
			(m as HTMLElement).style.color = i === idx ? '#fff' : '#92400e';
		});
		marks[idx]?.scrollIntoView({ behavior: 'smooth', block: 'center' });
	}

	function findNext() {
		if (findCount === 0) return;
		findCurrent = (findCurrent % findCount) + 1;
		scrollToMatch(findCurrent - 1);
	}

	function findPrev() {
		if (findCount === 0) return;
		findCurrent = findCurrent <= 1 ? findCount : findCurrent - 1;
		scrollToMatch(findCurrent - 1);
	}

	function clearHighlights() {
		if (!contentEl) return;
		const marks = contentEl.querySelectorAll('mark.mycelium-find-match');
		marks.forEach(mark => {
			const parent = mark.parentNode;
			if (parent) {
				parent.replaceChild(document.createTextNode(mark.textContent ?? ''), mark);
				parent.normalize(); // merge adjacent text nodes
			}
		});
	}

	function closeFind() {
		clearHighlights();
		showFind = false;
		findQuery = '';
		findCount = 0;
		findCurrent = 0;
	}

	// Source mode toolbar actions
	function onBold() { editorComponent?.wrapSelection('*', '*'); }
	function onItalic() { editorComponent?.wrapSelection('/', '/'); }
	function onCode() { editorComponent?.wrapSelection('~', '~'); }
	function onVerbatim() { editorComponent?.wrapSelection('=', '='); }
	function onUnderline() { editorComponent?.wrapSelection('_', '_'); }
	function onStrike() { editorComponent?.wrapSelection('+', '+'); }
	function onLink() { showLinkSwitcher = true; }
	function onCheckbox() { editorComponent?.insertLinePrefix('- [ ] '); }
	function onList() { editorComponent?.insertLinePrefix('- '); }
	function onHeading(_level?: number) { editorComponent?.insertHeading(); }
	function onSrcBlock() { editorComponent?.insertAtCursor('\n#+BEGIN_SRC \n\n#+END_SRC\n'); }
	function onQuote() { editorComponent?.insertAtCursor('\n#+BEGIN_QUOTE\n\n#+END_QUOTE\n'); }
	function onTable(rows: number = 2, cols: number = 2) {
		const hdr = '| ' + Array.from({length: cols}, (_, i) => `Header ${i+1}`).join(' | ') + ' |\n';
		const sep = '|' + Array.from({length: cols}, () => '----------').join('+') + '|\n';
		const body = Array.from({length: rows}, () =>
			'| ' + Array.from({length: cols}, () => '          ').join(' | ') + ' |\n'
		).join('');
		editorComponent?.insertAtCursor('\n' + hdr + sep + body);
	}
	function onTimestamp() {
		const d = new Date();
		const ds = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
		const days = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
		editorComponent?.insertAtCursor(`<${ds} ${days[d.getDay()]}>`);
	}


	function keywordConfig(): KeywordConfig {
		// `orgConfig.keywordConfig` folds the waiting states into the not-done set.
		// Building this by hand from `todoKeywords` alone would make a headline
		// like `* WAITING Ship it` parse as a task titled "WAITING Ship it".
		return orgConfig?.keywordConfig ?? { todoKeywords: ['TODO'], doneKeywords: ['DONE'] };
	}

	/** Split preserving the file's line endings, which `join` restores. */
	function splitLines(content: string): string[] {
		return content.split('\n');
	}

	function joinLines(next: readonly string[], original: string): string {
		const joined = next.join('\n');
		return joined === original ? original : joined;
	}

	function nowForOrg() {
		return { date: localDate(), time: localTime() };
	}

	/**
	 * Move a headline to a new TODO state with org's completion semantics: a
	 * repeating task is rescheduled and stays open, anything else gets a CLOSED
	 * stamp when it becomes DONE and loses it when reopened.
	 */
	function applyTodoState(src: readonly string[], idx: number, keyword: string | null): string[] {
		const config = keywordConfig();
		const wasDone = isDoneKeyword(getTodoKeyword(src[idx], config), config);
		const becomesDone = isDoneKeyword(keyword, config);
		let next = [...src];

		if (becomesDone && !wasDone) {
			const repeat = applyRepeaterOnDone(next, idx, nowForOrg());
			if (repeat.repeated) {
				next = repeat.lines;
				next[idx] = setTodoKeyword(next[idx], repeatKeyword(config), config);
				return next;
			}
			next[idx] = setTodoKeyword(next[idx], keyword, config);
			return setClosed(next, idx, nowForOrg());
		}

		next[idx] = setTodoKeyword(next[idx], keyword, config);
		return wasDone && !becomesDone ? setClosed(next, idx, null) : next;
	}

	function setPlanningEntry(kind: PlanningKind, timestamp: string | null) {
		modifyContent(content => {
			const src = splitLines(content);
			const idx = findNearestHeadlineIdx(src);
			if (idx === -1) return content;
			const next = timestamp
				? setPlanningDate(src, idx, kind, timestamp)
				: removePlanning(src, idx, kind);
			return joinLines(next, content);
		});
	}

	/** Modify the content via CodeMirror so the view stays in sync */
	function modifyContent(fn: (content: string) => string) {
		const newContent = fn(editor.content);
		if (newContent !== editor.content) {
			editorComponent?.replaceContent(newContent);
			editor.updateContent(newContent);
		}
	}

	/** Find the headline line index at or above the cursor position */
	function findNearestHeadlineIdx(lines: string[]): number {
		// Use cursor position from CodeMirror if available
		const cursorPos = editorComponent?.getCursorPos() ?? editor.content.length;
		let charCount = 0;
		let cursorLine = 0;
		for (let i = 0; i < lines.length; i++) {
			charCount += lines[i].length + 1; // +1 for newline
			if (charCount > cursorPos) { cursorLine = i; break; }
		}
		// Search backwards from cursor line to find nearest heading
		for (let i = cursorLine; i >= 0; i--) {
			if (/^\*+\s/.test(lines[i])) return i;
		}
		return -1;
	}

	function onTodo(keyword: string | null) {
		modifyContent(content => {
			const src = splitLines(content);
			const idx = findNearestHeadlineIdx(src);
			if (idx === -1) return content;
			return joinLines(applyTodoState(src, idx, keyword), content);
		});
	}

	/** Cycle through the configured TODO keywords, then back to none */
	function cycleTodo() {
		const src = splitLines(editor.content);
		const idx = findNearestHeadlineIdx(src);
		if (idx === -1) return;
		const current = getTodoKeyword(src[idx], keywordConfig());
		onTodo(nextTodoKeyword(current, keywordConfig()));
	}

	function onPriority(priority: string | null) {
		modifyContent(content => {
			const src = splitLines(content);
			const idx = findNearestHeadlineIdx(src);
			if (idx === -1) return content;
			const next = [...src];
			next[idx] = setPriority(next[idx], priority, keywordConfig());
			return joinLines(next, content);
		});
	}

	function onDeadline() {
		setDeadline(localDate());
	}

	function onScheduled() {
		setScheduled(localDate());
	}

	/** Set the deadline from the native date picker, or remove it */
	function setDeadline(timestamp: string | null) {
		setPlanningEntry('DEADLINE', timestamp);
	}

	/** Set the scheduled date from the native date picker, or remove it */
	function setScheduled(timestamp: string | null) {
		setPlanningEntry('SCHEDULED', timestamp);
	}

	function isTauriRuntime(): boolean {
		try {
			return typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__ !== undefined;
		} catch {
			return false;
		}
	}

	async function onImage() {
		// In the app the image must actually be copied into the vault. Only browser
		// preview falls back to a plain file input; a real import failure is an
		// error, not a reason to insert a link to a file that was never copied.
		if (!isTauriRuntime()) {
			const input = document.createElement('input');
			input.type = 'file';
			input.accept = 'image/*';
			input.onchange = () => {
				const file = input.files?.[0];
				if (file) editorComponent?.insertAtCursor(`[[file:images/${file.name}]]`);
			};
			input.click();
			return;
		}

		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const selected = await open({
				multiple: false,
				filters: [{ name: 'Images', extensions: ['png','jpg','jpeg','gif','svg','webp','bmp','heic'] }],
			});
			if (!selected) return;
			const relativePath = await importImage(selected as string);
			editorComponent?.insertAtCursor(`[[file:${relativePath}]]`);
		} catch (e) {
			error = `Could not import that image: ${e}`;
		}
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: calc(var(--safe-area-top) + 8px); padding-bottom: 8px; min-height: 48px;">
		<button onclick={handleBack} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>

		<h1 class="flex-1 truncate text-lg font-semibold">{node?.title ?? 'Loading...'}</h1>

		<!-- Find in page (reading mode) -->
		{#if !showSource}
			<button
				onclick={() => { showFind = !showFind; if (!showFind) closeFind(); }}
				class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800"
				aria-label="Find in page"
			>
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" /></svg>
			</button>
		{/if}

		<!-- Read / Edit toggle -->
		<button
			onclick={() => { showSource = !showSource; if (showFind) closeFind(); }}
			class="rounded-lg px-3 py-1.5 text-xs font-semibold transition-colors {showSource
				? 'bg-mycelium-600 text-white'
				: 'bg-surface-100 text-surface-700 dark:bg-surface-800 dark:text-surface-300'}"
			title="Toggle view (Cmd+E)"
		>
			{showSource ? 'Editing' : 'Reading'}
		</button>

		<!-- Links drawer toggle -->
		{#if backlinks.length > 0 || forwardLinks.length > 0}
			<button
				onclick={() => (showLinks = !showLinks)}
				class="flex items-center gap-1 rounded-lg px-2 py-1.5 text-xs font-medium transition-colors {showLinks ? 'bg-mycelium-100 text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300' : 'text-surface-700 hover:bg-surface-100 dark:text-surface-300 dark:hover:bg-surface-800'}"
				title="Toggle links panel"
			>
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 011.242 7.244l-4.5 4.5a4.5 4.5 0 01-6.364-6.364l1.757-1.757m9.86-1.125a4.5 4.5 0 00-1.242-7.244l-4.5-4.5a4.5 4.5 0 00-6.364 6.364L4.757 8.688" /></svg>
				{backlinks.length + forwardLinks.length}
			</button>
		{/if}

		<!-- Save indicator -->
		{#if editor.isSaving}
			<span class="text-xs text-surface-700 dark:text-surface-300">Saving...</span>
		{:else if editor.isDirty}
			<button onclick={handleSave} class="rounded-lg bg-mycelium-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-mycelium-700">Save</button>
		{/if}

		<!-- Export menu -->
		<div class="relative">
			<button onclick={() => (showMenu = !showMenu)} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Menu">
				<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6.75a.75.75 0 110-1.5.75.75 0 010 1.5zM12 12.75a.75.75 0 110-1.5.75.75 0 010 1.5zM12 18.75a.75.75 0 110-1.5.75.75 0 010 1.5z" /></svg>
			</button>
			{#if showMenu}
				<button class="fixed inset-0 z-30" onclick={() => (showMenu = false)} aria-label="Close"></button>
				<div class="absolute right-0 top-full z-40 mt-1 w-48 rounded-lg border border-surface-200 bg-surface-0 py-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<button onclick={startRename} class="flex w-full px-4 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800">Rename Node</button>
				</div>
			{/if}
		</div>
	</header>

	{#if error}<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>{/if}

	{#if conflict}
		<div class="flex flex-wrap items-center gap-3 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-300">
			<span class="grow">This file changed on disk since you opened it. Saving now would overwrite those changes.</span>
			<button
				class="rounded border border-amber-300 px-2 py-1 font-medium hover:bg-amber-100 dark:border-amber-700 dark:hover:bg-amber-900"
				onclick={reloadFromDisk}
			>Load their version</button>
			<button
				class="rounded border border-amber-300 px-2 py-1 font-medium hover:bg-amber-100 dark:border-amber-700 dark:hover:bg-amber-900"
				onclick={overwriteOnDisk}
			>Keep mine</button>
		</div>
	{/if}

	<!-- Find bar -->
	{#if showFind}
		<div class="flex shrink-0 items-center gap-2 border-b border-surface-200 bg-surface-50 px-4 py-2 dark:border-surface-700 dark:bg-surface-900">
			<input
				type="text"
				bind:value={findQuery}
				oninput={() => doFind()}
				onkeydown={(e) => { if (e.key === 'Enter') { e.shiftKey ? findPrev() : findNext(); } if (e.key === 'Escape') closeFind(); }}
				placeholder="Find in page..."
				class="flex-1 rounded-md border border-surface-200 bg-surface-0 px-3 py-1.5 text-sm focus:border-mycelium-500 focus:outline-none dark:border-surface-700 dark:bg-surface-950"
				autofocus
			/>
			{#if findCount > 0}
				<span class="text-xs text-surface-700 dark:text-surface-300">{findCurrent}/{findCount}</span>
			{/if}
			<button onclick={findPrev} class="rounded p-1.5 hover:bg-surface-200 dark:hover:bg-surface-700" aria-label="Previous">
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5" /></svg>
			</button>
			<button onclick={findNext} class="rounded p-1.5 hover:bg-surface-200 dark:hover:bg-surface-700" aria-label="Next">
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" /></svg>
			</button>
			<button onclick={closeFind} class="rounded p-1.5 hover:bg-surface-200 dark:hover:bg-surface-700" aria-label="Close">
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
			</button>
		</div>
	{/if}

	<!-- Content -->
	<div class="flex flex-1 overflow-hidden">
		<div class="flex-1 overflow-y-auto">
			{#if editor.hasFile}
				<div bind:this={contentEl} class="mx-auto max-w-3xl p-4 pb-8">
					{#if showSource}
						<OrgEditor bind:this={editorComponent} />
					{:else}
						{#key editor.content}
							<RenderedView content={editor.content} vaultPath={vault.path ?? ''} onLinkClick={(id) => navigation.navigateToNode(id)} onContentChange={(c) => editor.updateContent(c)} />
						{/key}
					{/if}
				</div>
			{:else}
				<div class="flex h-full items-center justify-center"><p class="text-surface-700 dark:text-surface-300">Loading...</p></div>
			{/if}
		</div>

		<!-- Links side drawer -->
		{#if showLinks}
			<button class="fixed inset-0 z-20 bg-black/30 lg:hidden" onclick={() => (showLinks = false)} aria-label="Close links"></button>
		{/if}
		<aside
			class="fixed right-0 top-14 bottom-0 z-30 w-80 max-w-[85vw] transform overflow-y-auto border-l bg-surface-0 transition-transform duration-200 dark:border-surface-700 dark:bg-surface-950 lg:relative lg:top-0 lg:z-0 lg:w-72 lg:shrink-0"
			class:translate-x-0={showLinks}
			class:translate-x-full={!showLinks}
			style="border-color: {document.documentElement.classList.contains('dark') ? '#334155' : '#e2e8f0'}"
		>
			<div class="sticky top-0 flex items-center justify-between border-b p-3" style="border-color:inherit;background:inherit">
				<span class="text-sm font-semibold">Links</span>
				<button onclick={() => (showLinks = false)} class="rounded p-1 hover:bg-surface-100 dark:hover:bg-surface-800 lg:hidden" aria-label="Close">
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
				</button>
			</div>
			<div class="p-3">
				<BacklinkPanel {backlinks} {forwardLinks} {unlinkedMentions} />
			</div>
		</aside>
	</div>

	<!-- Toolbar only in edit mode -->
	{#if showSource}
		<EditorToolbar {onBold} {onItalic} {onCode} {onVerbatim} {onUnderline} {onStrike} {onLink} {onCheckbox} {onHeading} {onList} {onSrcBlock} {onQuote} {onTable} {onTimestamp} {onImage} {onTodo} {onPriority} {onDeadline} {onScheduled} />
	{/if}

	<MobileNav />
</div>

<!-- Link inserter only in edit mode -->
{#if showLinkSwitcher && showSource}
	<QuickSwitcher open={true} mode="insert-link" onclose={() => (showLinkSwitcher = false)} oninsert={handleInsertLink} />
{/if}

<!-- Rename dialog -->
{#if showRename}
	<button class="fixed inset-0 z-40 bg-black/50" onclick={() => (showRename = false)} aria-label="Close"></button>
	<div class="fixed inset-x-4 top-[20%] z-50 mx-auto max-w-md rounded-xl border border-surface-200 bg-surface-0 p-5 shadow-2xl dark:border-surface-700 dark:bg-surface-900">
		<h2 class="mb-3 text-lg font-bold">Rename Node</h2>
		<p class="mb-3 text-xs text-surface-700 dark:text-surface-300">This will update the title and all backlink descriptions across your vault.</p>
		<input
			type="text"
			bind:value={renameTitle}
			onkeydown={(e) => e.key === 'Enter' && handleRename()}
			class="w-full rounded-lg border border-surface-200 bg-surface-50 px-4 py-2.5 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-950"
			autofocus
		/>
		<div class="mt-4 flex justify-end gap-2">
			<button onclick={() => (showRename = false)} class="rounded-lg px-4 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800">Cancel</button>
			<button onclick={handleRename} disabled={!renameTitle.trim()} class="rounded-lg bg-mycelium-600 px-4 py-2 text-sm font-semibold text-white hover:bg-mycelium-700 disabled:opacity-50">Rename</button>
		</div>
	</div>
{/if}
