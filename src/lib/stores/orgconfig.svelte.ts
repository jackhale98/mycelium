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
					if (data.todoKeywords?.length) this.todoKeywords = data.todoKeywords;
					if (data.doneKeywords?.length) this.doneKeywords = data.doneKeywords;
					if (data.priorities?.length) this.priorities = data.priorities;
				}
			} catch { /* ignore */ }
		}
	}

	get allKeywords(): string[] {
		return [...this.todoKeywords, ...this.doneKeywords];
	}

	update(config: Partial<OrgConfig>) {
		if (config.todoKeywords) this.todoKeywords = config.todoKeywords;
		if (config.doneKeywords) this.doneKeywords = config.doneKeywords;
		if (config.priorities) this.priorities = config.priorities;
		this.persist();
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
