<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import { syncVault, checkVaultChanges, getAllTags, getNodesByTag, listFiles, listNodes } from '$lib/tauri/commands';
	import type { NodeRecord } from '$lib/types/node';
	import { onDbUpdated } from '$lib/tauri/events';
	import { theme } from '$lib/stores/theme.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import CreateNodeModal from '$lib/components/common/CreateNodeModal.svelte';
	import QuickCapture from '$lib/components/common/QuickCapture.svelte';
	import type { TagCount } from '$lib/types/node';

	let showCreateModal = $state(false);
	let search = $state('');
	let tags = $state<TagCount[]>([]);
	let selectedTag = $state<string | null>(null);
	let showTagDropdown = $state(false);
	let tagNodeIds = $state<Set<string>>(new Set());

	const filteredNodes = $derived(
		vault.nodes.filter(n => {
			const matchesSearch = !search.trim() || (n.title?.toLowerCase().includes(search.toLowerCase()) ?? false);
			const matchesTag = !selectedTag || tagNodeIds.has(n.id);
			return matchesSearch && matchesTag;
		})
	);

	async function selectTag(tag: string | null) {
		selectedTag = tag;
		showTagDropdown = false;
		if (tag) {
			try {
				const nodes = await getNodesByTag(tag);
				tagNodeIds = new Set(nodes.map(n => n.id));
			} catch { tagNodeIds = new Set(); }
		} else {
			tagNodeIds = new Set();
		}
	}

	onMount(() => {
		if (!vault.isOpen) {
			window.location.href = '/';
			return;
		}
		const unlistenPromise = onDbUpdated();
		async function handleFocus() {
			try {
				const changed = await checkVaultChanges();
				if (changed) {
					const result = await syncVault();
					if (result.indexed > 0 || result.removed > 0) {
						const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
						vault.updateFiles(files);
						vault.updateNodes(nodes);
					}
				}
			} catch {}
		}
		window.addEventListener('focus', handleFocus);

		// Load tags
		getAllTags().then(t => { tags = t; }).catch(() => {});

		return () => {
			unlistenPromise.then((unlisten) => unlisten());
			window.removeEventListener('focus', handleFocus);
		};
	});
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header
		class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700"
		style="padding-top: calc(env(safe-area-inset-top, 0px) + 8px); padding-bottom: 8px;"
	>
		<img src="/logo.svg" alt="" class="h-7 w-7 shrink-0 rounded" />
		<h1 class="shrink-0 text-lg font-semibold">Mycelium</h1>
		<div class="flex-1"></div>
		<button onclick={() => (showCreateModal = true)} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="New node">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" /></svg>
		</button>
		<button onclick={() => theme.cycle()} class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Toggle theme">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
				{#if theme.isDark}
					<path stroke-linecap="round" stroke-linejoin="round" d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12 18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z" />
				{:else}
					<path stroke-linecap="round" stroke-linejoin="round" d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 009.002-5.998z" />
				{/if}
			</svg>
		</button>
		<a href="/vault/settings" class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Settings">
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
				<path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
				<path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
			</svg>
		</a>
	</header>

	<!-- Search + Tag filter bar -->
	<div class="shrink-0 border-b border-surface-200 px-4 py-2 dark:border-surface-700">
		<div class="flex items-center gap-2">
			<!-- Search input -->
			<div class="relative flex-1">
				<svg class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
				</svg>
				<input
					type="text"
					bind:value={search}
					placeholder="Filter nodes..."
					class="w-full rounded-lg border border-surface-200 bg-surface-50 py-2 pl-9 pr-3 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-900"
				/>
			</div>

			<!-- Tag filter dropdown -->
			<div class="relative">
				<button
					onclick={() => (showTagDropdown = !showTagDropdown)}
					class="flex items-center gap-1.5 rounded-lg border px-3 py-2 text-sm {selectedTag
						? 'border-mycelium-300 bg-mycelium-50 text-mycelium-700 dark:border-mycelium-700 dark:bg-mycelium-950 dark:text-mycelium-300'
						: 'border-surface-200 hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800'}"
				>
					<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
						<path stroke-linecap="round" stroke-linejoin="round" d="M9.568 3H5.25A2.25 2.25 0 003 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 005.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 009.568 3z" />
						<path stroke-linecap="round" stroke-linejoin="round" d="M6 6h.008v.008H6V6z" />
					</svg>
					{selectedTag ? `#${selectedTag}` : 'Tags'}
				</button>

				{#if showTagDropdown}
					<button class="fixed inset-0 z-10" onclick={() => (showTagDropdown = false)} aria-label="Close"></button>
					<div class="absolute right-0 top-full z-20 mt-1 w-56 max-h-64 overflow-y-auto rounded-lg border border-surface-200 bg-surface-0 py-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
						{#if selectedTag}
							<button
								onclick={() => selectTag(null)}
								class="flex w-full items-center gap-2 px-3 py-2 text-sm font-medium text-red-600 hover:bg-surface-100 dark:text-red-400 dark:hover:bg-surface-800"
							>
								Clear filter
							</button>
							<div class="my-1 border-t border-surface-200 dark:border-surface-700"></div>
						{/if}
						{#each tags as tag}
							<button
								onclick={() => selectTag(tag.tag)}
								class="flex w-full items-center justify-between px-3 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800 {selectedTag === tag.tag ? 'bg-mycelium-50 dark:bg-mycelium-950' : ''}"
							>
								<span>#{tag.tag}</span>
								<span class="text-xs text-surface-700 dark:text-surface-300">{tag.count}</span>
							</button>
						{/each}
						{#if tags.length === 0}
							<div class="px-3 py-3 text-center text-sm text-surface-700 dark:text-surface-300">No tags</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	</div>

	<!-- Stats bar -->
	{#if vault.lastSync}
		<div class="shrink-0 border-b border-surface-200 px-4 py-1.5 dark:border-surface-700">
			<p class="text-xs text-surface-700 dark:text-surface-300">
				{filteredNodes.length} of {vault.nodeCount} nodes
				{#if selectedTag}&middot; #{selectedTag}{/if}
				{#if search.trim()}&middot; matching "{search}"{/if}
			</p>
		</div>
	{/if}

	<!-- Node list -->
	<div class="flex-1 overflow-y-auto">
		<ul class="divide-y divide-surface-100 dark:divide-surface-800">
			{#each filteredNodes as node}
				<li>
					<button
						onclick={() => navigation.navigateToNode(node.id)}
						class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-surface-50 active:bg-surface-100 dark:hover:bg-surface-800/50 dark:active:bg-surface-800"
					>
						<!-- Icon -->
						<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg {node.todo ? 'bg-red-50 dark:bg-red-950' : 'bg-mycelium-50 dark:bg-mycelium-950'}">
							{#if node.todo}
								<span class="text-[10px] font-bold text-red-600 dark:text-red-400">{node.todo.slice(0,1)}</span>
							{:else if node.level === 0}
								<svg class="h-4 w-4 text-mycelium-600 dark:text-mycelium-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
									<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
								</svg>
							{:else}
								<span class="text-xs text-mycelium-600 dark:text-mycelium-400">{'*'.repeat(Math.min(node.level, 3))}</span>
							{/if}
						</div>
						<!-- Content -->
						<div class="min-w-0 flex-1">
							<div class="truncate font-medium">{node.title ?? 'Untitled'}</div>
							<div class="truncate text-xs text-surface-700 dark:text-surface-300">
								{node.file.split('/').pop()}
							</div>
						</div>
						<!-- Arrow -->
						<svg class="h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
							<path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" />
						</svg>
					</button>
				</li>
			{/each}
		</ul>
		{#if filteredNodes.length === 0}
			<div class="p-8 text-center text-sm text-surface-700 dark:text-surface-300">
				{#if search.trim() || selectedTag}
					No matching nodes.
				{:else}
					No nodes found. Make sure your .org files have :ID: properties.
				{/if}
			</div>
		{/if}
	</div>

	<MobileNav />
</div>

<CreateNodeModal open={showCreateModal} onclose={() => (showCreateModal = false)} />
<QuickCapture />
