<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { theme, type ThemeMode } from '$lib/stores/theme.svelte';
	import { prefs, type SaveMode } from '$lib/stores/prefs.svelte';
	import { syncVault, rebuildDatabase, listFiles, listNodes } from '$lib/tauri/commands';
	import type { SyncResult } from '$lib/types/vault';
	import {
		orgConfig,
		parseConfigList,
		validateKeywords,
		validatePriorities,
	} from '$lib/stores/orgconfig.svelte';
	import MobileNav from '$lib/components/common/MobileNav.svelte';

	let isSyncing = $state(false);
	let isRebuilding = $state(false);
	let syncMessage = $state<string | null>(null);

	// Editable copies of org config
	let todoInput = $state(orgConfig.todoKeywords.join(', '));
	let waitingInput = $state(orgConfig.waitingKeywords.join(', '));
	let doneInput = $state(orgConfig.doneKeywords.join(', '));
	let prioInput = $state(orgConfig.priorities.join(', '));

	const todoList = $derived(parseConfigList(todoInput));
	const waitingList = $derived(parseConfigList(waitingInput));
	const doneList = $derived(parseConfigList(doneInput));
	const prioList = $derived(parseConfigList(prioInput));

	const todoError = $derived(validateKeywords(todoList));
	const waitingError = $derived(validateKeywords(waitingList));
	const doneError = $derived(validateKeywords(doneList));
	const prioError = $derived(validatePriorities(prioList));
	// A keyword in two categories has no defined colour, so reject it here
	// rather than let one category silently win.
	const keywordSetError = $derived(
		todoList.length + doneList.length === 0
			? 'At least one active or done keyword is required.'
			: todoError || waitingError || doneError
				? null
				: validateKeywords([...todoList, ...waitingList, ...doneList])
	);
	const orgConfigError = $derived(
		todoError ?? waitingError ?? doneError ?? prioError ?? keywordSetError
	);

	/** Category swatches, so the colours are visible where they are configured. */
	const previewRows = $derived([
		{ label: 'Active', cls: 'state-todo', words: todoList },
		{ label: 'Waiting', cls: 'state-waiting', words: waitingList },
		{ label: 'Done', cls: 'state-done', words: doneList },
	].filter((r) => r.words.length > 0));

	async function saveOrgConfig() {
		if (orgConfigError) return;

		// Only the *set* of keywords affects the index. Moving a word between the
		// active and waiting categories changes nothing on disk, so it must not
		// trigger a full re-index.
		const setOf = (words: string[]) => [...words].sort().join(',');
		const keywordsChanged =
			setOf([...todoList, ...waitingList, ...doneList]) !== setOf(orgConfig.allKeywords);

		try {
			await orgConfig.update({
				todoKeywords: todoList,
				waitingKeywords: waitingList,
				doneKeywords: doneList,
				priorities: prioList,
			});
		} catch (e) {
			syncMessage = `Error saving org settings: ${e}`;
			return;
		}

		todoInput = orgConfig.todoKeywords.join(', ');
		waitingInput = orgConfig.waitingKeywords.join(', ');
		doneInput = orgConfig.doneKeywords.join(', ');
		prioInput = orgConfig.priorities.join(', ');

		// The parser now holds the new keywords, so re-indexing writes the new set.
		if (!keywordsChanged) return;

		if (isSyncing || isRebuilding) {
			syncMessage =
				'Keywords saved, but the index still uses the old set. Run "Rebuild DB" once the current operation finishes.';
			return;
		}

		isRebuilding = true;
		syncMessage = 'Re-indexing with new keywords...';
		try {
			const result = await rebuildDatabase();
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);
			syncMessage = `Re-indexed ${result.indexed} files with updated TODO keywords`;
		} catch (e) {
			syncMessage = `Error re-indexing: ${e}. Keywords are saved but the index still uses the old set — run "Rebuild DB" to finish.`;
		} finally {
			isRebuilding = false;
		}
	}

	async function handleResync() {
		isSyncing = true;
		syncMessage = null;
		try {
			const result = await syncVault();
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);
			let msg = `Synced: ${result.indexed} indexed, ${result.skipped} unchanged, ${result.removed} removed`;
			msg += collisionNote(result.id_collisions);
			if (result.broken_links && result.broken_links > 0) {
				msg += `. ${result.broken_links} broken link(s) cleaned up.`;
			}
			syncMessage = msg;
		} catch (e) {
			syncMessage = `Error: ${e}`;
		} finally {
			isSyncing = false;
		}
	}


	function collisionNote(collisions: SyncResult['id_collisions']): string {
		if (!collisions || collisions.length === 0) return '';
		const first = collisions[0];
		const a = first.existing_file.split('/').pop() ?? first.existing_file;
		const b = first.new_file.split('/').pop() ?? first.new_file;
		const extra = collisions.length - 1;
		const rest = extra > 0 ? ` and ${extra} other duplicate ID${extra === 1 ? '' : 's'}` : '';
		return ` ${a} and ${b} share the ID ${first.id}${rest} — org-roam needs every ID to be unique.`;
	}

	async function handleRebuild() {
		if (!confirm('This will drop all indexed data and rebuild the database from scratch. Continue?')) return;
		isRebuilding = true;
		syncMessage = null;
		try {
			const result = await rebuildDatabase();
			const [files, nodes] = await Promise.all([listFiles(), listNodes()]);
			vault.updateFiles(files);
			vault.updateNodes(nodes);
			let msg = `Rebuilt: ${result.indexed} files indexed from scratch`;
			msg += collisionNote(result.id_collisions);
			if (result.broken_links && result.broken_links > 0) {
				msg += `. ${result.broken_links} broken link(s) cleaned up.`;
			}
			syncMessage = msg;
		} catch (e) {
			syncMessage = `Error: ${e}`;
		} finally {
			isRebuilding = false;
		}
	}

	function handleCloseVault() {
		localStorage.removeItem('mycelium-vault-path');
		vault.close();
		navigation.navigateHome();
	}

	function handleForgetVault() {
		localStorage.removeItem('mycelium-vault-path');
		alert('Vault path cleared. You will be asked to choose a vault next time you open the app.');
	}

	function handleChangeVault() {
		localStorage.removeItem('mycelium-vault-path');
		vault.close();
		navigation.navigateHome();
	}

	const saveOptions: { value: SaveMode; label: string; hint: string }[] = [
		{ value: 'auto', label: 'While typing', hint: 'Saves 1.5s after you stop. Best for most vaults.' },
		{ value: 'manual', label: 'When I ask', hint: 'Saves on the Save button. Keeps a git working tree still.' },
	];

	const themeOptions: { value: ThemeMode; label: string; icon: string }[] = [
		{ value: 'light', label: 'Light', icon: '\u2600' },
		{ value: 'dark', label: 'Dark', icon: '\u263E' },
		{ value: 'system', label: 'System', icon: '\u2699' },
	];
