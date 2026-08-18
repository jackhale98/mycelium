/// Parsing and rendering of org timestamps: `<2024-01-15 Mon 09:00 ++1w -2d>`.

import { dayNameFor, formatTime, isValidDate, parseTime } from './date';
import type { OrgTimestamp } from './types';

const TIMESTAMP_DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const REPEATER_RE = /^(\+\+|\.\+|\+)(\d+)([hdwmy])$/;
const WARNING_RE = /^(--|-)(\d+)([hdwmy])$/;
const TIME_RANGE_RE = /^(\d{1,2}:\d{2})(?:-(\d{1,2}:\d{2}))?$/;

/** Result of reading a timestamp out of a longer string. */
export interface TimestampRead {
	timestamp: OrgTimestamp;
	/** Index just past the closing delimiter. */
	end: number;
}

/**
 * Read a timestamp starting at `from`. Returns `null` when the text at `from`
 * is not an org timestamp (wrong delimiter, unterminated, or no leading date).
 */
export function readTimestamp(text: string, from = 0): TimestampRead | null {
	const open = text[from];
	if (open !== '<' && open !== '[') return null;
	const active = open === '<';
	const close = active ? '>' : ']';
	const closeIndex = text.indexOf(close, from + 1);
	if (closeIndex === -1) return null;

	const raw = text.slice(from, closeIndex + 1);
	const inner = text.slice(from + 1, closeIndex);
	const parts = inner.split(/\s+/).filter((p) => p.length > 0);
	if (parts.length === 0) return null;
	const date = parts[0];
	if (!TIMESTAMP_DATE_RE.test(date)) return null;

	let dayName: string | null = null;
	let time: string | null = null;
	let endTime: string | null = null;
	let repeater: string | null = null;
	let warning: string | null = null;

	for (const part of parts.slice(1)) {
		const timeMatch = TIME_RANGE_RE.exec(part);
		if (timeMatch && time === null) {
			time = timeMatch[1];
			endTime = timeMatch[2] ?? null;
			continue;
		}
		if (REPEATER_RE.test(part) && repeater === null) {
			repeater = part;
			continue;
		}
		if (WARNING_RE.test(part) && warning === null) {
			warning = part;
			continue;
		}
		if (dayName === null && !/\d/.test(part)) {
			dayName = part;
		}
	}

	return {
		timestamp: { raw, active, date, dayName, time, endTime, repeater, warning },
		end: closeIndex + 1,
	};
}

/** Parse a standalone timestamp. The whole string need not be consumed. */
export function parseTimestamp(text: string): OrgTimestamp | null {
	const trimmed = text.trimStart();
	return readTimestamp(trimmed, 0)?.timestamp ?? null;
}

/** `true` when `text` begins with a well-formed timestamp. */
export function isTimestamp(text: string): boolean {
	return parseTimestamp(text) !== null;
}

/** Render a timestamp from its parts. Does not use the stored `raw`. */
export function formatTimestamp(ts: OrgTimestamp): string {
	const parts: string[] = [ts.date];
	if (ts.dayName) parts.push(ts.dayName);
	if (ts.time) parts.push(ts.endTime ? `${ts.time}-${ts.endTime}` : ts.time);
	if (ts.repeater) parts.push(ts.repeater);
	if (ts.warning) parts.push(ts.warning);
	const body = parts.join(' ');
	return ts.active ? `<${body}>` : `[${body}]`;
}

export interface MakeTimestampOptions {
	date: string;
	time?: string | null;
	endTime?: string | null;
	/** Omit to derive the English day name from `date`; pass `null` to leave it out. */
	dayName?: string | null;
	repeater?: string | null;
	warning?: string | null;
	/** `true` (default) writes `<…>`, `false` writes `[…]`. */
	active?: boolean;
}

