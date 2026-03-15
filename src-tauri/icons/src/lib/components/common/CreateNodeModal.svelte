<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { createFile, listNodes, listFiles } from '$lib/tauri/commands';

	let {
		open = false,
		onclose,
	}: {
		open?: boolean;
		onclose?: () => void;
	} = $props();

	let title = $state('');
	let isCreating = $state(false);
	let error = $state<string | null>(null);

	async function handleCreate() {
		if (!title.trim()) return;

		isCreating = true;
		error = null;

		try {
			const filePath = await createFile(title.trim());

			// Refresh vault data
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);

			// Navigate to the new node
			const newNode = nodes.find((n) => n.file === filePath);
			if (newNode) {
				navigation.navigateToNode(newNode.id);
			}

			title = '';
			onclose?.();
		} catch (e) {
			error = String(e);
		} finally {
			isCreating = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			handleCreate();
		} else if (e.key === 'Escape') {
			onclose?.();
		}
	}
</script>

{#if open}
	<!-- Backdrop -->
	<button
		class="fixed inset-0 z-40 bg-black/50"
		onclick={() => onclose?.()}
		aria-label="Close"
	></button>

	<!-- Modal -->
	<div class="fixed inset-x-4 top-[15%] z-50 mx-auto max-w-md rounded-xl border border-surface-200 bg-surface-0 p-6 shadow-2xl dark:border-surface-700 dark:bg-surface-900">
		<h2 class="mb-4 text-lg font-bold">New Node</h2>

		<input
			type="text"
			bind:value={title}
			onkeydown={handleKeydown}
			placeholder="Node title..."
			class="w-full rounded-lg border border-surface-200 bg-surface-50 px-4 py-3 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-950"
			autofocus
		/>

		{#if error}
			<p class="mt-2 text-sm text-red-600 dark:text-red-400">{error}</p>
		{/if}

		<div class="mt-4 flex justify-end gap-2">
			<button
				onclick={() => onclose?.()}
				class="rounded-lg px-4 py-2 text-sm font-medium hover:bg-surface-100 dark:hover:bg-surface-800"
			>
				Cancel
			</button>
			<button
				onclick={handleCreate}
				disabled={isCreating || !title.trim()}
				class="rounded-lg bg-mycelium-600 px-4 py-2 text-sm font-semibold text-white hover:bg-mycelium-700 disabled:opacity-50"
			>
				{isCreating ? 'Creating...' : 'Create'}
			</button>
		</div>

		<p class="mt-3 text-xs text-surface-700 dark:text-surface-300">
			Creates a new .org file with a unique :ID: property.
		</p>
	</div>
{/if}
