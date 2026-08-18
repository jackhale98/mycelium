/// User-configurable org-mode settings: TODO keywords, priority levels
/// Persisted to localStorage so they survive page reloads.

export interface OrgConfig {
	todoKeywords: string[];
	doneKeywords: string[];
	priorities: string[];
}

// Org-mode defaults: only TODO and DONE out of the box.
// Users can add NEXT, WAITING, HOLD, CANCELLED etc. in settings.
const DEFAULT_CONFIG: OrgConfig = {
	todoKeywords: ['TODO'],
	doneKeywords: ['DONE'],
	priorities: ['A', 'B', 'C'],
};

/** A keyword has to survive headline tokenisation: no whitespace, no regex metacharacters. */
const KEYWORD_PATTERN = /^[A-Z0-9][A-Z0-9_-]*$/;
/** Priorities are matched a single character at a time in `[#A]` cookies. */
const PRIORITY_PATTERN = /^[A-Z0-9]$/;

/** Split a comma-separated settings field into normalised entries. */
export function parseConfigList(input: string): string[] {
	return input
		.split(',')
		.map((s) => s.trim().toUpperCase())
		.filter(Boolean);
}

/** Returns an error message for the first unusable keyword, or null when all are valid. */
export function validateKeywords(keywords: string[]): string | null {
	for (const kw of keywords) {
		if (!KEYWORD_PATTERN.test(kw)) {
			return `"${kw}" is not a usable keyword. Use letters, digits, - or _ with no spaces (e.g. TODO, IN-PROGRESS).`;
		}
	}
	const seen = new Set<string>();
	for (const kw of keywords) {
		if (seen.has(kw)) return `"${kw}" is listed twice.`;
		seen.add(kw);
	}
	return null;
}

/** Returns an error message for the first unusable priority, or null when all are valid. */
export function validatePriorities(priorities: string[]): string | null {
	for (const p of priorities) {
		if (!PRIORITY_PATTERN.test(p)) {
			return `"${p}" is not a usable priority. Each priority is a single letter or digit (e.g. A, B, C).`;
		}
	}
	return null;
}

class OrgConfigStore {
	todoKeywords = $state<string[]>(DEFAULT_CONFIG.todoKeywords);
	doneKeywords = $state<string[]>(DEFAULT_CONFIG.doneKeywords);
	priorities = $state<string[]>(DEFAULT_CONFIG.priorities);

	constructor() {
		if (typeof localStorage !== 'undefined') {
			try {
				const saved = localStorage.getItem('mycelium-orgconfig');
				if (saved) {
					const data = JSON.parse(saved) as Partial<OrgConfig>;
					const todo = data.todoKeywords?.filter((k) => KEYWORD_PATTERN.test(k)) ?? [];
					const done = data.doneKeywords?.filter((k) => KEYWORD_PATTERN.test(k)) ?? [];
					const prio = data.priorities?.filter((p) => PRIORITY_PATTERN.test(p)) ?? [];
					if (todo.length) this.todoKeywords = todo;
					if (done.length) this.doneKeywords = done;
					if (prio.length) this.priorities = prio;
				}
			} catch { /* ignore */ }
		}
	}

	get allKeywords(): string[] {
		return [...this.todoKeywords, ...this.doneKeywords];
	}

	/**
	 * Send the current keyword set to the Rust parser. Call on app init and after
	 * updates, and await it before anything that re-indexes. Rejects if the parser
	 * could not be updated — the caller has to surface that.
	 */
	async syncToBackend(): Promise<void> {
		const { setTodoKeywords } = await import('$lib/tauri/commands');
		await setTodoKeywords(this.allKeywords);
	}

	/**
	 * Validate, store and push a config change. Throws on invalid input (nothing is
	 * stored) or when the backend push fails (the change is stored locally).
	 */
	async update(config: Partial<OrgConfig>): Promise<void> {
		const todo = config.todoKeywords ?? this.todoKeywords;
		const done = config.doneKeywords ?? this.doneKeywords;
		const prio = config.priorities ?? this.priorities;

		const keywordError = validateKeywords([...todo, ...done]);
		if (keywordError) throw new Error(keywordError);
		const priorityError = validatePriorities(prio);
		if (priorityError) throw new Error(priorityError);

		this.todoKeywords = todo;
		this.doneKeywords = done;
		this.priorities = prio;
		this.persist();
		await this.syncToBackend();
	}

	private persist() {
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('mycelium-orgconfig', JSON.stringify({
				todoKeywords: this.todoKeywords,
				doneKeywords: this.doneKeywords,
				priorities: this.priorities,
			}));
		}
	}
}

export const orgConfig = new OrgConfigStore();
