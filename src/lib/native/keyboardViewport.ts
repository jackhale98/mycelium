/**
 * Keep the app inside the part of the screen the keyboard leaves visible.
 *
 * iOS does not shrink the page when the keyboard opens: it overlays the
 * keyboard and scrolls the whole document to reveal the focused field, which
 * pushes the header off the top while the bottom of the note — where the
 * cursor usually is — sits under the keyboard. The visual viewport is the
 * part still showing, so the app is sized to it while a keyboard is up, and
 * the document scroll iOS adds is put back, leaving the note's own scroller to
 * keep the cursor in view.
 *
 * Android resizes the WebView for the keyboard itself, so this is iOS only.
 */

/** Shorter than this, and the viewport change is a toolbar, not a keyboard. */
const KEYBOARD_MIN = 120;

/** The app height for a visual viewport, or `null` to fill the screen. */
export function appHeightFor(viewportHeight: number, windowHeight: number): number | null {
	return windowHeight - viewportHeight >= KEYBOARD_MIN ? Math.round(viewportHeight) : null;
}

/** Dispatched after the app is resized for the keyboard. */
export const VIEWPORT_EVENT = 'mycelium-viewport';

export function installKeyboardViewport(): () => void {
	const vv = window.visualViewport;
	if (!vv || !/iPhone|iPad|iPod/i.test(navigator.userAgent)) return () => {};
	const root = document.documentElement;

	let frame = 0;
	const apply = () => {
		cancelAnimationFrame(frame);
		frame = requestAnimationFrame(() => {
			const height = appHeightFor(vv.height, window.innerHeight);
			if (height === null) {
				root.style.removeProperty('--app-height');
			} else {
				root.style.setProperty('--app-height', `${height}px`);
				if (window.scrollY !== 0) window.scrollTo(0, 0);
			}
			window.dispatchEvent(new Event(VIEWPORT_EVENT));
		});
	};

	vv.addEventListener('resize', apply);
	vv.addEventListener('scroll', apply);
	return () => {
		cancelAnimationFrame(frame);
		vv.removeEventListener('resize', apply);
		vv.removeEventListener('scroll', apply);
		root.style.removeProperty('--app-height');
	};
}

/**
 * Mark the document while the note editor has focus, so chrome that only
 * gets in the way of typing — the tab bar — can step aside.
 */
export function installEditingClass(isEditor: (el: Element | null) => boolean): () => void {
	const root = document.documentElement;
	const sync = () => root.classList.toggle('editing', isEditor(document.activeElement));
	// After focusout, activeElement is not yet the next field; look once it is.
	const onFocusOut = () => requestAnimationFrame(sync);
	document.addEventListener('focusin', sync);
	document.addEventListener('focusout', onFocusOut);
	sync();
	return () => {
		document.removeEventListener('focusin', sync);
		document.removeEventListener('focusout', onFocusOut);
		root.classList.remove('editing');
	};
}
