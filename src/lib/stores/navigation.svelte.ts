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
				// Keep last 50 entries
				if (history.length > 50) history.splice(0, history.length - 50);
				sessionStorage.setItem('mycelium-nav-history', JSON.stringify(history));
			} catch { /* ignore */ }
		}
	}

	/** Go back to the previous page in our history stack */
	goBack() {
		try {
			const history = JSON.parse(sessionStorage.getItem('mycelium-nav-history') ?? '[]') as string[];
			const prev = history.pop();
			sessionStorage.setItem('mycelium-nav-history', JSON.stringify(history));
			if (prev) {
				window.location.href = prev;
				return;
			}
		} catch { /* ignore */ }
		// Fallback: go to vault home
		window.location.href = '/vault';
	}

	navigateToNode(id: string) {
		this.pushHistory();
		this.activeTab = 'files';
		window.location.href = `/vault/node/${id}`;
	}

	navigateToGraph() {
		this.pushHistory();
		this.activeTab = 'graph';
		window.location.href = '/vault/graph';
	}

	navigateToSearch() {
		this.pushHistory();
		this.activeTab = 'search';
		window.location.href = '/vault/search';
	}

	navigateToDaily() {
		this.pushHistory();
		this.activeTab = 'daily';
		window.location.href = '/vault/daily';
	}

	navigateToTags() {
		this.pushHistory();
		window.location.href = '/vault/tags';
	}

	navigateToVault() {
		this.pushHistory();
		this.activeTab = 'files';
		window.location.href = '/vault';
	}

	navigateHome() {
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
