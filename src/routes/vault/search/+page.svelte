<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { searchFull } from '$lib/tauri/commands';
	import { highlightSnippet } from '$lib/search/snippet';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { SearchResult } from '$lib/types/node';

	let query = $state('');
	let results = $state<SearchResult[]>([]);
	let isSearching = $state(false);
	let error = $state<string | null>(null);
	let searchTimeout: ReturnType<typeof setTimeout>;

	function handleInput() {
		clearTimeout(searchTimeout);
		if (!query.trim()) {
			results = [];
			error = null;
			return;
		}
		searchTimeout = setTimeout(() => doSearch(), 200);
	}

	async function doSearch() {
		if (!query.trim()) return;
		isSearching = true;
		error = null;
		try {
			results = await searchFull(query.trim());
		} catch (e) {
			results = [];
			error = String(e);
		} finally {
			isSearching = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			clearTimeout(searchTimeout);
			doSearch();
		}
	}
</script>

<div class="flex h-full flex-col">
	<header
		class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700"
		style="padding-top: calc(env(safe-area-inset-top, 0px) + 8px); padding-bottom: 8px; min-height: 48px;"
	>
		<button
			onclick={() => navigation.navigateToVault()}
			class="rounded-lg p-2 hover:bg-surface-100 dark:hover:bg-surface-800"
			aria-label="Back"
		>
			<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
				<path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" />
			</svg>
		</button>
		<input
			type="text"
			bind:value={query}
			oninput={handleInput}
			onkeydown={handleKeydown}
			placeholder="Search titles and content..."
			class="flex-1 rounded-lg border border-surface-200 bg-surface-50 px-3 py-2 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-900"
		/>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if isSearching}
			<p class="text-sm text-surface-700 dark:text-surface-300">Searching...</p>
		{:else if error}
			<div class="rounded-lg bg-red-50 p-3 dark:bg-red-950">
				<p class="text-sm font-medium text-red-600 dark:text-red-400">Search failed</p>
				<p class="mt-1 text-xs text-red-600/80 dark:text-red-400/80">{error}</p>
				<button
					onclick={doSearch}
					class="mt-2 rounded-lg border border-red-200 px-3 py-1.5 text-xs font-medium text-red-600 hover:bg-red-100 dark:border-red-800 dark:text-red-400 dark:hover:bg-red-900"
				>
					Try again
				</button>
			</div>
		{:else if results.length > 0}
			<ul class="space-y-1">
				{#each results as result}
					<li>
						<button
							onclick={() => navigation.navigateToNode(result.id)}
							class="w-full rounded-lg px-4 py-3 text-left hover:bg-surface-100 dark:hover:bg-surface-800"
						>
							<div class="flex items-center gap-2">
								<span class="font-medium">{result.title ?? 'Untitled'}</span>
								<span
									class="rounded px-1.5 py-0.5 text-[10px] font-medium uppercase {result.match_type === 'title'
										? 'bg-mycelium-100 text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300'
										: 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300'}"
								>
									{result.match_type}
								</span>
							</div>
							{#if result.snippet}
								<div class="mt-1 text-xs leading-relaxed text-surface-700 dark:text-surface-300">
									{@html highlightSnippet(result.snippet)}
								</div>
							{/if}
							<div class="mt-1 truncate text-[10px] text-surface-700/60 dark:text-surface-300/60">
								{result.file}
							</div>
						</button>
					</li>
				{/each}
			</ul>
		{:else if query.trim()}
			<p class="text-sm text-surface-700 dark:text-surface-300">No results found.</p>
		{:else}
			<div class="space-y-3">
				<p class="text-sm text-surface-700 dark:text-surface-300">
					Search across titles and file content.
				</p>
				<div class="rounded-lg border border-surface-200 p-3 dark:border-surface-700">
					<p class="text-xs text-surface-700 dark:text-surface-300">
						Tips: Use quotes for exact phrases. Results include highlighted snippets showing matching context.
					</p>
				</div>
			</div>
		{/if}
	</div>

	<MobileNav />
</div>
