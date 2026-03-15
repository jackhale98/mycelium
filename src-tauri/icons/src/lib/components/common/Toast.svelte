<script lang="ts">
	let { message, type = 'info', visible = false }: {
		message: string;
		type?: 'info' | 'success' | 'error';
		visible?: boolean;
	} = $props();

	const colors = {
		info: 'bg-surface-800 text-white dark:bg-surface-200 dark:text-surface-900',
		success: 'bg-mycelium-600 text-white',
		error: 'bg-red-600 text-white',
	};
</script>

{#if visible}
	<div
		class="fixed bottom-20 left-1/2 z-50 -translate-x-1/2 transform rounded-lg px-4 py-2.5 text-sm font-medium shadow-lg {colors[type]}"
	>
		{message}
	</div>
{/if}
