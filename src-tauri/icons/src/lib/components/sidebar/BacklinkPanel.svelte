<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import type { BacklinkRecord, ForwardLink, SearchResult } from '$lib/types/node';

	let {
		backlinks = [],
		forwardLinks = [],
		unlinkedMentions = [],
	}: {
		backlinks?: BacklinkRecord[];
		forwardLinks?: ForwardLink[];
		unlinkedMentions?: SearchResult[];
	} = $props();

	let backlinkExpanded = $state(true);
	let forwardExpanded = $state(true);
	let mentionsExpanded = $state(false);

	function stripOrg(text: string): string {
		return text
			.replace(/\[\[id:[^\]]+\]\[([^\]]*)\]\]/g, '$1')  // [[id:...][desc]] -> desc
			.replace(/\[\[([^\]]+)\]\]/g, '$1')                 // [[path]] -> path
			.replace(/\*([^*]+)\*/g, '$1')                      // *bold*
			.replace(/\/([^/]+)\//g, '$1')                      // /italic/
			.replace(/~([^~]+)~/g, '$1')                        // ~code~
			.replace(/=([^=]+)=/g, '$1');                        // =verbatim=
	}
</script>

<!-- Backlinks -->
{#if backlinks.length > 0}
	<div class="mt-6 rounded-xl border border-surface-200 dark:border-surface-700">
		<button
			onclick={() => (backlinkExpanded = !backlinkExpanded)}
			class="flex w-full items-center justify-between px-4 py-3"
		>
			<div class="flex items-center gap-2">
				<svg class="h-4 w-4 text-mycelium-600 dark:text-mycelium-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M9 15L3 9m0 0l6-6M3 9h12a6 6 0 010 12h-3" />
				</svg>
				<span class="text-sm font-semibold">Backlinks</span>
				<span class="rounded-full bg-mycelium-100 px-2 py-0.5 text-xs font-medium text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300">
					{backlinks.length}
				</span>
			</div>
			<svg
				class="h-4 w-4 text-surface-700 transition-transform dark:text-surface-300"
				class:rotate-180={backlinkExpanded}
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				stroke-width="2"
			>
				<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" />
			</svg>
		</button>

		{#if backlinkExpanded}
			<ul class="border-t border-surface-200 dark:border-surface-700">
				{#each backlinks as bl}
					<li class="border-b border-surface-100 last:border-b-0 dark:border-surface-800">
						<button
							onclick={() => navigation.navigateToNode(bl.source_id)}
							class="w-full px-4 py-3 text-left transition-colors hover:bg-surface-50 dark:hover:bg-surface-800/50"
						>
							<div class="flex items-center gap-2">
								<span class="font-medium text-sm">{bl.source_title ?? bl.source_file.split('/').pop()}</span>
							</div>
							{#if bl.context}
								<p class="mt-1.5 text-xs leading-relaxed text-surface-700 dark:text-surface-300 line-clamp-2">
									{stripOrg(bl.context)}
								</p>
							{/if}
							<p class="mt-1 text-[10px] text-surface-700/50 dark:text-surface-300/50">
								{bl.source_file.split('/').pop()}
							</p>
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}

<!-- Forward links -->
{#if forwardLinks.length > 0}
	<div class="mt-3 rounded-xl border border-surface-200 dark:border-surface-700">
		<button
			onclick={() => (forwardExpanded = !forwardExpanded)}
			class="flex w-full items-center justify-between px-4 py-3"
		>
			<div class="flex items-center gap-2">
				<svg class="h-4 w-4 text-blue-600 dark:text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" />
				</svg>
				<span class="text-sm font-semibold">Links to</span>
				<span class="rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300">
					{forwardLinks.length}
				</span>
			</div>
			<svg
				class="h-4 w-4 text-surface-700 transition-transform dark:text-surface-300"
				class:rotate-180={forwardExpanded}
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				stroke-width="2"
			>
				<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" />
			</svg>
		</button>

		{#if forwardExpanded}
			<ul class="border-t border-surface-200 dark:border-surface-700">
				{#each forwardLinks as fl}
					<li>
						<button
							onclick={() => navigation.navigateToNode(fl.dest_id)}
							class="flex w-full items-center gap-2 px-4 py-2.5 text-left text-sm transition-colors hover:bg-surface-50 dark:hover:bg-surface-800/50"
						>
							<svg class="h-3.5 w-3.5 shrink-0 text-blue-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
								<path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 011.242 7.244l-4.5 4.5a4.5 4.5 0 01-6.364-6.364l1.757-1.757m9.86-1.125a4.5 4.5 0 00-1.242-7.244l-4.5-4.5a4.5 4.5 0 00-6.364 6.364L4.757 8.688" />
							</svg>
							<span class="truncate">{fl.dest_title ?? fl.dest_id}</span>
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}

<!-- Unlinked mentions -->
{#if unlinkedMentions.length > 0}
	<div class="mt-3 rounded-xl border border-surface-200 dark:border-surface-700">
		<button
			onclick={() => (mentionsExpanded = !mentionsExpanded)}
			class="flex w-full items-center justify-between px-4 py-3"
		>
			<div class="flex items-center gap-2">
				<svg class="h-4 w-4 text-amber-600 dark:text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path stroke-linecap="round" stroke-linejoin="round" d="M7.5 8.25h9m-9 3H12m-9.75 1.51c0 1.6 1.123 2.994 2.707 3.227 1.129.166 2.27.293 3.423.379.35.026.67.21.865.501L12 21l2.755-4.133a1.14 1.14 0 01.865-.501 48.172 48.172 0 003.423-.379c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0012 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018z" />
				</svg>
				<span class="text-sm font-semibold">Unlinked Mentions</span>
				<span class="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-700 dark:bg-amber-900 dark:text-amber-300">
					{unlinkedMentions.length}
				</span>
			</div>
			<svg class="h-4 w-4 text-surface-700 transition-transform dark:text-surface-300" class:rotate-180={mentionsExpanded} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" /></svg>
		</button>
		{#if mentionsExpanded}
			<ul class="border-t border-surface-200 dark:border-surface-700">
				{#each unlinkedMentions as m}
					<li class="border-b border-surface-100 last:border-b-0 dark:border-surface-800">
						<button onclick={() => navigation.navigateToNode(m.id)} class="w-full px-4 py-3 text-left hover:bg-surface-50 dark:hover:bg-surface-800/50">
							<div class="text-sm font-medium">{m.title ?? m.file.split('/').pop()}</div>
							{#if m.snippet}
								<p class="mt-1 text-xs text-surface-700 dark:text-surface-300 line-clamp-2">{m.snippet}</p>
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}

{#if backlinks.length === 0 && forwardLinks.length === 0 && unlinkedMentions.length === 0}
	<div class="mt-6 rounded-xl border border-dashed border-surface-200 p-4 text-center dark:border-surface-700">
		<p class="text-xs text-surface-700 dark:text-surface-300">
			No links yet. Type <code class="rounded bg-surface-100 px-1 dark:bg-surface-800">[[</code> in the editor to link to other nodes.
		</p>
	</div>
{/if}
