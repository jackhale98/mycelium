<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import FileTree from './FileTree.svelte';
</script>

<div class="flex h-full flex-col">
	<!-- Sidebar header -->
	<div class="flex items-center justify-between border-b border-surface-200 px-4 py-3 dark:border-surface-700">
		<h2 class="text-sm font-semibold">Files</h2>
		<span class="text-xs text-surface-700 dark:text-surface-300">
			{vault.fileCount} files
		</span>
	</div>

	<!-- File list -->
	<div class="flex-1 overflow-y-auto p-2">
		<FileTree />
	</div>

	<!-- Sidebar footer -->
	<div class="border-t border-surface-200 p-3 dark:border-surface-700">
		<button
			onclick={() => navigation.navigateToSearch()}
			class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-surface-700 hover:bg-surface-100 dark:text-surface-300 dark:hover:bg-surface-800"
		>
			<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
				/>
			</svg>
			Search...
		</button>
	</div>
</div>
