<script lang="ts">
	import { onMount } from 'svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { orgConfig } from '$lib/stores/orgconfig.svelte';

	let {
		onBold, onItalic, onCode, onVerbatim, onUnderline, onStrike,
		onLink, onCheckbox, onHeading, onList, onSrcBlock, onQuote, onTable, onTimestamp, onImage,
		onTodo, onPriority, onDeadline, onScheduled,
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
		onImage?: () => void;
		onTodo?: (keyword: string | null) => void;
		onPriority?: (priority: string | null) => void;
		onDeadline?: () => void;
		onScheduled?: () => void;
	} = $props();

	let showHeadingPicker = $state(false);
	let showTablePicker = $state(false);
	let showTodoPicker = $state(false);
	let showPrioPicker = $state(false);
	let hoverRow = $state(0);
	let hoverCol = $state(0);
	let toolbarEl: HTMLElement;
	let keyboardOffset = $state(0);

	/** Helper: call handler via pointerdown + preventDefault to keep editor focused on iOS */
	function act(e: PointerEvent | MouseEvent, fn?: () => void) {
		e.preventDefault(); // Prevents focus steal / keyboard dismiss on iOS
		fn?.();
	}

	onMount(() => {
		const vv = window.visualViewport;

		const update = () => {
			if (vv) {
				const offset = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
				keyboardOffset = offset > 30 ? offset : 0;
			}
		};

		if (vv) {
			vv.addEventListener('resize', update);
			vv.addEventListener('scroll', update);
		}

		// Detect focus on any editable element (CodeMirror uses a div, not textarea)
		const onFocusIn = () => {
			setTimeout(update, 100);
			setTimeout(update, 300);
			setTimeout(update, 600);
		};
		const onFocusOut = (e: FocusEvent) => {
			// Don't dismiss keyboard if focus is moving to the toolbar
			const related = e.relatedTarget as HTMLElement | null;
			if (related && toolbarEl?.contains(related)) return;
			setTimeout(() => {
				// Double-check: if something in the toolbar or editor still has focus, keep offset
				if (toolbarEl?.contains(document.activeElement)) return;
				keyboardOffset = 0;
			}, 200);
		};

		document.addEventListener('focusin', onFocusIn);
		document.addEventListener('focusout', onFocusOut);

		return () => {
			if (vv) {
				vv.removeEventListener('resize', update);
				vv.removeEventListener('scroll', update);
			}
			document.removeEventListener('focusin', onFocusIn);
			document.removeEventListener('focusout', onFocusOut);
		};
	});

	const popupStyle = $derived(
		keyboardOffset > 0
			? `position:fixed;bottom:${keyboardOffset + 48}px;left:8px;right:8px;`
			: 'position:absolute;bottom:100%;left:0;'
	);
</script>

