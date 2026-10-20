/// App preferences that are not org-mode configuration.
/// Persisted to localStorage so they survive reloads.

export type SaveMode = 'auto' | 'manual';

const KEY = 'mycelium-prefs';

class PrefsStore {
	/**
	 * `auto` saves 1.5s after you stop typing. `manual` saves only when you ask.
	 *
	 * Manual exists for vaults under version control: continuous saving turns a
	 * few minutes of editing into dozens of writes, so the working tree is never
	 * still and a git client shows it as permanently dirty.
	 *
	 * Manual does *not* mean nothing is written without you. Leaving a note or
	 * backgrounding the app still flushes, because a mobile OS can end the
	 * process at any moment and silently losing what someone typed is worse than
	 * an unasked-for write. What manual removes is the steady drip while typing.
	 */
	saveMode = $state<SaveMode>('auto');

	/**
	 * Let the keyboard help while writing: autocorrect, sentence capitals,
	 * spell-check and predictive text.
	 *
	 * On by default, because prose is most of what goes into a note and a
	 * keyboard that does none of this feels broken next to every other app.
	 * It can be turned off for vaults heavy on markup, where the system's
	 * smart punctuation turns a table's `--` into a dash.
	 */
	writingAssist = $state(true);

	constructor() {
		if (typeof localStorage === 'undefined') return;
		try {
			const saved = localStorage.getItem(KEY);
			if (!saved) return;
			const data = JSON.parse(saved) as Partial<{ saveMode: SaveMode; writingAssist: boolean }>;
			if (data.saveMode === 'auto' || data.saveMode === 'manual') {
				this.saveMode = data.saveMode;
			}
			if (typeof data.writingAssist === 'boolean') {
				this.writingAssist = data.writingAssist;
			}
		} catch {
			/* a corrupt preference is not worth failing over */
		}
	}

	get autoSaves(): boolean {
		return this.saveMode === 'auto';
	}

	setSaveMode(mode: SaveMode) {
		this.saveMode = mode;
		this.persist();
	}

	setWritingAssist(on: boolean) {
		this.writingAssist = on;
		this.persist();
	}

	private persist() {
		if (typeof localStorage === 'undefined') return;
		try {
			localStorage.setItem(
				KEY,
				JSON.stringify({ saveMode: this.saveMode, writingAssist: this.writingAssist })
			);
		} catch {
			/* storage can be unavailable; the choice still holds for this session */
		}
	}
}

export const prefs = new PrefsStore();