</script>

<div class="flex h-full flex-col">
	<header
		class="flex shrink-0 items-center gap-2 border-b border-surface-200 px-4 dark:border-surface-700"
		style="padding-top: calc(var(--safe-area-top) + 8px); padding-bottom: 8px; min-height: 48px;"
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
		<h1 class="text-lg font-semibold">Settings</h1>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		<div class="mx-auto max-w-lg space-y-6">
			<!-- Vault Info -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Vault
				</h2>
				<div class="space-y-2 text-sm">
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Path</span>
						<span class="max-w-[200px] truncate font-mono text-xs">{vault.path ?? 'None'}</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Files</span>
						<span class="font-semibold">{vault.fileCount}</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Nodes</span>
						<span class="font-semibold">{vault.nodeCount}</span>
					</div>
					{#if vault.lastSync}
						<div class="flex justify-between">
							<span class="text-surface-700 dark:text-surface-300">Last indexed</span>
							<span>{vault.lastSync.indexed} files</span>
						</div>
					{/if}
				</div>

				<div class="mt-4 space-y-2">
					<div class="flex gap-2">
						<button
							onclick={handleResync}
							disabled={isSyncing || isRebuilding}
							class="flex-1 rounded-lg border border-surface-200 px-3 py-2 text-sm font-medium hover:bg-surface-100 disabled:opacity-50 dark:border-surface-700 dark:hover:bg-surface-800"
						>
							{isSyncing ? 'Syncing...' : 'Re-sync Vault'}
						</button>
						<button
							onclick={handleRebuild}
							disabled={isSyncing || isRebuilding}
							class="flex-1 rounded-lg border border-amber-200 px-3 py-2 text-sm font-medium text-amber-700 hover:bg-amber-50 disabled:opacity-50 dark:border-amber-800 dark:text-amber-400 dark:hover:bg-amber-950"
						>
							{isRebuilding ? 'Rebuilding...' : 'Rebuild DB'}
						</button>
					</div>
					<div class="flex gap-2">
						<button
							onclick={handleChangeVault}
							class="flex-1 rounded-lg border border-surface-200 px-3 py-2 text-sm font-medium hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800"
						>
							Change Vault
						</button>
						<button
							onclick={handleCloseVault}
							class="flex-1 rounded-lg border border-red-200 px-3 py-2 text-sm font-medium text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-400 dark:hover:bg-red-950"
						>
							Close & Forget
						</button>
					</div>
					<p class="text-[11px] text-surface-700 dark:text-surface-300">
						Re-sync checks for changes. Rebuild drops all indexed data and re-indexes every file from scratch.
					</p>
				</div>

				{#if syncMessage}
					<p class="mt-2 text-xs text-surface-700 dark:text-surface-300">{syncMessage}</p>
				{/if}
			</section>

			<!-- Theme -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Appearance
				</h2>
				<div class="flex gap-2">
					{#each themeOptions as opt}
						<button
							onclick={() => theme.setMode(opt.value)}
							class="flex flex-1 flex-col items-center gap-1 rounded-lg border px-3 py-3 text-sm transition-colors {theme.mode === opt.value
								? 'border-mycelium-500 bg-mycelium-50 text-mycelium-700 dark:bg-mycelium-950 dark:text-mycelium-300'
								: 'border-surface-200 hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800'}"
						>
							<span class="text-lg">{opt.icon}</span>
							<span class="text-xs font-medium">{opt.label}</span>
						</button>
					{/each}
				</div>
			</section>

			<!-- Saving -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Saving
				</h2>
				<div class="flex gap-2">
					{#each saveOptions as opt}
						<button
							onclick={() => prefs.setSaveMode(opt.value)}
							class="flex flex-1 flex-col items-start gap-1 rounded-lg border px-3 py-3 text-left transition-colors {prefs.saveMode === opt.value
								? 'border-mycelium-500 bg-mycelium-50 dark:bg-mycelium-950'
								: 'border-surface-200 hover:bg-surface-100 dark:border-surface-700 dark:hover:bg-surface-800'}"
						>
							<span class="text-sm font-medium {prefs.saveMode === opt.value ? 'text-mycelium-700 dark:text-mycelium-300' : ''}">{opt.label}</span>
							<span class="text-[10px] leading-snug text-surface-700 dark:text-surface-300">{opt.hint}</span>
						</button>
					{/each}
				</div>
				<p class="mt-2 text-[11px] text-surface-700 dark:text-surface-300">
					Either way, leaving a note or switching away from the app saves it — the
					system can close Mycelium at any time, and unsaved edits should not be
					lost because of it.
				</p>
			</section>

			<!-- Org Mode Configuration -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					Org Mode
				</h2>
				<div class="space-y-3">
					<div>
						<label for="todo-keywords" class="mb-1 block text-xs font-medium text-surface-700 dark:text-surface-300">Active Keywords</label>
						<input id="todo-keywords" type="text" bind:value={todoInput} onblur={saveOrgConfig} aria-invalid={todoError !== null} class="w-full rounded-lg border bg-surface-50 px-3 py-2 text-sm dark:bg-surface-950 {todoError ? 'border-red-400 dark:border-red-700' : 'border-surface-200 dark:border-surface-700'}" placeholder="TODO, NEXT" />
						{#if todoError}
							<p class="mt-0.5 text-[10px] text-red-600 dark:text-red-400">{todoError}</p>
						{:else}
							<p class="mt-0.5 text-[10px] text-surface-700 dark:text-surface-300">Work you can start now, comma separated</p>
						{/if}
					</div>
					<div>
						<label for="waiting-keywords" class="mb-1 block text-xs font-medium text-surface-700 dark:text-surface-300">Waiting Keywords</label>
						<input id="waiting-keywords" type="text" bind:value={waitingInput} onblur={saveOrgConfig} aria-invalid={waitingError !== null} class="w-full rounded-lg border bg-surface-50 px-3 py-2 text-sm dark:bg-surface-950 {waitingError ? 'border-red-400 dark:border-red-700' : 'border-surface-200 dark:border-surface-700'}" placeholder="WAITING, HOLD" />
						{#if waitingError}
							<p class="mt-0.5 text-[10px] text-red-600 dark:text-red-400">{waitingError}</p>
						{:else}
							<p class="mt-0.5 text-[10px] text-surface-700 dark:text-surface-300">Blocked on someone else. Still unfinished to org — these stay on the agenda, and only their colour differs.</p>
						{/if}
					</div>
					<div>
						<label for="done-keywords" class="mb-1 block text-xs font-medium text-surface-700 dark:text-surface-300">Done Keywords</label>
						<input id="done-keywords" type="text" bind:value={doneInput} onblur={saveOrgConfig} aria-invalid={doneError !== null} class="w-full rounded-lg border bg-surface-50 px-3 py-2 text-sm dark:bg-surface-950 {doneError ? 'border-red-400 dark:border-red-700' : 'border-surface-200 dark:border-surface-700'}" placeholder="DONE, CANCELLED" />
						{#if doneError}
							<p class="mt-0.5 text-[10px] text-red-600 dark:text-red-400">{doneError}</p>
						{:else}
							<p class="mt-0.5 text-[10px] text-surface-700 dark:text-surface-300">Finished states, comma separated</p>
						{/if}
					</div>
					<div>
						<label for="priorities" class="mb-1 block text-xs font-medium text-surface-700 dark:text-surface-300">Priorities</label>
						<input id="priorities" type="text" bind:value={prioInput} onblur={saveOrgConfig} aria-invalid={prioError !== null} class="w-full rounded-lg border bg-surface-50 px-3 py-2 text-sm dark:bg-surface-950 {prioError ? 'border-red-400 dark:border-red-700' : 'border-surface-200 dark:border-surface-700'}" placeholder="A, B, C" />
						{#if prioError}
							<p class="mt-0.5 text-[10px] text-red-600 dark:text-red-400">{prioError}</p>
						{:else}
							<p class="mt-0.5 text-[10px] text-surface-700 dark:text-surface-300">Priority levels (highest first), comma separated</p>
						{/if}
					</div>
					{#if previewRows.length > 0 && !orgConfigError}
						<div class="rounded-lg border border-surface-200 p-3 dark:border-surface-700">
							<p class="mb-2 text-[10px] font-medium uppercase tracking-wide text-surface-700 dark:text-surface-300">How they appear</p>
							<div class="space-y-1.5">
								{#each previewRows as row}
									<div class="flex items-baseline gap-2">
										<span class="w-14 shrink-0 text-[10px] text-surface-700 dark:text-surface-300">{row.label}</span>
										<span class="flex flex-wrap gap-1">
											{#each row.words as word}<span class="state-chip {row.cls}">{word}</span>{/each}
										</span>
									</div>
								{/each}
							</div>
						</div>
					{/if}
					{#if keywordSetError}
						<p class="text-[11px] text-red-600 dark:text-red-400">{keywordSetError}</p>
					{/if}
					{#if orgConfigError}
						<p class="text-[11px] text-surface-700 dark:text-surface-300">Settings are not saved while there is an error above.</p>
					{/if}
				</div>
			</section>

			<!-- About -->
			<section class="rounded-xl border border-surface-200 p-4 dark:border-surface-700">
				<h2 class="mb-3 text-sm font-semibold uppercase text-surface-700 dark:text-surface-300">
					About
				</h2>
				<div class="space-y-1 text-sm">
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">Mycelium</span>
						<span class="font-mono text-xs">v0.1.0</span>
					</div>
					<div class="flex justify-between">
						<span class="text-surface-700 dark:text-surface-300">License</span>
						<span class="text-xs">Apache 2.0</span>
					</div>
				</div>
				<p class="mt-3 text-xs text-surface-700 dark:text-surface-300">
					Open-source Org Roam mobile knowledge base. Built with Tauri, Svelte, and Rust.
				</p>
			</section>
		</div>
	</div>

	<MobileNav />
</div>
