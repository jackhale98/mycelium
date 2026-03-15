<script lang="ts">
	import { navigation, type Tab } from '$lib/stores/navigation.svelte';

	const tabs: { id: Tab; label: string; icon: string; href: string }[] = [
		{
			id: 'files', label: 'Files', href: '/vault',
			icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z',
		},
		{
			id: 'daily', label: 'Daily', href: '/vault/daily',
			icon: 'M6.75 3v2.25M17.25 3v2.25M3 18.75V7.5a2.25 2.25 0 012.25-2.25h13.5A2.25 2.25 0 0121 7.5v11.25m-18 0A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75m-18 0v-7.5A2.25 2.25 0 015.25 9h13.5A2.25 2.25 0 0121 11.25v7.5',
		},
		{
			id: 'agenda', label: 'Agenda', href: '/vault/agenda',
			icon: 'M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z',
		},
		{
			id: 'graph', label: 'Graph', href: '/vault/graph',
			icon: 'M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5',
		},
		{
			id: 'search', label: 'Search', href: '/vault/search',
			icon: 'M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z',
		},
	];
</script>

<nav
	class="flex h-14 shrink-0 items-center justify-around border-t border-surface-200 bg-surface-0 dark:border-surface-700 dark:bg-surface-950 lg:hidden"
	style="padding-bottom: var(--safe-area-bottom)"
>
	{#each tabs as tab}
		<a
			href={tab.href}
			onclick={(e) => { e.preventDefault(); navigation.setTab(tab.id); window.location.href = tab.href; }}
			class="flex flex-col items-center gap-0.5 px-3 py-1.5 text-[10px] transition-colors"
			class:text-mycelium-600={navigation.activeTab === tab.id}
			class:dark:text-mycelium-400={navigation.activeTab === tab.id}
			class:text-surface-700={navigation.activeTab !== tab.id}
			class:dark:text-surface-300={navigation.activeTab !== tab.id}
		>
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
				<path stroke-linecap="round" stroke-linejoin="round" d={tab.icon} />
			</svg>
			<span>{tab.label}</span>
		</a>
	{/each}
</nav>