{#if editor.hasFile}
	<div
		bind:this={toolbarEl}
		class="flex h-12 items-center gap-0.5 overflow-x-auto border-t border-surface-200 bg-surface-50 px-2 dark:border-surface-700 dark:bg-surface-900"
		style="-webkit-overflow-scrolling: touch; {keyboardOffset > 0 ? `position:fixed;bottom:${keyboardOffset}px;left:0;right:0;z-index:50;` : `flex-shrink:0;padding-bottom:env(safe-area-inset-bottom, 0px);`}"
	>
		<!-- Link (primary action) -->
		<button onpointerdown={(e) => act(e, onLink)} title="Insert link (Cmd+K)" class="flex h-9 min-w-[44px] shrink-0 items-center justify-center rounded-md text-xs font-semibold hover:bg-surface-200 dark:hover:bg-surface-700" style="color:#16a34a">Link</button>

		<div class="mx-0.5 h-6 w-px shrink-0 bg-surface-200 dark:bg-surface-700"></div>

		<!-- Heading level picker -->
		<div class="relative shrink-0">
			<button onpointerdown={(e) => { e.preventDefault(); showHeadingPicker = !showHeadingPicker; }} title="Heading" class="flex h-9 min-w-[40px] items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 active:bg-surface-300 dark:text-surface-300 dark:hover:bg-surface-700">H</button>
			{#if showHeadingPicker}
				<button class="fixed inset-0 z-20" onclick={() => (showHeadingPicker = false)} aria-label="Close"></button>
				<div style="{popupStyle}" class="z-[60] mb-1 flex gap-0.5 rounded-lg border border-surface-200 bg-surface-0 p-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					{#each [1,2,3,4] as lvl}
						<button onpointerdown={(e) => { e.preventDefault(); onHeading?.(lvl); showHeadingPicker = false; }} class="flex h-8 w-8 items-center justify-center rounded text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800">H{lvl}</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- TODO keyword picker -->
		<div class="relative shrink-0">
			<button onpointerdown={(e) => { e.preventDefault(); showTodoPicker = !showTodoPicker; }} title="Set TODO state" class="flex h-9 min-w-[44px] items-center justify-center rounded-md text-[10px] font-bold text-red-600 hover:bg-surface-200 dark:text-red-400 dark:hover:bg-surface-700">TODO</button>
			{#if showTodoPicker}
				<button class="fixed inset-0 z-20" onclick={() => (showTodoPicker = false)} aria-label="Close"></button>
				<div style="{popupStyle}" class="z-[60] mb-1 min-w-[120px] rounded-lg border border-surface-200 bg-surface-0 py-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<button onpointerdown={(e) => { e.preventDefault(); onTodo?.(null); showTodoPicker = false; }} class="flex w-full px-3 py-1.5 text-xs hover:bg-surface-100 dark:hover:bg-surface-800" style="color:#6b7280">None</button>
					{#each orgConfig.todoKeywords as kw}
						<button onpointerdown={(e) => { e.preventDefault(); onTodo?.(kw); showTodoPicker = false; }} class="flex w-full px-3 py-1.5 text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800" style="color:#dc2626">{kw}</button>
					{/each}
					{#each orgConfig.doneKeywords as kw}
						<button onpointerdown={(e) => { e.preventDefault(); onTodo?.(kw); showTodoPicker = false; }} class="flex w-full px-3 py-1.5 text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800" style="color:#16a34a">{kw}</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Priority picker -->
		<div class="relative shrink-0">
			<button onpointerdown={(e) => { e.preventDefault(); showPrioPicker = !showPrioPicker; }} title="Set priority" class="flex h-9 min-w-[36px] items-center justify-center rounded-md text-[10px] font-bold text-amber-600 hover:bg-surface-200 dark:text-amber-400 dark:hover:bg-surface-700">[#]</button>
			{#if showPrioPicker}
				<button class="fixed inset-0 z-20" onclick={() => (showPrioPicker = false)} aria-label="Close"></button>
				<div style="{popupStyle}" class="z-[60] mb-1 flex gap-0.5 rounded-lg border border-surface-200 bg-surface-0 p-1 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<button onpointerdown={(e) => { e.preventDefault(); onPriority?.(null); showPrioPicker = false; }} class="flex h-8 w-8 items-center justify-center rounded text-[10px] hover:bg-surface-100 dark:hover:bg-surface-800" style="color:#6b7280">--</button>
					{#each orgConfig.priorities as p}
						<button onpointerdown={(e) => { e.preventDefault(); onPriority?.(p); showPrioPicker = false; }} class="flex h-8 w-8 items-center justify-center rounded text-xs font-bold hover:bg-surface-100 dark:hover:bg-surface-800" style="color:#ea580c">#{p}</button>
					{/each}
				</div>
			{/if}
		</div>

		<button onpointerdown={(e) => act(e, onDeadline)} title="Set DEADLINE" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-[10px] font-semibold text-red-600 hover:bg-surface-200 dark:text-red-400 dark:hover:bg-surface-700">DL</button>
		<button onpointerdown={(e) => act(e, onScheduled)} title="Set SCHEDULED" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-[10px] font-semibold text-blue-600 hover:bg-surface-200 dark:text-blue-400 dark:hover:bg-surface-700">SC</button>

		<div class="mx-0.5 h-6 w-px shrink-0 bg-surface-200 dark:bg-surface-700"></div>

		<button onpointerdown={(e) => act(e, onBold)} title="Bold" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs font-bold text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">B</button>
		<button onpointerdown={(e) => act(e, onItalic)} title="Italic" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs italic text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">I</button>
		<button onpointerdown={(e) => act(e, onUnderline)} title="Underline" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs underline text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">U</button>
		<button onpointerdown={(e) => act(e, onStrike)} title="Strikethrough" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-xs line-through text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">S</button>
		<button onpointerdown={(e) => act(e, onCode)} title="Code" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">~c~</button>
		<button onpointerdown={(e) => act(e, onVerbatim)} title="Verbatim" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">=v=</button>

		<div class="mx-0.5 h-6 w-px shrink-0 bg-surface-200 dark:bg-surface-700"></div>

		<button onpointerdown={(e) => act(e, onList)} title="List" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&bull;</button>
		<button onpointerdown={(e) => act(e, onCheckbox)} title="Checkbox" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&#9744;</button>

		<div class="relative shrink-0">
			<button onpointerdown={(e) => { e.preventDefault(); showTablePicker = !showTablePicker; }} title="Table" class="flex h-9 min-w-[36px] items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">|T|</button>
			{#if showTablePicker}
				<button class="fixed inset-0 z-20" onclick={() => (showTablePicker = false)} aria-label="Close"></button>
				<div style="{popupStyle}" class="z-[60] mb-1 rounded-lg border border-surface-200 bg-surface-0 p-2 shadow-lg dark:border-surface-700 dark:bg-surface-900">
					<p class="mb-1 text-center text-[10px] text-surface-700 dark:text-surface-300">{hoverRow > 0 ? `${hoverRow} x ${hoverCol}` : 'Select size'}</p>
					<div class="grid grid-cols-5 gap-0.5">
						{#each Array(5) as _, r}
							{#each Array(5) as _, c}
								<button
									class="h-5 w-5 rounded-sm border {r < hoverRow && c < hoverCol ? 'border-mycelium-400 bg-mycelium-100 dark:bg-mycelium-900' : 'border-surface-200 dark:border-surface-700'}"
									onmouseenter={() => { hoverRow = r + 1; hoverCol = c + 1; }}
									onpointerdown={(e) => { e.preventDefault(); onTable?.(r + 1, c + 1); showTablePicker = false; hoverRow = 0; hoverCol = 0; }}
								></button>
							{/each}
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<button onpointerdown={(e) => act(e, onSrcBlock)} title="Code block" class="flex h-9 min-w-[40px] shrink-0 items-center justify-center rounded-md font-mono text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">SRC</button>
		<button onpointerdown={(e) => act(e, onQuote)} title="Quote" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">&rdquo;</button>
		<button onpointerdown={(e) => act(e, onTimestamp)} title="Timestamp" class="flex h-9 min-w-[40px] shrink-0 items-center justify-center rounded-md text-[10px] text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">Date</button>
		<button onpointerdown={(e) => act(e, onImage)} title="Image" class="flex h-9 min-w-[36px] shrink-0 items-center justify-center rounded-md text-sm text-surface-700 hover:bg-surface-200 dark:text-surface-300 dark:hover:bg-surface-700">
			<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909M6.75 7.5a.75.75 0 11-1.5 0 .75.75 0 011.5 0zM18 3.75H6A2.25 2.25 0 003.75 6v12A2.25 2.25 0 006 20.25h12A2.25 2.25 0 0020.25 18V6A2.25 2.25 0 0018 3.75z" /></svg>
		</button>

		<div class="flex-1 shrink-0"></div>
		{#if editor.isDirty}
			<span class="shrink-0 text-xs text-amber-600 dark:text-amber-400">Unsaved</span>
		{/if}
	</div>
{/if}
