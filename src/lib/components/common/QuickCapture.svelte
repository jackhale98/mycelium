<script lang="ts">
	import { quickCapture } from '$lib/tauri/commands';

	let open = $state(false);
	let text = $state('');
	let saving = $state(false);
	let saved = $state(false);

	async function handleSubmit() {
		if (!text.trim()) return;
		saving = true;
		try {
			await quickCapture(text.trim());
			text = '';
			saved = true;
			setTimeout(() => { saved = false; open = false; }, 800);
		} catch (e) {
			alert(String(e));
		} finally {
			saving = false;
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handleSubmit();
		if (e.key === 'Escape') open = false;
	}
</script>

<!-- Floating capture button -->
{#if !open}
	<button
		onclick={() => { open = true; saved = false; }}
		class="fixed bottom-20 right-4 z-10 flex h-12 w-12 items-center justify-center rounded-full shadow-lg lg:bottom-6"
		style="background:#16a34a;color:white"
		aria-label="Quick capture"
	>
		<svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
			<path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
		</svg>
	</button>
{/if}

<!-- Capture sheet -->
{#if open}
	<button class="fixed inset-0 z-40 bg-black/40" onclick={() => (open = false)} aria-label="Close"></button>
	<div class="fixed inset-x-0 bottom-0 z-50 rounded-t-2xl border-t bg-surface-0 p-4 shadow-2xl dark:border-surface-700 dark:bg-surface-900" style="padding-bottom: calc(1rem + env(safe-area-inset-bottom, 0px))">
		{#if saved}
			<div class="py-4 text-center">
				<span style="color:#16a34a;font-size:1.5rem">&#10003;</span>
				<p class="mt-1 text-sm font-medium" style="color:#16a34a">Captured to daily note</p>
			</div>
		{:else}
			<div class="mb-2 flex items-center justify-between">
				<span class="text-sm font-semibold">Quick Capture</span>
				<span class="text-[10px] text-surface-700 dark:text-surface-300">Saves to today's daily note</span>
			</div>
			<textarea
				bind:value={text}
				onkeydown={handleKeydown}
				placeholder="Jot down a thought..."
				rows="3"
				class="w-full resize-none rounded-lg border border-surface-200 bg-surface-50 p-3 text-sm focus:border-mycelium-500 focus:outline-none focus:ring-2 focus:ring-mycelium-500/20 dark:border-surface-700 dark:bg-surface-950"
				autofocus
			></textarea>
			<div class="mt-2 flex items-center justify-between">
				<span class="text-[10px] text-surface-700 dark:text-surface-300">Cmd+Enter to save</span>
				<button
					onclick={handleSubmit}
					disabled={saving || !text.trim()}
					class="rounded-lg px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
					style="background:#16a34a"
				>
					{saving ? 'Saving...' : 'Capture'}
				</button>
			</div>
		{/if}
	</div>
{/if}
