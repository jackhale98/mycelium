<script lang="ts">
	import { onMount } from 'svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import {
		forceSimulation,
		forceLink,
		forceManyBody,
		forceCenter,
		forceCollide,
		type SimulationNodeDatum,
	} from 'd3-force';
	import { select } from 'd3-selection';
	import { zoom } from 'd3-zoom';
	import type { GraphData } from '$lib/types/node';

	let { data }: { data: GraphData } = $props();

	let svgElement: SVGSVGElement;
	let zoomBehavior: ReturnType<typeof zoom<SVGSVGElement, unknown>>;

	interface SimNode extends SimulationNodeDatum {
		id: string;
		title: string | null;
		tags: string[];
		link_count: number;
	}

	interface SimLink {
		source: string | SimNode;
		target: string | SimNode;
	}

	// Tag-based color palette
	const tagColors: Record<string, string> = {};
	const palette = [
		'#22c55e', '#3b82f6', '#a855f7', '#f59e0b', '#ef4444',
		'#06b6d4', '#ec4899', '#84cc16', '#f97316', '#6366f1',
	];
	let colorIdx = 0;

	function nodeColor(tags: string[]): string {
		if (tags.length === 0) return '#22c55e';
		const primary = tags[0];
		if (!tagColors[primary]) {
			tagColors[primary] = palette[colorIdx % palette.length];
			colorIdx++;
		}
		return tagColors[primary];
	}

	function nodeRadius(linkCount: number): number {
		return Math.max(4, Math.min(16, 4 + Math.sqrt(linkCount) * 3));
	}

	export function zoomIn() {
		if (svgElement && zoomBehavior) {
			select(svgElement).transition().duration(300).call(zoomBehavior.scaleBy, 1.5);
		}
	}

	export function zoomOut() {
		if (svgElement && zoomBehavior) {
			select(svgElement).transition().duration(300).call(zoomBehavior.scaleBy, 0.67);
		}
	}

	export function resetZoom() {
		if (svgElement && zoomBehavior) {
			select(svgElement).transition().duration(500).call(zoomBehavior.scaleTo, 1);
		}
	}

	onMount(() => {
		if (!data || data.nodes.length === 0) return;

		const width = svgElement.clientWidth;
		const height = svgElement.clientHeight;

		const nodes: SimNode[] = data.nodes.map((n) => ({ ...n }));
		const links: SimLink[] = data.links.map((l) => ({ ...l }));

		const svg = select(svgElement);
		svg.selectAll('*').remove();

		const g = svg.append('g');

		// Zoom
		zoomBehavior = zoom<SVGSVGElement, unknown>()
			.scaleExtent([0.1, 6])
			.on('zoom', (event) => {
				g.attr('transform', event.transform);
			});
		svg.call(zoomBehavior);

		// Links
		const link = g
			.append('g')
			.selectAll('line')
			.data(links)
			.join('line')
			.attr('stroke', '#94a3b8')
			.attr('stroke-width', 0.8)
			.attr('stroke-opacity', 0.4);

		// Nodes
		const node = g
			.append('g')
			.selectAll('circle')
			.data(nodes)
			.join('circle')
			.attr('r', (d: SimNode) => nodeRadius(d.link_count))
			.attr('fill', (d: SimNode) => nodeColor(d.tags))
			.attr('stroke', '#fff')
			.attr('stroke-width', 1.5)
			.style('cursor', 'pointer')
			.on('click', (_event: MouseEvent, d: SimNode) => {
				navigation.navigateToNode(d.id);
			});

		// Hover: show title tooltip
		node.append('title').text((d: SimNode) => d.title ?? d.id);

		// Labels for all nodes
		const label = g
			.append('g')
			.selectAll('text')
			.data(nodes)
			.join('text')
			.text((d: SimNode) => {
				const t = d.title ?? d.id.slice(0, 8);
				return t.length > 20 ? t.slice(0, 18) + '...' : t;
			})
			.attr('font-size', 9)
			.attr('dx', (d: SimNode) => nodeRadius(d.link_count) + 4)
			.attr('dy', 3)
			.attr('fill', '#64748b')
			.attr('pointer-events', 'none');

		// Simulation
		const simulation = forceSimulation(nodes)
			.force(
				'link',
				forceLink(links)
					.id((d) => (d as SimNode).id)
					.distance(60)
			)
			.force('charge', forceManyBody().strength(-150))
			.force('center', forceCenter(width / 2, height / 2))
			.force('collide', forceCollide().radius((d) => nodeRadius((d as SimNode).link_count) + 4));

		simulation.on('tick', () => {
			link
				.attr('x1', (d: SimLink) => (d.source as SimNode).x ?? 0)
				.attr('y1', (d: SimLink) => (d.source as SimNode).y ?? 0)
				.attr('x2', (d: SimLink) => (d.target as SimNode).x ?? 0)
				.attr('y2', (d: SimLink) => (d.target as SimNode).y ?? 0);

			node
				.attr('cx', (d: SimNode) => d.x ?? 0)
				.attr('cy', (d: SimNode) => d.y ?? 0);

			label
				.attr('x', (d: SimNode) => d.x ?? 0)
				.attr('y', (d: SimNode) => d.y ?? 0);
		});

		return () => {
			simulation.stop();
		};
	});
</script>

<svg
	bind:this={svgElement}
	class="h-full w-full bg-surface-50 dark:bg-surface-900"
	style="touch-action: none"
></svg>
