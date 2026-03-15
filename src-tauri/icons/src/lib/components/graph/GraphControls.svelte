<script lang="ts">
	let { onZoomIn, onZoomOut, onReset }: {
		onZoomIn?: () => void;
		onZoomOut?: () => void;
		onReset?: () => void;
	} = $props();
</script>

<div class="absolute bottom-4 right-4 flex flex-col gap-1">
	<button
		onclick={() => onZoomIn?.()}
		class="rounded-lg bg-surface-0 p-2 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700"
		title="Zoom in"
	>
		<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
			<path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
		</svg>
	</button>
	<button
		onclick={() => onZoomOut?.()}
		class="rounded-lg bg-surface-0 p-2 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700"
		title="Zoom out"
	>
		<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
			<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 12h-15" />
		</svg>
	</button>
	<button
		onclick={() => onReset?.()}
		class="rounded-lg bg-surface-0 p-2 shadow-md hover:bg-surface-100 dark:bg-surface-800 dark:hover:bg-surface-700"
		title="Reset view"
	>
		<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
			<path stroke-linecap="round" stroke-linejoin="round" d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
		</svg>
	</button>
</div>
