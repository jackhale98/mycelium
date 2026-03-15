<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { getAllTags, getNodesByTag } from '$lib/tauri/commands';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { NodeRecord, TagCount } from '$lib/types/node';

	let tags = $state<TagCount[]>([]);
	let selectedTag = $state<string | null>(null);
	let tagNodes = $state<NodeRecord[]>([]);
	let error = $state<string | null>(null);
	let filter = $state('');
	let dropdownOpen = $state(false);

	const filteredTags = $derived(
		filter.trim()
			? tags.filter(t => t.tag.toLowerCase().includes(filter.toLowerCase()))
			: tags
	);

	$effect(() => {
		loadTags();
	});

	async function loadTags() {
		try {
			tags = await getAllTags();
		} catch (e) {
			error = String(e);
		}
	}

	async function selectTag(tag: string) {
		selectedTag = tag;
		filter = '';
		dropdownOpen = false;
		try {
			tagNodes = await getNodesByTag(tag);
		} catch (e) {
			error = String(e);
		}
	}

	function clearTag() {
		selectedTag = null;
		tagNodes = [];
		filter = '';
	}

	const tagColors = [
		{ bg: '#dcfce7', text: '#15803d' },
		{ bg: '#dbeafe', text: '#1d4ed8' },
		{ bg: '#ede9fe', text: '#6d28d9' },
		{ bg: '#fef3c7', text: '#b45309' },
		{ bg: '#fce7f3', text: '#be185d' },
		{ bg: '#ccfbf1', text: '#0f766e' },
	];
	function tagStyle(index: number): string {
		const c = tagColors[index % tagColors.length];
		return `background:${c.bg};color:${c.text}`;
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
		<h1 class="text-lg font-semibold">Tags</h1>
	</header>

	{#if error}
		<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">{error}</div>
	{/if}

	<div class="flex-1 overflow-y-auto p-4">
		<div class="mx-auto max-w-lg">
			<!-- Tag selector dropdown -->
			<div class="relative mb-4">
				<div
					class="flex items-center gap-2 rounded-lg border border-surface-200 bg-surface-50 px-3 py-2.5 dark:border-surface-700 dark:bg-surface-900"
				>
					{#if selectedTag}
						<span style={tagStyle(tags.findIndex(t => t.tag === selectedTag))} class="inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold">
							#{selectedTag}
							<button onclick={clearTag} class="ml-0.5 opacity-70 hover:opacity-100" aria-label="Clear tag">&times;</button>
						</span>
					{/if}
					<input
						type="text"
						bind:value={filter}
						onfocus={() => (dropdownOpen = true)}
						placeholder={selectedTag ? 'Change tag...' : 'Search tags...'}
						class="flex-1 bg-transparent text-sm focus:outline-none"
					/>
					<svg class="h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
						<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" />
					</svg>
				</div>

				<!-- Dropdown -->
				{#if dropdownOpen}
					<button class="fixed inset-0 z-10" onclick={() => (dropdownOpen = false)} aria-label="Close"></button>
					<div class="absolute left-0 right-0 top-full z-20 mt-1 max-h-64 overflow-y-auto rounded-lg border border-surface-200 bg-surface-0 py-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
						{#each filteredTags as tag, i}
							<button
								onclick={() => selectTag(tag.tag)}
								class="flex w-full items-center justify-between px-3 py-2 text-sm hover:bg-surface-100 dark:hover:bg-surface-800"
							>
								<span class="flex items-center gap-2">
									<span style={tagStyle(tags.indexOf(tag))} class="inline-block rounded-full px-2 py-0.5 text-xs font-semibold">
										#{tag.tag}
									</span>
								</span>
								<span class="text-xs text-surface-700 dark:text-surface-300">{tag.count} nodes</span>
							</button>
						{/each}
						{#if filteredTags.length === 0}
							<div class="px-3 py-4 text-center text-sm text-surface-700 dark:text-surface-300">
								No matching tags
							</div>
						{/if}
					</div>
				{/if}
			</div>

			<!-- Results -->
			{#if selectedTag}
				<div class="mb-3 text-sm text-surface-700 dark:text-surface-300">
					{tagNodes.length} node{tagNodes.length !== 1 ? 's' : ''} tagged <span class="font-semibold">#{selectedTag}</span>
				</div>
				<div class="space-y-1">
					{#each tagNodes as node}
						<button
							onclick={() => navigation.navigateToNode(node.id)}
							class="w-full rounded-lg px-4 py-3 text-left hover:bg-surface-100 dark:hover:bg-surface-800"
						>
							<div class="font-medium">{node.title ?? 'Untitled'}</div>
							{#if node.todo}
								<span class="mr-1 rounded px-1.5 py-0.5 text-xs font-medium" style="color:#dc2626;background:#fef2f2">{node.todo}</span>
							{/if}
							<div class="mt-0.5 truncate text-xs text-surface-700 dark:text-surface-300">{node.file}</div>
						</button>
					{/each}
				</div>
			{:else}
				<!-- All tags as clickable chips when nothing selected -->
				{#if tags.length > 0 && !dropdownOpen}
					<div class="flex flex-wrap gap-2">
						{#each tags as tag, i}
							<button
								onclick={() => selectTag(tag.tag)}
								style={tagStyle(i)}
								class="inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium transition-opacity hover:opacity-80"
							>
								#{tag.tag}
								<span class="text-xs opacity-60">({tag.count})</span>
							</button>
						{/each}
					</div>
				{:else if tags.length === 0}
					<p class="text-sm text-surface-700 dark:text-surface-300">
						No tags found. Add tags to your org headings: <code class="rounded bg-surface-100 px-1 dark:bg-surface-800">* Heading :tag1:tag2:</code>
					</p>
				{/if}
			{/if}
		</div>
	</div>

	<MobileNav />
</div>
