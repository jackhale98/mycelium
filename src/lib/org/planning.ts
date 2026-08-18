/// Planning lines: `SCHEDULED: <…>`, `DEADLINE: <…>`, `CLOSED: […]`.
///
/// Org allows exactly ONE planning line, immediately after the headline and
/// BEFORE any property drawer. Everything here is bounded to that single line so
/// body text can never be mistaken for planning and rewritten.

import { isHeadlineLine } from './headline';
import { formatTimestamp, parseTimestampInput, readTimestamp } from './timestamp';
import { PLANNING_ORDER } from './types';
import type { OrgNow, OrgTimestamp, PlanningKind } from './types';

const PLANNING_KEYWORD_RE = /^(SCHEDULED|DEADLINE|CLOSED):[ \t]*/;

export interface PlanningEntry {
	kind: PlanningKind;
	timestamp: OrgTimestamp;
}

export interface ParsedPlanningLine {
	entries: PlanningEntry[];
	/** Leading whitespace, preserved on rewrite. */
	indent: string;
	/** `\r` when the source used CRLF line endings, otherwise `''`. */
	eol: string;
}

function splitEol(line: string): [string, string] {
	return line.endsWith('\r') ? [line.slice(0, -1), '\r'] : [line, ''];
}

/**
 * Parse a planning line. Returns `null` unless the line consists solely of
 * planning entries — a sentence merely containing the word "DEADLINE:" is body
 * text, not planning.
 */
export function parsePlanningLine(line: string): ParsedPlanningLine | null {
	const [text, eol] = splitEol(line);
	const indentMatch = /^[ \t]*/.exec(text)!;
	const indent = indentMatch[0];
	let rest = text.slice(indent.length);
	if (rest.length === 0) return null;

	const entries: PlanningEntry[] = [];
	while (rest.length > 0) {
		const kwMatch = PLANNING_KEYWORD_RE.exec(rest);
		if (!kwMatch) return null;
		const kind = kwMatch[1] as PlanningKind;
		rest = rest.slice(kwMatch[0].length);

		const read = readTimestamp(rest, 0);
		if (!read) return null;
		if (entries.some((e) => e.kind === kind)) return null;
		entries.push({ kind, timestamp: read.timestamp });

		rest = rest.slice(read.end).replace(/^[ \t]+/, '');
	}

	return entries.length > 0 ? { entries, indent, eol } : null;
}

/** `true` when the line is a planning line. */
export function isPlanningLine(line: string): boolean {
	return parsePlanningLine(line) !== null;
}

/** Render planning entries in org's own order: CLOSED, DEADLINE, SCHEDULED. */
export function formatPlanningLine(planning: ParsedPlanningLine): string {
	const ordered = [...planning.entries].sort(
		(a, b) => PLANNING_ORDER.indexOf(a.kind) - PLANNING_ORDER.indexOf(b.kind)
	);
	const body = ordered.map((e) => `${e.kind}: ${formatTimestamp(e.timestamp)}`).join(' ');
	return `${planning.indent}${body}${planning.eol}`;
}

/**
 * Index of the planning line belonging to `headlineIndex`, or `null`.
 * Only the line directly after the headline qualifies.
 */
export function findPlanningLineIndex(
	lines: readonly string[],
	headlineIndex: number
): number | null {
	const candidate = headlineIndex + 1;
	if (candidate >= lines.length) return null;
	if (isHeadlineLine(lines[candidate])) return null;
	return isPlanningLine(lines[candidate]) ? candidate : null;
}

/** The planning timestamps of a headline, keyed by kind. */
export function getPlanning(
	lines: readonly string[],
	headlineIndex: number
): Record<PlanningKind, OrgTimestamp | null> {
	const result: Record<PlanningKind, OrgTimestamp | null> = {
		SCHEDULED: null,
		DEADLINE: null,
		CLOSED: null,
	};
	const index = findPlanningLineIndex(lines, headlineIndex);
	if (index === null) return result;
	const parsed = parsePlanningLine(lines[index]);
	if (!parsed) return result;
	for (const entry of parsed.entries) result[entry.kind] = entry.timestamp;
	return result;
}

/** A single planning timestamp of a headline, or `null`. */
export function getPlanningTimestamp(
	lines: readonly string[],
	headlineIndex: number,
	kind: PlanningKind
): OrgTimestamp | null {
	return getPlanning(lines, headlineIndex)[kind];
}

/**
 * Set or remove one planning entry, leaving the others on the line untouched.
 *
 * Passing `null` removes only `kind`; a combined
 * `SCHEDULED: <…> DEADLINE: <…>` line keeps the entry that was not targeted.
 * A new planning line is inserted directly after the headline, before any
 * property drawer, which is the only position org recognises.
 */
export function setPlanning(
	lines: readonly string[],
	headlineIndex: number,
	kind: PlanningKind,
	timestamp: OrgTimestamp | null
): string[] {
	const next = [...lines];
	if (headlineIndex < 0 || headlineIndex >= next.length) return next;
	if (!isHeadlineLine(next[headlineIndex])) return next;

	const index = findPlanningLineIndex(next, headlineIndex);

	if (index === null) {
		if (!timestamp) return next;
		const eol = splitEol(next[headlineIndex])[1];
		const line = formatPlanningLine({ entries: [{ kind, timestamp }], indent: '', eol });
		next.splice(headlineIndex + 1, 0, line);
		return next;
	}

	const parsed = parsePlanningLine(next[index])!;
	const entries = parsed.entries.filter((e) => e.kind !== kind);
	if (timestamp) entries.push({ kind, timestamp });

	if (entries.length === 0) {
		next.splice(index, 1);
		return next;
	}

	next[index] = formatPlanningLine({ ...parsed, entries });
	return next;
}

/**
 * Set a planning date from user input (a date picker value or a full timestamp),
 * inheriting the repeater and warning cookies of the entry being replaced.
 *
 * Cookies are taken from the SAME entry only: editing SCHEDULED on a combined
 * line never copies DEADLINE's `+1w -2d` onto it.
 */
export function setPlanningDate(
	lines: readonly string[],
	headlineIndex: number,
	kind: PlanningKind,
	input: string
): string[] {
	const base = getPlanningTimestamp(lines, headlineIndex, kind);
	const timestamp = parseTimestampInput(input, base, { active: kind !== 'CLOSED' });
	if (!timestamp) return [...lines];
	return setPlanning(lines, headlineIndex, kind, timestamp);
}

/** Remove one planning entry, keeping any others on the same line. */
export function removePlanning(
	lines: readonly string[],
	headlineIndex: number,
	kind: PlanningKind
): string[] {
	return setPlanning(lines, headlineIndex, kind, null);
}

/**
 * Write or clear the inactive `CLOSED: [date time]` stamp org records on
 * completion. Passing `null` for `now` removes it.
 */
export function setClosed(
	lines: readonly string[],
	headlineIndex: number,
	now: OrgNow | null
): string[] {
	if (!now) return setPlanning(lines, headlineIndex, 'CLOSED', null);
	const timestamp = parseTimestampInput(
		now.time ? `${now.date} ${now.time}` : now.date,
		null,
		{ active: false }
	);
	if (!timestamp) return [...lines];
	return setPlanning(lines, headlineIndex, 'CLOSED', timestamp);
}
