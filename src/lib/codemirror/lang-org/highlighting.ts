import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';
import { EditorView } from '@codemirror/view';
import { org } from './org-language';

const orgHighlightStyle = HighlightStyle.define([
	{ tag: t.heading1, fontWeight: 'bold', fontSize: '1.4em', color: '#15803d' },
	{ tag: t.heading2, fontWeight: 'bold', fontSize: '1.25em', color: '#166534' },
	{ tag: t.heading3, fontWeight: 'bold', fontSize: '1.1em', color: '#14532d' },
	{ tag: t.heading4, fontWeight: '600', fontSize: '1.05em', color: '#1a4731' },
	{ tag: t.heading5, fontWeight: '600', color: '#1a4731' },
	{ tag: t.keyword, color: '#dc2626', fontWeight: 'bold' },
	{ tag: t.atom, color: '#ea580c', fontWeight: 'bold' },
	{ tag: t.tagName, color: '#7c3aed', fontStyle: 'italic' },
	{ tag: t.meta, color: '#6b7280', fontStyle: 'italic' },
	{ tag: t.special(t.meta), color: '#9ca3af', fontSize: '0.9em' },
	{ tag: t.attributeName, color: '#0891b2' },
	{ tag: t.attributeValue, color: '#0d9488' },
	{ tag: t.blockComment, backgroundColor: '#f8fafc' },
	{ tag: t.processingInstruction, color: '#7c3aed', fontWeight: '500' },
	{ tag: t.content, color: 'inherit' },
	{ tag: t.link, color: '#16a34a', textDecoration: 'underline', cursor: 'pointer' },
	{ tag: t.url, color: '#0891b2' },
	{ tag: t.labelName, color: '#16a34a', textDecoration: 'underline' },
	{ tag: t.strong, fontWeight: 'bold' },
	{ tag: t.emphasis, fontStyle: 'italic' },
	{ tag: t.special(t.emphasis), textDecoration: 'underline' },
	{ tag: t.strikethrough, textDecoration: 'line-through', color: '#9ca3af' },
	{ tag: t.monospace, fontFamily: 'monospace', backgroundColor: '#f1f5f9', padding: '0 2px', borderRadius: '2px', color: '#dc2626' },
	{ tag: t.literal, fontFamily: 'monospace', backgroundColor: '#f1f5f9', padding: '0 2px', borderRadius: '2px' },
	{ tag: t.list, color: 'inherit' },
	{ tag: t.punctuation, color: '#3b82f6', fontWeight: 'bold' },
	{ tag: t.separator, color: '#d1d5db' },
	{ tag: t.special(t.string), color: '#7c3aed' },
	{ tag: t.brace, color: '#9ca3af' },
	{ tag: t.lineComment, color: '#9ca3af', fontStyle: 'italic' },
]);

const darkOrgHighlightStyle = HighlightStyle.define([
	{ tag: t.heading1, fontWeight: 'bold', fontSize: '1.4em', color: '#4ade80' },
	{ tag: t.heading2, fontWeight: 'bold', fontSize: '1.25em', color: '#86efac' },
	{ tag: t.heading3, fontWeight: 'bold', fontSize: '1.1em', color: '#bbf7d0' },
	{ tag: t.heading4, fontWeight: '600', fontSize: '1.05em', color: '#dcfce7' },
	{ tag: t.heading5, fontWeight: '600', color: '#dcfce7' },
	{ tag: t.keyword, color: '#f87171', fontWeight: 'bold' },
	{ tag: t.atom, color: '#fb923c', fontWeight: 'bold' },
	{ tag: t.tagName, color: '#a78bfa', fontStyle: 'italic' },
	{ tag: t.meta, color: '#9ca3af', fontStyle: 'italic' },
	{ tag: t.special(t.meta), color: '#6b7280', fontSize: '0.9em' },
	{ tag: t.attributeName, color: '#22d3ee' },
	{ tag: t.attributeValue, color: '#2dd4bf' },
	{ tag: t.processingInstruction, color: '#a78bfa', fontWeight: '500' },
	{ tag: t.link, color: '#4ade80', textDecoration: 'underline', cursor: 'pointer' },
	{ tag: t.url, color: '#22d3ee' },
	{ tag: t.labelName, color: '#4ade80', textDecoration: 'underline' },
	{ tag: t.strong, fontWeight: 'bold' },
	{ tag: t.emphasis, fontStyle: 'italic' },
	{ tag: t.special(t.emphasis), textDecoration: 'underline' },
	{ tag: t.strikethrough, textDecoration: 'line-through', color: '#6b7280' },
	{ tag: t.monospace, fontFamily: 'monospace', backgroundColor: '#1e293b', padding: '0 2px', borderRadius: '2px', color: '#f87171' },
	{ tag: t.literal, fontFamily: 'monospace', backgroundColor: '#1e293b', padding: '0 2px', borderRadius: '2px' },
	{ tag: t.punctuation, color: '#60a5fa', fontWeight: 'bold' },
	{ tag: t.separator, color: '#4b5563' },
	{ tag: t.special(t.string), color: '#a78bfa' },
	{ tag: t.brace, color: '#6b7280' },
	{ tag: t.lineComment, color: '#6b7280', fontStyle: 'italic' },
]);

const orgBaseTheme = EditorView.baseTheme({
	'.cm-line': {
		lineHeight: '1.6',
	},
});

export function orgHighlighting() {
	return [
		org(),
		syntaxHighlighting(orgHighlightStyle),
		syntaxHighlighting(darkOrgHighlightStyle),
		orgBaseTheme,
	];
}
