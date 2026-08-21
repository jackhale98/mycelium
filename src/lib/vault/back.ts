import { editor } from '$lib/stores/editor.svelte';

/** What a back press should do. */
export type BackAction = 'dismiss' | 'navigate' | 'exit';

/**
 * Decide what a back press means, given what is on screen.
 *
 * Split out from the DOM work so it can be tested: this is the decision that
 * determines whether the app closes, and getting it wrong either traps the user
 * inside or drops them out of a note they were editing.
 *
 * An open overlay is the innermost thing, so it goes first. Otherwise back
 * retraces a step, except at the top of the stack where Android expects the app
 * to close — refusing there is the trapped-user bug.
 */
export function decideBack(pathname: string, overlayOpen: boolean): BackAction {
	if (overlayOpen) return 'dismiss';
	const atRoot = pathname === '/' || pathname === '/vault';
	return atRoot ? 'exit' : 'navigate';
}

/**
 * Answer the Android system back press.
 *
 * Returns `true` when the app handled it and should stay open, `false` to let
 * Android close the app — which is what should happen from the top of the
 * stack, and only there.
 *
 * Unsaved work is flushed before leaving a note, for the same reason the
 * editor flushes when the app is backgrounded: back can be the last thing that
 * happens before the process ends.
 */
export function handleBackPress(): boolean {
	if (typeof window === 'undefined') return false;

	// An open overlay cancels the event to claim the press.
	const overlayOpen = !window.dispatchEvent(
		new CustomEvent('mycelium-back', { cancelable: true })
	);

	switch (decideBack(window.location.pathname, overlayOpen)) {
		case 'dismiss':
			return true;
		case 'exit':
			return false;
		case 'navigate':
			void editor.flush();
			window.history.back();
			return true;
	}
}

/** Expose the hook the Android back handler calls into. Returns a cleanup fn. */
export function installBackHandler(): () => void {
	if (typeof window === 'undefined') return () => {};
	(window as unknown as Record<string, unknown>).__myceliumBack = handleBackPress;
	return () => {
		delete (window as unknown as Record<string, unknown>).__myceliumBack;
	};
}
