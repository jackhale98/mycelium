import {
	type CompletionContext,
	type CompletionResult,
	autocompletion,
} from '@codemirror/autocomplete';
import type { NodeRecord } from '$lib/types/node';

export interface LinkAutocompleteConfig {
	getNodes: () => NodeRecord[];
}

/// Creates a CM6 autocompletion extension that triggers on [[ and suggests org-roam nodes
export function orgLinkAutocomplete(config: LinkAutocompleteConfig) {
	return autocompletion({
		override: [
			(context: CompletionContext): CompletionResult | null => {
				// Look for [[ before cursor
				const line = context.state.doc.lineAt(context.pos);
				const textBefore = line.text.slice(0, context.pos - line.from);

				// Find the last [[ that isn't closed
				const lastOpen = textBefore.lastIndexOf('[[');
				if (lastOpen === -1) return null;

				// Check there's no ]] after it
				const afterOpen = textBefore.slice(lastOpen + 2);
				if (afterOpen.includes(']]')) return null;

				// The typed query is everything after [[
				// Strip any "id:" prefix the user might have already typed
				let query = afterOpen;
				if (query.startsWith('id:')) query = query.slice(3);

				const from = line.from + lastOpen;
				const nodes = config.getNodes();

				// Filter nodes by query
				const filtered = query
					? nodes.filter(
							(n) =>
								n.title?.toLowerCase().includes(query.toLowerCase()) ||
								n.id.toLowerCase().includes(query.toLowerCase())
						)
					: nodes;

				if (filtered.length === 0) return null;

				return {
					from,
					to: context.pos,
					options: filtered.slice(0, 20).map((node) => ({
						label: node.title ?? node.id,
						detail: node.file.split('/').pop() ?? node.file,
						apply: `[[id:${node.id}][${node.title ?? node.id}]]`,
						type: 'text',
					})),
					filter: false,
				};
			},
		],
		activateOnTyping: true,
		defaultKeymap: true,
	});
}
