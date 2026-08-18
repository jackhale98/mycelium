import { describe, expect, it } from 'vitest';
import { addMonths, compareDates, dayNameFor, shiftDate } from './date';
import { formatTimestamp, makeTimestamp, parseTimestamp, parseTimestampInput, withTimestamp } from './timestamp';

describe('parseTimestamp', () => {
	it('parses an active timestamp', () => {
		const ts = parseTimestamp('<2026-08-17 Mon>')!;
		expect(ts.active).toBe(true);
		expect(ts.date).toBe('2026-08-17');
		expect(ts.dayName).toBe('Mon');
	});

	it('parses an inactive timestamp', () => {
		expect(parseTimestamp('[2026-08-17 Mon]')!.active).toBe(false);
	});

	it('parses a time of day', () => {
		expect(parseTimestamp('<2026-08-17 Mon 09:30>')!.time).toBe('09:30');
	});

	it('parses a time range rather than dropping it', () => {
		const ts = parseTimestamp('<2026-08-17 Mon 10:00-11:30>')!;
		expect(ts.time).toBe('10:00');
		expect(ts.endTime).toBe('11:30');
	});

	it('parses repeater and warning cookies together', () => {
		const ts = parseTimestamp('<2026-08-17 Mon ++1w -2d>')!;
		expect(ts.repeater).toBe('++1w');
		expect(ts.warning).toBe('-2d');
	});

	it('rejects malformed input', () => {
		expect(parseTimestamp('<not a date>')).toBeNull();
		expect(parseTimestamp('<2026-08-17')).toBeNull();
		expect(parseTimestamp('plain text')).toBeNull();
	});
});

describe('formatTimestamp', () => {
	it('round-trips every component', () => {
		for (const raw of [
			'<2026-08-17 Mon>',
			'[2026-08-17 Mon]',
			'<2026-08-17 Mon 09:30>',
			'<2026-08-17 Mon 10:00-11:30>',
			'<2026-08-17 Mon +1w>',
			'<2026-08-17 Mon 09:30 ++1w -2d>',
		]) {
			expect(formatTimestamp(parseTimestamp(raw)!)).toBe(raw);
		}
	});
});

describe('makeTimestamp', () => {
	it('derives the day name from the date', () => {
		expect(makeTimestamp({ date: '2026-08-17' }).raw).toBe('<2026-08-17 Mon>');
	});

	it('writes an inactive stamp when asked', () => {
		expect(makeTimestamp({ date: '2026-08-17', active: false }).raw).toBe('[2026-08-17 Mon]');
	});
});

describe('withTimestamp', () => {
	it('recomputes the day name when the date changes', () => {
		const ts = parseTimestamp('<2026-08-17 Mon>')!;
		expect(withTimestamp(ts, { date: '2026-08-20' }).raw).toBe('<2026-08-20 Thu>');
	});

	it('keeps cookies unless replaced', () => {
		const ts = parseTimestamp('<2026-08-17 Mon +1w -2d>')!;
		expect(withTimestamp(ts, { date: '2026-08-24' }).raw).toBe('<2026-08-24 Mon +1w -2d>');
	});
});

describe('parseTimestampInput', () => {
	it('accepts a plain date and a datetime-local value', () => {
		expect(parseTimestampInput('2026-08-17')!.raw).toBe('<2026-08-17 Mon>');
		expect(parseTimestampInput('2026-08-17T14:00')!.raw).toBe('<2026-08-17 Mon 14:00>');
	});

	it('inherits cookies from the timestamp being replaced', () => {
		const base = parseTimestamp('<2026-08-17 Mon +1w -2d>')!;
		expect(parseTimestampInput('2026-09-01', base)!.raw).toBe('<2026-09-01 Tue +1w -2d>');
	});

	it('prefers cookies supplied by the input itself', () => {
		const base = parseTimestamp('<2026-08-17 Mon +1w>')!;
		expect(parseTimestampInput('<2026-09-01 Tue +2d>', base)!.repeater).toBe('+2d');
	});

	it('rejects junk', () => {
		expect(parseTimestampInput('')).toBeNull();
		expect(parseTimestampInput('tomorrow')).toBeNull();
	});
});

describe('date arithmetic', () => {
	it('names weekdays correctly', () => {
		expect(dayNameFor('2026-08-17')).toBe('Mon');
		expect(dayNameFor('2026-01-01')).toBe('Thu');
	});

	it('shifts by each unit', () => {
		expect(shiftDate('2026-08-17', 3, 'd')).toBe('2026-08-20');
		expect(shiftDate('2026-08-17', 1, 'w')).toBe('2026-08-24');
		expect(shiftDate('2026-08-17', 1, 'y')).toBe('2027-08-17');
	});

	it('clamps month arithmetic and crosses leap years', () => {
		expect(addMonths('2026-01-31', 1)).toBe('2026-02-28');
		expect(addMonths('2028-01-31', 1)).toBe('2028-02-29');
		expect(addMonths('2026-12-15', 1)).toBe('2027-01-15');
	});

	it('compares dates', () => {
		expect(compareDates('2026-08-17', '2026-08-18')).toBeLessThan(0);
		expect(compareDates('2026-08-17', '2026-08-17')).toBe(0);
	});
});
