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

	/** Go back to the previous page in our history stack */
	async goBack() {
		await editor.flush();
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
		await editor.flush();
		this.pushHistory();
		this.activeTab = 'files';
		window.location.href = `/vault/node/${id}`;
	}

	// Tab navigations use goto() for instant client-side transitions (no flash)
	async navigateToGraph() {
		await editor.flush();
		this.pushHistory();
		this.activeTab = 'graph';
		goto('/vault/graph');
	}

	async navigateToSearch() {
		await editor.flush();
		this.pushHistory();
		this.activeTab = 'search';
		goto('/vault/search');
	}

	async navigateToDaily() {
		await editor.flush();
		this.pushHistory();
		this.activeTab = 'daily';
		goto('/vault/daily');
	}

	async navigateToTags() {
		await editor.flush();
		this.pushHistory();
		goto('/vault/tags');
	}

	async navigateToVault() {
		await editor.flush();
		this.pushHistory();
		this.activeTab = 'files';
		goto('/vault');
	}

	async navigateHome() {
		await editor.flush();
		window.location.href = '/';
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
