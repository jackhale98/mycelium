import { goto } from '$app/navigation';

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
	goBack() {
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
	navigateToNode(id: string) {
		this.pushHistory();
		this.activeTab = 'files';
		window.location.href = `/vault/node/${id}`;
	}

	// Tab navigations use goto() for instant client-side transitions (no flash)
	navigateToGraph() {
		this.pushHistory();
		this.activeTab = 'graph';
		goto('/vault/graph');
	}

	navigateToSearch() {
		this.pushHistory();
		this.activeTab = 'search';
		goto('/vault/search');
	}

	navigateToDaily() {
		this.pushHistory();
		this.activeTab = 'daily';
		goto('/vault/daily');
	}

	navigateToTags() {
		this.pushHistory();
		goto('/vault/tags');
	}

	navigateToVault() {
		this.pushHistory();
		this.activeTab = 'files';
		goto('/vault');
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