/** Build a timestamp object from scratch, defaulting the day name from the date. */
export function makeTimestamp(options: MakeTimestampOptions): OrgTimestamp {
	const active = options.active ?? true;
	const dayName =
		options.dayName === undefined ? dayNameFor(options.date) || null : options.dayName;
	const time = normaliseTime(options.time);
	const ts: OrgTimestamp = {
		raw: '',
		active,
		date: options.date,
		dayName,
		time,
		endTime: time ? normaliseTime(options.endTime) : null,
		repeater: options.repeater ?? null,
		warning: options.warning ?? null,
	};
	return { ...ts, raw: formatTimestamp(ts) };
}

function normaliseTime(time: string | null | undefined): string | null {
	if (time === null || time === undefined || time === '') return null;
	const minutes = parseTime(time);
	return minutes === null ? null : formatTime(minutes);
}

export interface TimestampPatch {
	date?: string;
	time?: string | null;
	endTime?: string | null;
	dayName?: string | null;
	repeater?: string | null;
	warning?: string | null;
	active?: boolean;
}

/**
 * Return a copy of `ts` with the given fields replaced and `raw` re-rendered.
 * When the date changes and the original carried a day name, the day name is
 * recomputed so it never contradicts the date.
 */
export function withTimestamp(ts: OrgTimestamp, patch: TimestampPatch): OrgTimestamp {
	const date = patch.date ?? ts.date;
	let dayName = patch.dayName === undefined ? ts.dayName : patch.dayName;
	if (patch.dayName === undefined && dayName !== null && date !== ts.date) {
		dayName = dayNameFor(date) || dayName;
	}
	const time = patch.time === undefined ? ts.time : normaliseTime(patch.time);
	const next: OrgTimestamp = {
		raw: '',
		active: patch.active ?? ts.active,
		date,
		dayName,
		time,
		endTime: time ? (patch.endTime === undefined ? ts.endTime : normaliseTime(patch.endTime)) : null,
		repeater: patch.repeater === undefined ? ts.repeater : patch.repeater,
		warning: patch.warning === undefined ? ts.warning : patch.warning,
	};
	return { ...next, raw: formatTimestamp(next) };
}

/**
 * Parse user input into a timestamp.
 *
 * Accepts a full timestamp (`<2026-03-20 Fri 14:00 +1w>`, `[2026-03-20]`) or a
 * plain date/date-time (`2026-03-20`, `2026-03-20T14:00`, `2026-03-20 14:00`).
 * When `base` is given, its repeater and warning cookies are inherited unless
 * the input supplies its own — this is what keeps `+1w -2d` alive when a user
 * picks a new date in a date picker.
 */
export function parseTimestampInput(
	input: string,
	base?: OrgTimestamp | null,
	options?: { active?: boolean }
): OrgTimestamp | null {
	const trimmed = input.trim();
	if (!trimmed) return null;

	const read = readTimestamp(trimmed, 0);
	if (read) {
		const ts = read.timestamp;
		return withTimestamp(ts, {
			active: options?.active ?? ts.active,
			repeater: ts.repeater ?? base?.repeater ?? null,
			warning: ts.warning ?? base?.warning ?? null,
			dayName: ts.dayName ?? (dayNameFor(ts.date) || null),
		});
	}

	const [datePart, ...rest] = trimmed.split(/[T\s]+/);
	if (!isValidDate(datePart)) return null;
	const timeText = rest.join(' ').trim();
	// Fall back to the first `HH:MM` so `2026-03-20T14:00:00` still works.
	const timeMatch = TIME_RANGE_RE.exec(timeText) ?? TIME_RANGE_RE.exec(timeText.slice(0, 5));
	return makeTimestamp({
		date: datePart,
		time: timeMatch ? timeMatch[1] : null,
		endTime: timeMatch ? (timeMatch[2] ?? null) : null,
		repeater: base?.repeater ?? null,
		warning: base?.warning ?? null,
		active: options?.active ?? base?.active ?? true,
	});
}
