/// Headline parsing and editing: stars, TODO keyword, priority cookie, tags.

import { DEFAULT_KEYWORD_CONFIG, DEFAULT_PRIORITY_CONFIG } from './types';
import type { KeywordConfig, PriorityConfig } from './types';

/** Org tag characters: alphanumerics plus `_ @ # %` (Unicode-aware). */
const TAG_CHAR_RE = /^[\p{L}\p{N}_@#%]+$/u;
const TAG_BLOCK_RE = /(^|[ \t])(:(?:[\p{L}\p{N}_@#%]+:)+)[ \t]*$/u;
const PRIORITY_COOKIE_RE = /^\[#([^\]\s]+)\][ \t]*/;

export interface ParsedHeadline {
	/** The leading stars, e.g. `**`. */
	stars: string;
	/** Number of stars. */
	level: number;
	/** Whitespace between the stars and the content. */
	spacing: string;
	keyword: string | null;
	priority: string | null;
	title: string;
	tags: string[];
	/** Whitespace between the title and the tag block, preserved on rewrite. */
	tagSpacing: string;
	/** `\r` when the source used CRLF line endings, otherwise `''`. */
	eol: string;
}

export function escapeRegExp(text: string): string {
	return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function splitEol(line: string): [string, string] {
	return line.endsWith('\r') ? [line.slice(0, -1), '\r'] : [line, ''];
}

/**
 * A headline is column-0 stars followed by whitespace. `**` on its own and an
 * indented `  * bullet` are list items or plain text, never headlines.
 */
export function isHeadlineLine(line: string): boolean {
	const [text] = splitEol(line);
	return /^\*+[ \t]/.test(text);
}

/** Star count of a headline, or `0` when the line is not a headline. */
export function headlineLevel(line: string): number {
	if (!isHeadlineLine(line)) return 0;
	return /^\*+/.exec(splitEol(line)[0])![0].length;
}

function allKeywords(config: KeywordConfig): string[] {
	// Longest first so `TODO` cannot shadow a keyword like `TODOLATER`.
	return [...config.todoKeywords, ...config.doneKeywords]
		.filter((k) => k.length > 0)
		.sort((a, b) => b.length - a.length);
}

/** Regex matching a configured keyword at the start of a headline's content. */
export function todoKeywordRegex(config: KeywordConfig = DEFAULT_KEYWORD_CONFIG): RegExp {
	const keywords = allKeywords(config).map(escapeRegExp);
	if (keywords.length === 0) return /(?!)/;
	// The keyword must be followed by whitespace or end the line: `* TODO` alone
	// is a keyword with an empty title, not a title reading "TODO".
	return new RegExp(`^(${keywords.join('|')})(?=[ \\t]|$)`);
}

/** Parse a headline line. Returns `null` when the line is not a headline. */
export function parseHeadline(
	line: string,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): ParsedHeadline | null {
	const [text, eol] = splitEol(line);
	const m = /^(\*+)([ \t]+)(.*)$/.exec(text);
	if (!m) return null;
	const stars = m[1];
	const spacing = m[2];
	let rest = m[3];

	let keyword: string | null = null;
	const kwMatch = todoKeywordRegex(config).exec(rest);
	if (kwMatch) {
		keyword = kwMatch[1];
		rest = rest.slice(kwMatch[0].length).replace(/^[ \t]+/, '');
	}

	// Strip every priority cookie so stale/duplicated cookies never accumulate.
	let priority: string | null = null;
	for (;;) {
		const prioMatch = PRIORITY_COOKIE_RE.exec(rest);
		if (!prioMatch) break;
		if (priority === null) priority = prioMatch[1];
		rest = rest.slice(prioMatch[0].length);
	}

	let tags: string[] = [];
	let tagSpacing = '';
	const tagMatch = TAG_BLOCK_RE.exec(rest);
	if (tagMatch) {
		tags = tagMatch[2].split(':').filter((t) => t.length > 0);
		tagSpacing = tagMatch[1] === '' ? '' : ' ';
		rest = rest.slice(0, tagMatch.index);
	}

	return {
		stars,
		level: stars.length,
		spacing,
		keyword,
		priority,
		title: rest.trim(),
		tags,
		tagSpacing,
		eol,
	};
}

/** Render a headline back to text: `** TODO [#A] Title :tag1:tag2:`. */
export function formatHeadline(headline: ParsedHeadline): string {
	const parts: string[] = [];
	if (headline.keyword) parts.push(headline.keyword);
	if (headline.priority) parts.push(`[#${headline.priority}]`);
	if (headline.title) parts.push(headline.title);
	let body = parts.join(' ');
	if (headline.tags.length > 0) {
		const block = `:${headline.tags.join(':')}:`;
		body = body ? `${body}${headline.tagSpacing || ' '}${block}` : block;
	}
	// An empty headline keeps its trailing space; `*` alone would stop being a headline.
	const spacing = headline.spacing || ' ';
	return `${headline.stars}${spacing}${body}${headline.eol}`;
}

/** The keyword on a headline, or `null`. */
export function getTodoKeyword(
	line: string,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): string | null {
	return parseHeadline(line, config)?.keyword ?? null;
}

/** The priority cookie value on a headline, or `null`. */
export function getPriority(line: string, config: KeywordConfig = DEFAULT_KEYWORD_CONFIG): string | null {
	return parseHeadline(line, config)?.priority ?? null;
}

export function isDoneKeyword(
	keyword: string | null,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): boolean {
	return keyword !== null && config.doneKeywords.includes(keyword);
}

export function isTodoKeyword(
	keyword: string | null,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): boolean {
	return keyword !== null && config.todoKeywords.includes(keyword);
}

/**
 * Set (or with `null` clear) the TODO keyword on a headline line.
 * Non-headline lines are returned unchanged.
 */
export function setTodoKeyword(
	line: string,
	keyword: string | null,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): string {
	const parsed = parseHeadline(line, config);
	if (!parsed) return line;
	return formatHeadline({ ...parsed, keyword: keyword || null });
}

/**
 * The keyword that follows `current` in the configured cycle
 * `todo… → done… → none → todo…`. `direction` `-1` walks backwards.
 */
export function nextTodoKeyword(
	current: string | null,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG,
	direction: 1 | -1 = 1
): string | null {
	const cycle: (string | null)[] = [...config.todoKeywords, ...config.doneKeywords, null];
	if (cycle.length === 1) return null;
	const index = cycle.indexOf(current);
	if (index === -1) return direction === 1 ? cycle[0] : cycle[cycle.length - 2];
	return cycle[(index + direction + cycle.length) % cycle.length];
}

/** Advance the headline's keyword one step through the configured cycle. */
export function cycleTodoKeyword(
	line: string,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG,
	direction: 1 | -1 = 1
): string {
	const parsed = parseHeadline(line, config);
	if (!parsed) return line;
	return formatHeadline({ ...parsed, keyword: nextTodoKeyword(parsed.keyword, config, direction) });
}

/** `true` when `priority` is one of the configured values. */
export function isValidPriority(
	priority: string,
	config: PriorityConfig = DEFAULT_PRIORITY_CONFIG
): boolean {
	return config.priorities.includes(priority);
}

/**
 * Set (or with `null` clear) the priority cookie, dropping any existing
 * cookies first so custom values never stack up as `[#2] [#1] Title`.
 */
export function setPriority(
	line: string,
	priority: string | null,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): string {
	const parsed = parseHeadline(line, config);
	if (!parsed) return line;
	const value = priority === null || priority === '' ? null : priority.trim();
	if (value !== null && (value === '' || /[\]\s]/.test(value))) return line;
	return formatHeadline({ ...parsed, priority: value });
}

/** The next priority in the configured list; wraps through "no priority". */
export function nextPriority(
	current: string | null,
	config: PriorityConfig = DEFAULT_PRIORITY_CONFIG,
	direction: 1 | -1 = 1
): string | null {
	const cycle: (string | null)[] = [...config.priorities, null];
	if (cycle.length === 1) return null;
	const index = cycle.indexOf(current);
	if (index === -1) return direction === 1 ? cycle[0] : cycle[cycle.length - 2];
	return cycle[(index + direction + cycle.length) % cycle.length];
}

/** Tags written at the end of a headline. */
export function getHeadlineTags(
	line: string,
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): string[] {
	return parseHeadline(line, config)?.tags ?? [];
}

/** Replace the headline's tag block. An empty list removes it. */
export function setHeadlineTags(
	line: string,
	tags: readonly string[],
	config: KeywordConfig = DEFAULT_KEYWORD_CONFIG
): string {
	const parsed = parseHeadline(line, config);
	if (!parsed) return line;
	return formatHeadline({ ...parsed, tags: tags.filter((t) => TAG_CHAR_RE.test(t)) });
}

/** Index of the nearest headline at or above `fromIndex`, or `-1`. */
export function findHeadlineIndex(lines: readonly string[], fromIndex: number): number {
	for (let i = Math.min(fromIndex, lines.length - 1); i >= 0; i--) {
		if (isHeadlineLine(lines[i])) return i;
	}
	return -1;
}

/** Index of the line containing character offset `offset` in `lines.join('\n')`. */
export function lineIndexAtOffset(lines: readonly string[], offset: number): number {
	if (offset <= 0) return 0;
	let count = 0;
	for (let i = 0; i < lines.length; i++) {
		count += lines[i].length + 1;
		if (count > offset) return i;
	}
	return Math.max(0, lines.length - 1);
}

/** Index of the first line after the headline's own subtree. */
export function findSubtreeEnd(lines: readonly string[], headlineIndex: number): number {
	const level = headlineLevel(lines[headlineIndex] ?? '');
	if (level === 0) return headlineIndex + 1;
	for (let i = headlineIndex + 1; i < lines.length; i++) {
		const other = headlineLevel(lines[i]);
		if (other > 0 && other <= level) return i;
	}
	return lines.length;
}
