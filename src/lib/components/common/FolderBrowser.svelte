<script lang="ts">
	import { onMount } from 'svelte';
	import { getDocumentsPath, listSubdirectories, type DirEntry } from '$lib/tauri/commands';

	let {
		open = false,
		onselect,
		onclose,
	}: {
		open?: boolean;
		onselect?: (path: string) => void;
		onclose?: () => void;
	} = $props();

	let currentPath = $state('');
	let entries = $state<DirEntry[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let pathHistory = $state<string[]>([]);

	$effect(() => {
		if (open && !currentPath) {
			loadDocuments();
		}
	});

	async function loadDocuments() {
		loading = true;
		try {
			currentPath = await getDocumentsPath();
			await loadDir(currentPath);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	async function loadDir(path: string) {
		loading = true;
		error = null;
		try {
			entries = await listSubdirectories(path);
			currentPath = path;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	function navigateTo(path: string) {
		pathHistory = [...pathHistory, currentPath];
		loadDir(path);
	}

	function goBack() {
		const prev = pathHistory.pop();
		if (prev) {
			pathHistory = pathHistory;
			loadDir(prev);
		}
	}

	function selectCurrent() {
		onselect?.(currentPath);
	}

	function displayPath(path: string): string {
		// Show just the last 2-3 components
		const parts = path.split('/').filter(Boolean);
		return parts.length > 3 ? '.../' + parts.slice(-3).join('/') : '/' + parts.join('/');
	}
</script>

{#if open}
	<button class="fixed inset-0 z-40 bg-black/50" onclick={() => onclose?.()} aria-label="Close"></button>

	<div class="fixed inset-x-2 top-[10%] bottom-[10%] z-50 flex flex-col overflow-hidden rounded-2xl border border-surface-200 bg-surface-0 shadow-2xl dark:border-surface-700 dark:bg-surface-900" style="max-width: 480px; margin: 0 auto;">
		<!-- Header -->
		<div class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 py-3 dark:border-surface-700">
			{#if pathHistory.length > 0}
				<button onclick={goBack} class="rounded-lg p-1.5 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Back">
					<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5" /></svg>
				</button>
			{/if}
			<div class="min-w-0 flex-1">
				<div class="text-sm font-semibold">Select Folder</div>
				<div class="truncate text-[11px] text-surface-700 dark:text-surface-300">{displayPath(currentPath)}</div>
			</div>
			<button onclick={() => onclose?.()} class="rounded-lg p-1.5 hover:bg-surface-100 dark:hover:bg-surface-800" aria-label="Close">
				<svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" /></svg>
			</button>
		</div>

		<!-- Directory listing -->
		<div class="flex-1 overflow-y-auto">
			{#if loading}
				<div class="p-8 text-center text-sm text-surface-700 dark:text-surface-300">Loading...</div>
			{:else if error}
				<div class="p-4 text-sm text-red-600 dark:text-red-400">{error}</div>
			{:else if entries.length === 0}
				<div class="p-8 text-center text-sm text-surface-700 dark:text-surface-300">Empty folder</div>
			{:else}
				<ul class="divide-y divide-surface-100 dark:divide-surface-800">
					{#each entries as entry}
						{#if entry.is_dir}
							<li>
								<button
									onclick={() => navigateTo(entry.path)}
									class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-surface-50 dark:hover:bg-surface-800/50"
								>
									<svg class="h-5 w-5 shrink-0" style="color:#f59e0b" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z" /></svg>
									<div class="min-w-0 flex-1">
										<div class="truncate text-sm font-medium">{entry.name}</div>
									</div>
									{#if entry.has_org_files}
										<span class="shrink-0 rounded-full bg-mycelium-100 px-2 py-0.5 text-[10px] font-medium text-mycelium-700 dark:bg-mycelium-900 dark:text-mycelium-300">.org</span>
									{/if}
									<svg class="h-4 w-4 shrink-0 text-surface-700 dark:text-surface-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" /></svg>
								</button>
							</li>
						{/if}
					{/each}
				</ul>
			{/if}
		</div>

		<!-- Footer: select this folder -->
		<div class="shrink-0 border-t border-surface-200 p-3 dark:border-surface-700">
			<button
				onclick={selectCurrent}
				class="w-full rounded-lg bg-mycelium-600 px-4 py-2.5 text-sm font-semibold text-white hover:bg-mycelium-700"
			>
				Use This Folder
			</button>
		</div>
	</div>
{/if}
