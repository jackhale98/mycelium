import {
	Language,
	LanguageSupport,
	defineLanguageFacet,
	languageDataProp,
	foldNodeProp,
	indentNodeProp,
} from '@codemirror/language';
import { parser } from './org-parser';

const orgLanguageFacet = defineLanguageFacet();

export const orgLanguage = new Language(
	orgLanguageFacet,
	parser.configure({
		props: [
			languageDataProp.add({
				Document: orgLanguageFacet,
			}),
			foldNodeProp.add({
				PropertyDrawer(node) {
					return { from: node.from, to: node.to };
				},
				Block(node) {
					return { from: node.from, to: node.to };
				},
				Drawer(node) {
					return { from: node.from, to: node.to };
				},
			}),
			indentNodeProp.add({
				ListItem: (context) => context.column(context.node.from) + 2,
			}),
		],
	}),
	[],
	'org'
);

export function org() {
	return new LanguageSupport(orgLanguage);
}
