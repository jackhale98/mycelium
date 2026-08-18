import { describe, it, expect } from 'vitest';
import { escapeHtml, highlightSnippet } from './snippet';

describe('escapeHtml', () => {
	it('escapes all five HTML-significant characters', () => {
		expect(escapeHtml(`&<>"'`)).toBe('&amp;&lt;&gt;&quot;&#39;');
	});

	it('escapes ampersands before the entities it introduces', () => {
		expect(escapeHtml('a & <b>')).toBe('a &amp; &lt;b&gt;');
	});
});

describe('highlightSnippet', () => {
	it('wraps <<match>> markers in a mark element', () => {
		expect(highlightSnippet('some <<match>> here')).toBe(
			'some <mark class="bg-mycelium-200 dark:bg-mycelium-800 rounded px-0.5">match</mark> here'
		);
	});

	it('neutralises HTML embedded in the note body', () => {
		const out = highlightSnippet('<img src=x onerror=alert(document.cookie)>');
		expect(out).not.toContain('<img');
		expect(out).toContain('&lt;img src=x onerror=alert(document.cookie)&gt;');
	});

	it('neutralises HTML inside a highlighted match', () => {
		const out = highlightSnippet('<<<script>alert(1)</script>>>');
		expect(out).not.toContain('<script');
		expect(out).toContain('<mark');
	});

	it('does not let a note close the mark element early', () => {
		const out = highlightSnippet('<<a</mark><img src=x onerror=alert(1)>b>>');
		expect(out).not.toContain('</mark><img');
		expect(out.match(/<mark /g)?.length).toBe(1);
	});

	it('leaves unmarked text intact', () => {
		expect(highlightSnippet('plain text')).toBe('plain text');
	});

	it('handles multiple matches', () => {
		const out = highlightSnippet('<<one>> and <<two>>');
		expect(out.match(/<mark /g)?.length).toBe(2);
	});
});
