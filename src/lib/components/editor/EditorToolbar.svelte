<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';

	let {
		onBold, onItalic, onCode, onVerbatim, onUnderline, onStrike,
		onLink, onCheckbox, onHeading, onList, onSrcBlock, onQuote, onTable, onTimestamp,
	}: {
		onBold?: () => void;
		onItalic?: () => void;
		onCode?: () => void;
		onVerbatim?: () => void;
		onUnderline?: () => void;
		onStrike?: () => void;
		onLink?: () => void;
		onCheckbox?: () => void;
		onHeading?: (level: number) => void;
		onList?: () => void;
		onSrcBlock?: () => void;
		onQuote?: () => void;
		onTable?: (rows: number, cols: number) => void;
		onTimestamp?: () => void;
	} = $props();

	let showHeadingPicker = $state(false);
	let showTablePicker = $state(false);
	let hoverRow = $state(0);
	let hoverCol = $state(0);
</script>

{#if editor.hasFile}
	<div
		class="flex h-12 shrink-0 items-center gap-0.5 overflow-x-auto border-t border-surface-200 bg-surface-50 px-2 dark:border-surface-700 dark:bg-surface-900"
		style="-webkit-overflow-scrolling: touch;"
	>
		<!-- Heading level picker -->
		<div class="relative shrink-0">
			<button
				onclick={() => (showHeadingPicker = !showHeadingPicker)}
				title="Heading"
				class="flex h-9 min-w-[40px] items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 active:bg-surface-300 dark:text-surface-300 dark:hover:bg-surface-700"
			>H</button>
			{#if showHeadingPicker}
				<button class="fixed inset-0 z-20" onclick={() => (showHeadingPicker = false)} aria-label="Close"></button>
				<div class="absolute bottom-full left-0 z-30 mb-1 flex gap-0.5 rounded-lg border border-surface-200 bg-surface-0 p-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					{#each [1,2,3,4] as lvl}
						<button
							onclick={() => { onHeading?.(lvl); showHeadingPicker = false; }}
							class="flex h-8 w-8 items-center justify-center rounded text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800"
						>H{lvl}</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Inline formatting -->
		<button onclick={() => onBold?.()} title="Bold *text*" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">B</button>
		<button onclick={() => onItalic?.()} title="Italic /text/" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs italic text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">I</button>
		<button onclick={() => onUnderline?.()} title="Underline _text_" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs underline text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">U</button>
		<button onclick={() => onStrike?.()} title="Strikethrough +text+" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs line-through text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">S</button>
		<button onclick={() => onCode?.()} title="Code ~text~" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">~c~</button>
		<button onclick={() => onVerbatim?.()} title="Verbatim =text=" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">=v=</button>

		<!-- Separator -->
		<div class="mx-1 h-6 w-px shrink-0 bg-surface-200 dark:bg-surface-700"></div>

		<!-- Structure -->
		<button onclick={() => onLink?.()} title="Insert link (Cmd+K)" class="flex h-9 min-w-[40px] shrink-0 items-center justify-center rounded-md text-xs font-medium text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">Link</button>
		<button onclick={() => onList?.()} title="List item (- )" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&bull;</button>
		<button onclick={() => onCheckbox?.()} title="Checkbox (- [ ] )" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&#9744;</button>
		<div class="relative shrink-0">
			<button onclick={() => (showTablePicker = !showTablePicker)} title="Table" class="flex h-9 min-w-[36px] items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">|T|</button>
			{#if showTablePicker}
				<button class="fixed inset-0 z-20" onclick={() => (showTablePicker = false)} aria-label="Close"></button>
				<div class="absolute bottom-full left-0 z-30 mb-1 rounded-lg border border-surface-200 bg-surface-0 p-2 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<p class="mb-1 text-center text-[10px] text-surface-700 dark:text-surface-300">{hoverRow > 0 ? `${hoverRow} x ${hoverCol}` : 'Select size'}</p>
					<div class="grid grid-cols-5 gap-0.5">
						{#each Array(5) as _, r}
							{#each Array(5) as _, c}
								<button
									class="h-5 w-5 rounded-sm border {r < hoverRow && c < hoverCol ? 'border-mycelium-400 bg-mycelium-100 dark:bg-mycelium-900' : 'border-surface-200 dark:border-surface-700'}"
									onmouseenter={() => { hoverRow = r + 1; hoverCol = c + 1; }}
									onclick={() => { onTable?.(r + 1, c + 1); showTablePicker = false; hoverRow = 0; hoverCol = 0; }}
								></button>
							{/each}
						{/each}
					</div>
				</div>
			{/if}
		</div>
		<button onclick={() => onSrcBlock?.()} title="Code block" class="flex h-9 min-w-[40px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">SRC</button>
		<button onclick={() => onQuote?.()} title="Quote block" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&rdquo;</button>
		<button onclick={() => onTimestamp?.()} title="Timestamp" class="flex h-9 min-w-[40px] shrink-0 items-center justify-center rounded-md text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">Date</button>

		<div class="flex-1 shrink-0"></div>

		{#if editor.isDirty}
			<span class="shrink-0 text-xs text-amber-600 dark:text-amber-400">Unsaved</span>
		{/if}
	</div>
{/if}
