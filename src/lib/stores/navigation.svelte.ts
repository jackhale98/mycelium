import { goto } from '$app/navigation';
import { editor } from './editor.svelte';

export type Tab = 'files' | 'graph' | 'search' | 'daily' | 'agenda' | 'settings';

class NavigationStore {
	activeTab = $state<Tab>('files');
	sidebarOpen = $state(false);
	searchOpen = $state(false);

	/** Push current page to history before navigating */
	private pushHistory() {
		const current = window.location.pathname;
		if (current && current !== '/') {
			try {
				const history = JSON.parse(sessionStorage.getItem('mycelium-nav-history') ?? '[]') as string[];
				history.push(current);
				if (history.length > 50) history.splice(0, history.length - 50);
				sessionStorage.setItem('mycelium-nav-history', JSON.stringify(history));
			} catch { /* ignore */ }
		}
	}

	/**
	 * Save pending edits before leaving the page. `false` means they did not
	 * land (a conflict, a failed write) and the caller must stay: every way out
	 * of a note throws its buffer away, and the editor is showing why.
	 */
	private async leave(): Promise<boolean> {
		return editor.flush();
	}

	/** Go back to the previous page in our history stack */
	async goBack() {
		if (!(await this.leave())) return;
		try {
			const history = JSON.parse(sessionStorage.getItem('mycelium-nav-history') ?? '[]') as string[];
			const prev = history.pop();
			sessionStorage.setItem('mycelium-nav-history', JSON.stringify(history));
			if (prev) {
				// Node pages need full reload to re-mount with new ID
				if (prev.includes('/vault/node/')) {
					window.location.href = prev;
				} else {
					goto(prev);
				}
				return;
			}
		} catch { /* ignore */ }
		goto('/vault');
	}

	/** Navigate to a node — uses full reload since node page reads ID on mount */
	async navigateToNode(id: string) {
		// A reload discards the JS heap, so pending edits must reach disk first.
		if (!(await this.leave())) return;
		this.pushHistory();
		this.activeTab = 'files';
		window.location.href = `/vault/node/${id}`;
	}

	// Tab navigations use goto() for instant client-side transitions (no flash)
	async navigateToGraph() {
		if (!(await this.leave())) return;
		this.pushHistory();
		this.activeTab = 'graph';
		goto('/vault/graph');
	}

	async navigateToSearch() {
		if (!(await this.leave())) return;
		this.pushHistory();
		this.activeTab = 'search';
		goto('/vault/search');
	}

	async navigateToDaily() {
		if (!(await this.leave())) return;
		this.pushHistory();
		this.activeTab = 'daily';
		goto('/vault/daily');
	}

	async navigateToTags() {
		if (!(await this.leave())) return;
		this.pushHistory();
		goto('/vault/tags');
	}

	async navigateToVault() {
		if (!(await this.leave())) return;
		this.pushHistory();
		this.activeTab = 'files';
		goto('/vault');
	}

	async navigateHome() {
		if (!(await this.leave())) return;
		window.location.href = '/';
	}

	/** Switch to a bottom-bar tab, saving the open note first. */
	async navigateToTab(tab: Tab, href: string) {
		if (!(await this.leave())) return;
		this.activeTab = tab;
		goto(href);
	}

	toggleSidebar() {
		this.sidebarOpen = !this.sidebarOpen;
	}

	toggleSearch() {
		this.searchOpen = !this.searchOpen;
	}

	setTab(tab: Tab) {
		this.activeTab = tab;
	}
}

export const navigation = new NavigationStore();
