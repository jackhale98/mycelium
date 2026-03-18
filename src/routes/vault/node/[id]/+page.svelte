<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import {
		getNode, getBacklinks, getForwardLinks, getUnlinkedMentions,
		readFile, saveFile, listNodes, createFile,
		exportMarkdown, exportHtml, renameNode, importImage,
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

	// Auto-save debounce
	$effect(() => {
		const _ = editor.content;
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
		const onKey = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === 'e') { e.preventDefault(); showSource = !showSource; }
			if ((e.metaKey || e.ctrlKey) && e.key === 'k' && showSource) { e.preventDefault(); showLinkSwitcher = true; }
		};
		document.addEventListener('keydown', onKey);

		// Register native iOS keyboard toolbar bridge
		// The native toolbar calls these functions via evaluateJavaScript
		(window as any).__myceliumToolbar = {
			link: () => onLink(),
			heading: () => onHeading(),
			headingLevel: (lvl: number) => { editorComponent?.insertHeadingWithId(lvl); },
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
			srcblock: () => onSrcBlock(),
			quote: () => onQuote(),
			timestamp: () => onTimestamp(),
		};

		return () => {
			document.removeEventListener('org-editor-save', onSave);
			document.removeEventListener('keydown', onKey);
			delete (window as any).__myceliumToolbar;
			clearTimeout(autoSaveTimer);
		};
	});

	async function loadNode(id: string) {
		error = null;
		try {
			node = await getNode(id);
			if (!node) { error = 'Node not found'; return; }
			const content = await readFile(node.file);
			editor.openFile(node.file, content, id);
			const [bl, fl, um] = await Promise.all([getBacklinks(id), getForwardLinks(id), getUnlinkedMentions(id)]);
			backlinks = bl; forwardLinks = fl; unlinkedMentions = um;
		} catch (e) { error = String(e); }
	}

	async function handleSave() {
		if (!editor.filePath || !editor.isDirty) return;
		editor.isSaving = true;
		try {
			await saveFile(editor.filePath, editor.content);
			editor.markSaved();
			if (nodeId) {
				const [bl, fl] = await Promise.all([getBacklinks(nodeId), getForwardLinks(nodeId)]);
				backlinks = bl; forwardLinks = fl;
			}
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) { error = String(e); editor.isSaving = false; }
	}

	function handleBack() {
		clearTimeout(autoSaveTimer);
		if (editor.isDirty && editor.filePath) handleSave();
		navigation.goBack();
	}

	function handleInsertLink(n: NodeRecord) {
		editorComponent?.insertAtCursor(`[[id:${n.id}][${n.title ?? n.id}]]`);
	}

	async function handleExportMd() {
		showMenu = false;
		if (!editor.filePath) return;
		try {
			const md = await exportMarkdown(editor.filePath);
			dl(new Blob([md], {type:'text/markdown'}), (node?.title??'export')+'.md');
		} catch (e) { error = String(e); }
	}
	async function handleExportHtml() {
		showMenu = false;
		if (!editor.filePath) return;
		try {
			const html = await exportHtml(editor.filePath);
			dl(new Blob([html], {type:'text/html'}), (node?.title??'export')+'.html');
		} catch (e) { error = String(e); }
	}
	function dl(b: Blob, n: string) { const a=document.createElement('a'); a.href=URL.createObjectURL(b); a.download=n; a.click(); URL.revokeObjectURL(a.href); }

	function startRename() {
		showMenu = false;
		renameTitle = node?.title ?? '';
		showRename = true;
	}
	async function handleRename() {
		if (!nodeId || !renameTitle.trim()) return;
		try {
			await renameNode(nodeId, renameTitle.trim());
			showRename = false;
			// Reload the node
			await loadNode(nodeId);
			try { vault.updateNodes(await listNodes()); } catch {}
		} catch (e) { error = String(e); }
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

	function todayTimestamp(): string {
		const d = new Date();
		const ds = `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
		const days = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
		return `<${ds} ${days[d.getDay()]}>`;
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
			const lines = content.split('\n');
			const idx = findNearestHeadlineIdx(lines);
			if (idx === -1) return content;
			const line = lines[idx];
			const m = line.match(/^(\*+\s+)/);
			if (!m) return content;
			const stars = m[1];
			let rest = line.slice(stars.length);
			const kwMatch = rest.match(/^(TODO|DONE|NEXT|WAITING|HOLD|CANCELLED|CANCELED)\s+/);
			if (kwMatch) rest = rest.slice(kwMatch[0].length);
			lines[idx] = keyword ? `${stars}${keyword} ${rest}` : `${stars}${rest}`;
			return lines.join('\n');
		});
	}

	/** Cycle through TODO keywords: none -> TODO -> DONE -> none */
	function cycleTodo() {
		const lines = editor.content.split('\n');
		const idx = findNearestHeadlineIdx(lines);
		if (idx === -1) return;
		const line = lines[idx];
		const m = line.match(/^(\*+\s+)/);
		if (!m) return;
		const rest = line.slice(m[1].length);
		const allKw = [...(orgConfig?.todoKeywords ?? ['TODO']), ...(orgConfig?.doneKeywords ?? ['DONE'])];
		const kwMatch = rest.match(/^(\S+)\s/);
		const current = kwMatch ? kwMatch[1] : null;
		const currentIdx = current ? allKw.indexOf(current) : -1;
		let next: string | null;
		if (currentIdx === -1) {
			next = allKw[0] ?? 'TODO'; // none -> first keyword
		} else if (currentIdx === allKw.length - 1) {
			next = null; // last -> none
		} else {
			next = allKw[currentIdx + 1]; // advance
		}
		onTodo(next);
	}

	function onPriority(priority: string | null) {
		modifyContent(content => {
			const lines = content.split('\n');
			const idx = findNearestHeadlineIdx(lines);
			if (idx === -1) return content;
			const line = lines[idx];
			const m = line.match(/^(\*+\s+(?:(?:TODO|DONE|NEXT|WAITING|HOLD|CANCELLED|CANCELED)\s+)?)/);
			if (!m) return content;
			const prefix = m[1];
			let rest = line.slice(prefix.length);
			const prioMatch = rest.match(/^\[#[A-Z]\]\s*/);
			if (prioMatch) rest = rest.slice(prioMatch[0].length);
			lines[idx] = priority ? `${prefix}[#${priority}] ${rest}` : `${prefix}${rest}`;
			return lines.join('\n');
		});
	}

	function onDeadline() {
		modifyContent(content => {
			const lines = content.split('\n');
			const targetLine = findNearestHeadlineIdx(lines);
			if (targetLine === -1) return content;
			let insertAfter = targetLine;
			for (let j = targetLine + 1; j < lines.length; j++) {
				const t = lines[j].trim();
				if (t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:') || t === ':PROPERTIES:') {
					insertAfter = j;
					if (t === ':PROPERTIES:') { while (j < lines.length && lines[j].trim() !== ':END:') j++; insertAfter = j; }
				} else break;
			}
			for (let j = targetLine + 1; j <= insertAfter + 1 && j < lines.length; j++) {
				if (lines[j].includes('DEADLINE:')) {
					lines[j] = lines[j].replace(/DEADLINE:\s*<[^>]*>/, `DEADLINE: ${todayTimestamp()}`);
					return lines.join('\n');
				}
			}
			lines.splice(insertAfter + 1, 0, `DEADLINE: ${todayTimestamp()}`);
			return lines.join('\n');
		});
	}

	function onScheduled() {
		modifyContent(content => {
			const lines = content.split('\n');
			const targetLine = findNearestHeadlineIdx(lines);
			if (targetLine === -1) return content;
			let insertAfter = targetLine;
			for (let j = targetLine + 1; j < lines.length; j++) {
				const t = lines[j].trim();
				if (t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:') || t === ':PROPERTIES:') {
					insertAfter = j;
					if (t === ':PROPERTIES:') { while (j < lines.length && lines[j].trim() !== ':END:') j++; insertAfter = j; }
				} else break;
			}
			for (let j = targetLine + 1; j <= insertAfter + 1 && j < lines.length; j++) {
				if (lines[j].includes('SCHEDULED:')) {
					lines[j] = lines[j].replace(/SCHEDULED:\s*<[^>]*>/, `SCHEDULED: ${todayTimestamp()}`);
					return lines.join('\n');
				}
			}
			lines.splice(insertAfter + 1, 0, `SCHEDULED: ${todayTimestamp()}`);
			return lines.join('\n');
		});
	}

	/** Set deadline with a specific timestamp (from native date picker), or remove it */
	function setDeadline(timestamp: string | null) {
		modifyContent(content => {
			const lines = content.split('\n');
			const targetLine = findNearestHeadlineIdx(lines);
			if (targetLine === -1) return content;
			let insertAfter = targetLine;
			for (let j = targetLine + 1; j < lines.length; j++) {
				const t = lines[j].trim();
				if (t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:') || t === ':PROPERTIES:') {
					insertAfter = j;
					if (t === ':PROPERTIES:') { while (j < lines.length && lines[j].trim() !== ':END:') j++; insertAfter = j; }
				} else break;
			}
			// Find existing DEADLINE line
			for (let j = targetLine + 1; j <= insertAfter + 1 && j < lines.length; j++) {
				if (lines[j].includes('DEADLINE:')) {
					if (timestamp) {
						lines[j] = lines[j].replace(/DEADLINE:\s*<[^>]*>/, `DEADLINE: ${timestamp}`);
					} else {
						lines.splice(j, 1); // Remove the line
					}
					return lines.join('\n');
				}
			}
			if (timestamp) {
				lines.splice(insertAfter + 1, 0, `DEADLINE: ${timestamp}`);
			}
			return lines.join('\n');
		});
	}

	/** Set scheduled with a specific timestamp (from native date picker), or remove it */
	function setScheduled(timestamp: string | null) {
		modifyContent(content => {
			const lines = content.split('\n');
			const targetLine = findNearestHeadlineIdx(lines);
			if (targetLine === -1) return content;
			let insertAfter = targetLine;
			for (let j = targetLine + 1; j < lines.length; j++) {
				const t = lines[j].trim();
				if (t.startsWith('SCHEDULED:') || t.startsWith('DEADLINE:') || t.startsWith('CLOSED:') || t === ':PROPERTIES:') {
					insertAfter = j;
					if (t === ':PROPERTIES:') { while (j < lines.length && lines[j].trim() !== ':END:') j++; insertAfter = j; }
				} else break;
			}
			for (let j = targetLine + 1; j <= insertAfter + 1 && j < lines.length; j++) {
				if (lines[j].includes('SCHEDULED:')) {
					if (timestamp) {
						lines[j] = lines[j].replace(/SCHEDULED:\s*<[^>]*>/, `SCHEDULED: ${timestamp}`);
					} else {
						lines.splice(j, 1);
					}
					return lines.join('\n');
				}
			}
			if (timestamp) {
				lines.splice(insertAfter + 1, 0, `SCHEDULED: ${timestamp}`);
			}
			return lines.join('\n');
		});
	}

	async function onImage() {
		try {
			// Use Tauri dialog to pick an image file
			const { open } = await import('@tauri-apps/plugin-dialog');
			const selected = await open({
				multiple: false,
				filters: [{ name: 'Images', extensions: ['png','jpg','jpeg','gif','svg','webp','bmp'] }],
			});
			if (!selected) return;
			const sourcePath = selected as string;
			const relativePath = await importImage(sourcePath);
			editorComponent?.insertAtCursor(`[[file:${relativePath}]]`);
		} catch {
			// Fallback for browser mode: use file input
			const input = document.createElement('input');
			input.type = 'file';
			input.accept = 'image/*';
			input.onchange = () => {
				const file = input.files?.[0];
				if (file) {
					editorComponent?.insertAtCursor(`[[file:images/${file.name}]]`);
				}
			};
			input.click();
		}
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700" style="padding-top: calc(env(safe-area-inset-top, 0px) + 8px); padding-bottom: 8px; min-height: 48px;">
		<button onclick={handleBack} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
		</button>

		<h1 class="flex-1 truncate text-lg font-semibold">{node?.title ?? 'Loading...'}</h1>

		<!-- Read / Edit toggle -->
		<button
			onclick={() => (showSource = !showSource)}
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
					<div class="my-1 border-t border-surface-200 dark:border-surface-700"></div>
					<button onclick={handleExportMd} class="flex w-full px-4 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800">Export Markdown</button>
					<button onclick={handleExportHtml} class="flex w-full px-4 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800">Export HTML</button>
				</div>
			{/if}
		</div>
	</header>

	{#if error}<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>{/if}

	<!-- Content -->
	<div class="flex flex-1 overflow-hidden">
		<div class="flex-1 overflow-y-auto">
			{#if editor.hasFile}
				<div class="mx-auto max-w-3xl p-4 pb-8">
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
