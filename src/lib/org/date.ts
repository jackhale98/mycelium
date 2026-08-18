/// Calendar arithmetic on `YYYY-MM-DD` / `HH:MM` strings.
/// Deterministic: no function here reads the system clock.

import type { TimeUnit } from './types';

export const DAY_NAMES: readonly string[] = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const TIME_RE = /^(\d{1,2}):(\d{2})$/;

export interface CalendarDate {
	year: number;
	month: number;
	day: number;
}

/** Number of days in a month (1-based month). */
export function daysInMonth(year: number, month: number): number {
	if (month === 2) {
		const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
		return leap ? 29 : 28;
	}
	return [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1] ?? 30;
}

/** Parse `YYYY-MM-DD`. Returns `null` for malformed or non-existent dates. */
export function parseDate(iso: string): CalendarDate | null {
	const m = DATE_RE.exec(iso.trim());
	if (!m) return null;
	const year = Number(m[1]);
	const month = Number(m[2]);
	const day = Number(m[3]);
	if (month < 1 || month > 12) return null;
	if (day < 1 || day > daysInMonth(year, month)) return null;
	return { year, month, day };
}

/** `true` when `iso` is a well-formed, existing `YYYY-MM-DD` date. */
export function isValidDate(iso: string): boolean {
	return parseDate(iso) !== null;
}

export function formatDate(d: CalendarDate): string {
	return `${String(d.year).padStart(4, '0')}-${String(d.month).padStart(2, '0')}-${String(d.day).padStart(2, '0')}`;
}

/** Days since 1970-01-01 (proleptic Gregorian, no timezone involved). */
export function toDayNumber(iso: string): number {
	const d = parseDate(iso);
	if (!d) return NaN;
	const dt = new Date(0);
	dt.setUTCFullYear(d.year, d.month - 1, d.day);
	dt.setUTCHours(0, 0, 0, 0);
	return Math.round(dt.getTime() / 86400000);
}

/** Inverse of {@link toDayNumber}. */
export function fromDayNumber(days: number): string {
	const dt = new Date(days * 86400000);
	return formatDate({
		year: dt.getUTCFullYear(),
		month: dt.getUTCMonth() + 1,
		day: dt.getUTCDate(),
	});
}

/** Three-letter English day name for a date, as org writes it. */
export function dayNameFor(iso: string): string {
	const n = toDayNumber(iso);
	if (Number.isNaN(n)) return '';
	// 1970-01-01 was a Thursday (index 4).
	return DAY_NAMES[(((n + 4) % 7) + 7) % 7];
}

export function addDays(iso: string, days: number): string {
	const n = toDayNumber(iso);
	if (Number.isNaN(n)) return iso;
	return fromDayNumber(n + days);
}

/** Add months, clamping the day into the target month (Jan 31 + 1m = Feb 28/29). */
export function addMonths(iso: string, months: number): string {
	const d = parseDate(iso);
	if (!d) return iso;
	const total = d.year * 12 + (d.month - 1) + months;
	const year = Math.floor(total / 12);
	const month = (((total % 12) + 12) % 12) + 1;
	const day = Math.min(d.day, daysInMonth(year, month));
	return formatDate({ year, month, day });
}

/** Add years, clamping Feb 29 to Feb 28 in non-leap years. */
export function addYears(iso: string, years: number): string {
	return addMonths(iso, years * 12);
}

/** Shift a date by `value` `unit`s. Hour shifts round down to whole days. */
export function shiftDate(iso: string, value: number, unit: TimeUnit): string {
	switch (unit) {
		case 'h':
			return addDays(iso, Math.trunc(value / 24));
		case 'd':
			return addDays(iso, value);
		case 'w':
			return addDays(iso, value * 7);
		case 'm':
			return addMonths(iso, value);
		case 'y':
			return addYears(iso, value);
	}
}

/** Lexicographic comparison is correct for `YYYY-MM-DD`; this also tolerates blanks. */
export function compareDates(a: string, b: string): number {
	return a < b ? -1 : a > b ? 1 : 0;
}

/** Parse `HH:MM` to minutes past midnight, or `null`. */
export function parseTime(time: string): number | null {
	const m = TIME_RE.exec(time.trim());
	if (!m) return null;
	const hours = Number(m[1]);
	const minutes = Number(m[2]);
	if (hours > 23 || minutes > 59) return null;
	return hours * 60 + minutes;
}

/** `true` when `time` is a well-formed `HH:MM` clock time. */
export function isValidTime(time: string): boolean {
	return parseTime(time) !== null;
}

/** Normalise `9:05` to `09:05`. Returns `null` for malformed input. */
export function formatTime(minutes: number): string {
	const wrapped = ((minutes % 1440) + 1440) % 1440;
	return `${String(Math.floor(wrapped / 60)).padStart(2, '0')}:${String(wrapped % 60).padStart(2, '0')}`;
}

/** Shift a date+time by whole hours, rolling the date over as needed. */
export function addHours(
	iso: string,
	time: string | null,
	hours: number
): { date: string; time: string | null } {
	if (time === null) return { date: addDays(iso, Math.trunc(hours / 24)), time: null };
	const minutes = parseTime(time);
	if (minutes === null) return { date: iso, time };
	const total = minutes + hours * 60;
	const dayShift = Math.floor(total / 1440);
	return { date: addDays(iso, dayShift), time: formatTime(total) };
}

/**
 * Parse the value of an `<input type="datetime-local">` / `date` control:
 * `2026-03-20`, `2026-03-20T14:00` or `2026-03-20 14:00`.
 */
export function parseDateTimeInput(value: string): { date: string; time: string | null } | null {
	const trimmed = value.trim();
	if (!trimmed) return null;
	const [datePart, ...rest] = trimmed.split(/[T ]+/);
	if (!isValidDate(datePart)) return null;
	const timePart = rest.join(' ').trim();
	if (!timePart) return { date: datePart, time: null };
	const minutes = parseTime(timePart.slice(0, 5));
	if (minutes === null) return { date: datePart, time: null };
	return { date: datePart, time: formatTime(minutes) };
}
