import { describe, expect, it } from 'vitest';
import { parser } from './org-parser';
import { orgLanguage, org } from './org-language';

describe('org language', () => {
	it('loads without throwing, so highlighting actually activates', () => {
		// `Parser` has no `configure`; when this module could not call it, the
		// TypeError was swallowed by the editor's try/catch and org highlighting
		// silently never loaded.
		expect(orgLanguage).toBeDefined();
		expect(org()).toBeDefined();
	});

	it('configure returns a parser carrying the extra props', () => {
		const configured = parser.configure({ props: [] });
		expect(configured.nodeSet).toBeDefined();
		expect(typeof configured.createParse).toBe('function');
	});

	it('parses a document into a tree', () => {
		const text = '#+TITLE: Test\n* TODO [#A] Heading :tag:\nBody text\n';
		const tree = parser.parse(text);
		expect(tree.length).toBe(text.length);
	});

	it('parses an empty document', () => {
		expect(parser.parse('').length).toBe(0);
	});

	it('gives the language a parser that produces org node types', () => {
		const tree = orgLanguage.parser.parse('* Heading\n');
		expect(tree.topNode.name).toBe('Document');
	});
});
