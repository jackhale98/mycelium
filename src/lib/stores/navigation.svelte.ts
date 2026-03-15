export type Tab = 'files' | 'graph' | 'search' | 'daily' | 'settings';

class NavigationStore {
	activeTab = $state<Tab>('files');
	sidebarOpen = $state(false);
	searchOpen = $state(false);

	navigateToNode(id: string) {
		this.activeTab = 'files';
		window.location.hash = '';
		window.location.href = `/vault/node/${id}`;
	}

	navigateToGraph() {
		this.activeTab = 'graph';
		window.location.href = '/vault/graph';
	}

	navigateToSearch() {
		this.activeTab = 'search';
		window.location.href = '/vault/search';
	}

	navigateToDaily() {
		this.activeTab = 'daily';
		window.location.href = '/vault/daily';
	}

	navigateToTags() {
		window.location.href = '/vault/tags';
	}

	navigateToVault() {
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
