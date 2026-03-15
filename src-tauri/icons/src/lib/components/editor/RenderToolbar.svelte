<script lang="ts">
	let {
		onAddHeading,
		onAddLink,
		onAddList,
		onAddCheckbox,
		onAddCodeBlock,
		onAddBold,
		onAddItalic,
		onAddCode,
		onAddVerbatim,
		onToggleSource,
	}: {
		onAddHeading?: (level: number) => void;
		onAddLink?: () => void;
		onAddList?: () => void;
		onAddCheckbox?: () => void;
		onAddCodeBlock?: () => void;
		onAddBold?: () => void;
		onAddItalic?: () => void;
		onAddCode?: () => void;
		onAddVerbatim?: () => void;
		onToggleSource?: () => void;
	} = $props();

	let showHeadingPicker = $state(false);
</script>

<div class="flex h-12 shrink-0 items-center gap-0.5 overflow-x-auto border-t border-surface-200 bg-surface-50 px-2 dark:border-surface-700 dark:bg-surface-900" style="-webkit-overflow-scrolling: touch;">
	<!-- Heading with level picker -->
	<div class="relative">
		<button
			onclick={() => (showHeadingPicker = !showHeadingPicker)}
			title="Add heading"
			class="flex h-9 min-w-[40px] items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 active:bg-surface-300 dark:text-surface-300 dark:hover:bg-surface-700"
		>
			H
		</button>
		{#if showHeadingPicker}
			<button class="fixed inset-0 z-20" onclick={() => (showHeadingPicker = false)} aria-label="Close"></button>
			<div class="absolute bottom-full left-0 z-30 mb-1 flex gap-0.5 rounded-lg border border-surface-200 bg-surface-0 p-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
				{#each [1, 2, 3, 4] as level}
					<button
						onclick={() => { onAddHeading?.(level); showHeadingPicker = false; }}
						class="flex h-8 w-8 items-center justify-center rounded text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800"
						title="Heading level {level}"
					>
						H{level}
					</button>
				{/each}
			</div>
		{/if}
	</div>

	<button onclick={() => onAddBold?.()} title="Bold (*text*)" class="flex h-9 min-w-[36px] items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">B</button>
	<button onclick={() => onAddItalic?.()} title="Italic (/text/)" class="flex h-9 min-w-[36px] items-center justify-center rounded-md text-xs italic text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">I</button>
	<button onclick={() => onAddCode?.()} title="Code (~text~)" class="flex h-9 min-w-[36px] items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&lt;&gt;</button>
	<button onclick={() => onAddVerbatim?.()} title="Verbatim (=text=)" class="flex h-9 min-w-[36px] items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">= =</button>
	<button onclick={() => onAddLink?.()} title="Insert link (Cmd+K)" class="flex h-9 min-w-[40px] items-center justify-center rounded-md text-xs font-medium text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">Link</button>
	<button onclick={() => onAddCheckbox?.()} title="Add checkbox" class="flex h-9 min-w-[36px] items-center justify-center rounded-md text-xs text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&#9744;</button>
	<button onclick={() => onAddList?.()} title="Add list item" class="flex h-9 min-w-[36px] items-center justify-center rounded-md text-xs text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&#8226;</button>
	<button onclick={() => onAddCodeBlock?.()} title="Add code block" class="flex h-9 min-w-[36px] items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">{'{}'}</button>

	<div class="flex-1"></div>

	<button
		onclick={() => onToggleSource?.()}
		class="rounded-md px-2.5 py-1.5 text-xs font-medium text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700"
		title="Edit source (Cmd+E)"
	>
		Source
	</button>
</div>
