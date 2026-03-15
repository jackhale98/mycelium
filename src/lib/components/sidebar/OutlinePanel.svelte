<script lang="ts">
	let { content = '' }: { content?: string } = $props();

	// Extract headlines from org content for outline
	const headings = $derived(
		content
			.split('\n')
			.filter((line) => /^\*+ /.test(line))
			.map((line) => {
				const match = line.match(/^(\*+)\s+(.*)$/);
				if (!match) return null;
				return {
					level: match[1].length,
					text: match[2].replace(/\s+:[\w:]+:\s*$/, ''), // Strip tags
				};
			})
			.filter(Boolean) as { level: number; text: string }[]
	);
</script>

{#if headings.length > 0}
	<div class="rounded-lg border border-surface-200 p-3 dark:border-surface-700">
		<h3 class="mb-2 text-xs font-semibold uppercase text-surface-700 dark:text-surface-300">
			Outline
		</h3>
		<ul class="space-y-1">
			{#each headings as heading}
				<li style="padding-left: {(heading.level - 1) * 12}px">
					<span class="text-xs text-surface-700 dark:text-surface-300">{heading.text}</span>
				</li>
			{/each}
		</ul>
	</div>
{/if}
