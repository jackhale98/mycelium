<script lang="ts">
	import { navigation } from '$lib/stores/navigation.svelte';
	import {
		forceSimulation,
		forceLink,
		forceManyBody,
		forceCenter,
		forceCollide,
		type SimulationNodeDatum,
	} from 'd3-force';
	import { select, type Selection } from 'd3-selection';
	import { zoom, zoomIdentity } from 'd3-zoom';
	import type { GraphData } from '$lib/types/node';

	let { data }: { data: GraphData } = $props();

	let svgElement: SVGSVGElement;
	let zoomBehavior: ReturnType<typeof zoom<SVGSVGElement, unknown>>;
	let renderError = $state<string | null>(null);

	/** Labels are only drawn past this zoom scale, so text isn't painted for every node. */
	const LABEL_ZOOM_THRESHOLD = 1.2;
	let updateLabelVisibility: (scale: number) => void = () => {};

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

	type SvgSelection = Selection<SVGSVGElement, unknown, null, undefined>;

	/**
	 * `d3-transition` rides along with `d3-zoom` at runtime but ships no types here,
	 * so the animated selection is reached through this cast.
	 */
	function animated(element: SVGSVGElement, ms: number): SvgSelection {
		const selection = select(element) as SvgSelection & {
			transition(): { duration(ms: number): SvgSelection };
		};
		return selection.transition().duration(ms);
	}

	export function zoomIn() {
		if (svgElement && zoomBehavior) {
			animated(svgElement, 300).call(zoomBehavior.scaleBy, 1.5);
		}
	}

	export function zoomOut() {
		if (svgElement && zoomBehavior) {
			animated(svgElement, 300).call(zoomBehavior.scaleBy, 0.67);
		}
	}

	export function resetZoom() {
		if (!svgElement || !zoomBehavior) return;
		const g = select(svgElement).select('g');
		const gNode = g.node() as SVGGElement | null;
		if (!gNode) return;
		const bbox = gNode.getBBox();
		if (bbox.width === 0 || bbox.height === 0) return;
		const svgW = svgElement.clientWidth;
		const svgH = svgElement.clientHeight;
		const padding = 40;
		const scale = Math.min(
			(svgW - padding * 2) / bbox.width,
			(svgH - padding * 2) / bbox.height,
			2 // max scale
		);
		const tx = svgW / 2 - (bbox.x + bbox.width / 2) * scale;
		const ty = svgH / 2 - (bbox.y + bbox.height / 2) * scale;
		const transform = zoomIdentity.translate(tx, ty).scale(scale);
		animated(svgElement, 500).call(zoomBehavior.transform, transform);
	}

	$effect(() => {
		const current = data;
		if (!svgElement) return;

		renderError = null;
		select(svgElement).selectAll('*').remove();
		if (!current || current.nodes.length === 0) return;

		try {
			return render(current);
		} catch (e) {
			renderError = String(e);
		}
	});

	function render(data: GraphData) {
		const width = svgElement.clientWidth;
		const height = svgElement.clientHeight;

		const nodes: SimNode[] = data.nodes.map((n) => ({ ...n }));
		const nodeIds = new Set(nodes.map((n) => n.id));
		// d3's forceLink throws on any link referencing an id outside the node set
		const links: SimLink[] = data.links
			.filter((l) => nodeIds.has(l.source) && nodeIds.has(l.target))
			.map((l) => ({ ...l }));

		const svg = select(svgElement);
		svg.selectAll('*').remove();

		const g = svg.append('g');

		// Zoom
		zoomBehavior = zoom<SVGSVGElement, unknown>()
			.scaleExtent([0.1, 6])
			.on('zoom', (event) => {
				g.attr('transform', event.transform);
				updateLabelVisibility(event.transform.k);
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

		// Labels, hidden until the view is zoomed in far enough to read them
		const labelGroup = g.append('g').attr('display', 'none');
		const label = labelGroup
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

		let labelsVisible = false;
		const positionLabels = () => {
			label.attr('x', (d: SimNode) => d.x ?? 0).attr('y', (d: SimNode) => d.y ?? 0);
		};

		updateLabelVisibility = (scale: number) => {
			const visible = scale >= LABEL_ZOOM_THRESHOLD;
			if (visible === labelsVisible) return;
			labelsVisible = visible;
			labelGroup.attr('display', visible ? null : 'none');
			if (visible) positionLabels();
		};

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

			if (labelsVisible) positionLabels();
		});

		return () => {
			simulation.stop();
		};
	}
</script>

<div class="relative h-full w-full">
	<svg
		bind:this={svgElement}
		class="h-full w-full bg-surface-50 dark:bg-surface-900"
		style="touch-action: none"
	></svg>

	{#if !renderError && data.nodes.length === 0}
		<div class="absolute inset-0 flex items-center justify-center p-6 text-center">
			<p class="text-sm text-surface-700 dark:text-surface-300">Nothing to draw yet — add a note with an :ID: and links between notes.</p>
		</div>
	{/if}

	{#if renderError}
		<div class="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-surface-50/90 p-6 text-center dark:bg-surface-900/90">
			<p class="text-sm font-medium text-red-600 dark:text-red-400">Could not draw the graph</p>
			<p class="max-w-sm text-xs text-red-600/80 dark:text-red-400/80">{renderError}</p>
		</div>
	{/if}
</div>
