<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { getOrCreateDaily, listDailyNotes } from '$lib/tauri/commands';
	import MobileNav from '$lib/components/common/MobileNav.svelte';
	import type { NodeRecord } from '$lib/types/node';

	let dailyNotes = $state<NodeRecord[]>([]);
	let isLoading = $state(false);
	let error = $state<string | null>(null);

	function todayString(): string {
		const d = new Date();
		return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
	}

	$effect(() => {
		loadDailyNotes();
	});

	async function loadDailyNotes() {
		try {
			dailyNotes = await listDailyNotes();
		} catch (e) {
			error = String(e);
		}
	}

	async function openToday() {
		isLoading = true;
		error = null;
		try {
			const node = await getOrCreateDaily(todayString());
			navigation.navigateToNode(node.id);
		} catch (e) {
			error = String(e);
		} finally {
			isLoading = false;
		}
	}

	async function openDate(date: string) {
		isLoading = true;
		try {
			const node = await getOrCreateDaily(date);
			navigation.navigateToNode(node.id);
		} catch (e) {
			error = String(e);
		} finally {
			isLoading = false;
		}
	}

	// Generate last 7 days for quick access
	function recentDays(): string[] {
		const days: string[] = [];
		for (let i = 0; i < 7; i++) {
			const d = new Date();
			d.setDate(d.getDate() - i);
			days.push(
				`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
			);
		}
		return days;
	}

	const days = recentDays();
</script>

<div class="flex h-full flex-col">
	<header
		class="flex h-14 shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700"
		style="padding-top: var(--safe-area-top)"
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
		<h1 class="text-lg font-semibold">Daily Notes</h1>
	</header>

	{#if error}
		<div class="bg-red-50 px-4 py-2 text-sm text-red-600 dark:bg-red-950 dark:text-red-400">
			{error}
		</div>
	{/if}

	<div class="flex-1 overflow-y-auto p-4">
		<div class="mx-auto max-w-lg space-y-6">
			<!-- Today button -->
			<button
				onclick={openToday}
				disabled={isLoading}
				class="w-full rounded-xl bg-mycelium-600 px-6 py-4 text-left text-white shadow-md transition-colors hover:bg-mycelium-700 disabled:opacity-50"
			>
				<div class="text-lg font-bold">Today</div>
				<div class="mt-1 text-sm opacity-80">{todayString()}</div>
			</button>

			<!-- Quick access: last 7 days -->
			<div>
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Recent Days
				</h2>
				<div class="space-y-1">
					{#each days as day}
						{@const existing = dailyNotes.find((n) => n.title === day)}
						<button
							onclick={() => openDate(day)}
							class="flex w-full items-center justify-between rounded-lg px-4 py-3 text-left hover:bg-surface-100 dark:hover:bg-surface-800"
						>
							<span class="font-medium">{day}</span>
							{#if existing}
								<span class="text-xs text-mycelium-600 dark:text-mycelium-400">
									exists
								</span>
							{:else}
								<span class="text-xs text-surface-700 dark:text-surface-300">create</span>
							{/if}
						</button>
					{/each}
				</div>
			</div>

			<!-- All daily notes -->
			{#if dailyNotes.length > 0}
				<div>
					<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
						All Daily Notes ({dailyNotes.length})
					</h2>
					<div class="space-y-1">
						{#each dailyNotes as note}
							<button
								onclick={() => navigation.navigateToNode(note.id)}
								class="w-full rounded-lg px-4 py-2.5 text-left text-sm hover:bg-surface-100 dark:hover:bg-surface-800"
							>
								{note.title ?? note.file}
							</button>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>

	<MobileNav />
</div>
