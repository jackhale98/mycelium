/**
 * Tell the native keyboard toolbar whether the note editor has focus.
 *
 * The toolbar's buttons all act on the editor, so it belongs above the keyboard
 * only while the editor is being typed into. Every other field — the search
 * bar, a modal, quick capture — should get a plain keyboard, as a native text
 * field would.
 *
 * Split out from the DOM listeners so the decision can be tested.
 */

/**
 * `true` when `target` is the editor's text. The editable area, not the whole
 * editor: CodeMirror's own find panel lives inside `.cm-editor` too, and is a
 * plain field.
 */
export function isEditorTarget(target: EventTarget | null): boolean {
	if (!target || typeof (target as Element).closest !== 'function') return false;
	return (target as Element).closest('.cm-content') !== null;
}

/**
 * What to report for a focus event, or `null` when nothing should be sent.
 *
 * `focusin` always reports where focus landed: it is the one event that fires
 * for every field, and it fires before the keyboard comes up. `focusout`
 * reports only when focus is leaving for nowhere — when it moves to another
 * field, the `focusin` that follows already says where — so a hop between two
 * fields never flashes the toolbar.
 */
export function focusReport(
	type: 'focusin' | 'focusout',
	target: EventTarget | null,
	relatedTarget: EventTarget | null
): boolean | null {
	if (type === 'focusin') return isEditorTarget(target);
	return relatedTarget ? null : false;
}

type Sender = (focused: boolean) => void;

/**
 * The fastest channel to native: iOS registers a script message handler that
 * arrives ahead of WebKit's own focus notification, so the keyboard comes up
 * with the right accessory. Android, and iOS before the handler exists, go
 * through the plugin command.
 */
function nativeSender(): Sender {
	return (focused) => {
		const handler = (window as any).webkit?.messageHandlers?.myceliumEditorFocus;
		if (handler) {
			handler.postMessage(focused);
			return;
		}
		import('@tauri-apps/api/core')
			.then(({ invoke }) => invoke('plugin:folder-picker|set_editor_focused', { focused }))
			.catch(() => {
				/* no toolbar on this platform */
			});
	};
}

/**
 * Report editor focus to the native toolbar for as long as the page lives.
 * Returns a function that removes the listeners.
 */
export function installEditorFocusReporting(send: Sender = nativeSender()): () => void {
	let last: boolean | null = null;
	const report = (focused: boolean | null) => {
		if (focused === null || focused === last) return;
		last = focused;
		send(focused);
	};

	const onFocusIn = (e: FocusEvent) => report(focusReport('focusin', e.target, e.relatedTarget));
	const onFocusOut = (e: FocusEvent) => report(focusReport('focusout', e.target, e.relatedTarget));
	// A tap on the editor arms the toolbar before focus even moves, so it is in
	// place when the keyboard is raised. If no focus follows, nothing shows: the
	// accessory only exists while a keyboard does, and the next field to take
	// focus reports for itself.
	const onTouchStart = (e: TouchEvent) => {
		if (isEditorTarget(e.target)) report(true);
	};
	document.addEventListener('focusin', onFocusIn);
	document.addEventListener('focusout', onFocusOut);
	document.addEventListener('touchstart', onTouchStart, { capture: true, passive: true });

	// A page load starts with nothing focused, whatever the previous page left
	// the toolbar believing.
	report(isEditorTarget(document.activeElement));

	return () => {
		document.removeEventListener('focusin', onFocusIn);
		document.removeEventListener('focusout', onFocusOut);
		document.removeEventListener('touchstart', onTouchStart, { capture: true });
	};
}
