/// Repeating tasks: `+1w`, `++1w`, `.+1w`.
///
/// Completing a repeater does not close the task — org shifts its planning dates
/// forward and returns the headline to its first TODO state, so the next
/// occurrence stays on the agenda.

import { addHours, compareDates, shiftDate } from './date';
import { getPlanning, setPlanning } from './planning';
import { withTimestamp } from './timestamp';
import type { KeywordConfig, OrgNow, OrgTimestamp, PlanningKind, RepeaterCookie } from './types';
import { DEFAULT_KEYWORD_CONFIG } from './types';

const COOKIE_RE = /^(\+\+|\.\+|\+)(\d+)([hdwmy])$/;

/** Parse a repeater cookie such as `++1w`. Returns `null` when absent or malformed. */
export function parseRepeater(cookie: string | null | undefined): RepeaterCookie | null {
	if (!cookie) return null;
	const m = COOKIE_RE.exec(cookie);
	if (!m) return null;
	const value = Number(m[2]);
	if (!Number.isFinite(value) || value <= 0) return null;
	return {
		raw: cookie,
		type: m[1] === '++' ? 'catch-up' : m[1] === '.+' ? 'restart' : 'cumulate',
		value,
		unit: m[3] as RepeaterCookie['unit'],
	};
}

/** `true` when the timestamp carries a valid repeater cookie. */
export function hasRepeater(ts: OrgTimestamp | null | undefined): boolean {
	return parseRepeater(ts?.repeater) !== null;
}

function shiftOnce(date: string, time: string | null, cookie: RepeaterCookie): [string, string | null] {
	if (cookie.unit === 'h') {
		const shifted = addHours(date, time, cookie.value);
		return [shifted.date, shifted.time];
	}
	return [shiftDate(date, cookie.value, cookie.unit), time];
}

/**
 * Next occurrence of a repeating timestamp, per org's three flavours:
 *
 * - `+1w`  shift from the stored date by one interval
 * - `++1w` shift from the stored date, repeatedly, until strictly after today
 * - `.+1w` shift from today
 */
export function nextOccurrence(ts: OrgTimestamp, now: OrgNow): OrgTimestamp {
	const cookie = parseRepeater(ts.repeater);
	if (!cookie) return ts;

	if (cookie.type === 'restart') {
		const [date, time] = shiftOnce(now.date, ts.time ?? now.time ?? null, cookie);
		return withTimestamp(ts, { date, time });
	}

	let [date, time] = shiftOnce(ts.date, ts.time, cookie);

	if (cookie.type === 'catch-up') {
		// Bounded so a malformed cookie can never spin forever.
		for (let i = 0; i < 1000 && compareDates(date, now.date) <= 0; i += 1) {
			[date, time] = shiftOnce(date, time, cookie);
		}
	}

	return withTimestamp(ts, { date, time });
}

export interface RepeatResult {
	lines: string[];
	/** `true` when a repeater was found and the task was rescheduled instead of closed. */
	repeated: boolean;
	/** Planning kinds whose timestamps were shifted forward. */
	shifted: PlanningKind[];
}

/**
 * Apply org's completion semantics to a repeating headline: shift SCHEDULED and
 * DEADLINE to their next occurrence and leave the task open.
 *
 * Returns `repeated: false` and the lines unchanged when the headline does not
 * repeat, in which case the caller should close it normally.
 */
export function applyRepeaterOnDone(
	lines: readonly string[],
	headlineIndex: number,
	now: OrgNow
): RepeatResult {
	const planning = getPlanning(lines, headlineIndex);
	const shifted: PlanningKind[] = [];
	let next = [...lines];

	for (const kind of ['SCHEDULED', 'DEADLINE'] as const) {
		const ts = planning[kind];
		if (!ts || !hasRepeater(ts)) continue;
		next = setPlanning(next, headlineIndex, kind, nextOccurrence(ts, now));
		shifted.push(kind);
	}

	if (shifted.length === 0) return { lines: next, repeated: false, shifted };

	// A rescheduled task is not closed, so any previous CLOSED stamp is stale.
	next = setPlanning(next, headlineIndex, 'CLOSED', null);
	return { lines: next, repeated: true, shifted };
}

/** The state a repeating task returns to on completion: its first TODO keyword. */
export function repeatKeyword(config: KeywordConfig = DEFAULT_KEYWORD_CONFIG): string | null {
	return config.todoKeywords[0] ?? null;
}
