import { describe, expect, it, vi } from 'vitest';
import { localDate, localTime, localTimestamp } from './commands';

// Pin a zone behind UTC so a UTC-derived date would visibly land on the wrong day.
vi.stubEnv('TZ', 'America/Los_Angeles');

describe('local time formatters', () => {
	it('formats the components the date was constructed with', () => {
		const d = new Date(2026, 0, 5, 9, 7, 3);
		expect(localDate(d)).toBe('2026-01-05');
		expect(localTime(d)).toBe('09:07');
		expect(localTimestamp(d)).toBe('20260105090703');
	});

	it('pads single-digit months, days, hours, minutes and seconds', () => {
		const d = new Date(2026, 8, 2, 0, 0, 0);
		expect(localDate(d)).toBe('2026-09-02');
		expect(localTime(d)).toBe('00:00');
		expect(localTimestamp(d)).toBe('20260902000000');
	});

	it('uses the local day, not the UTC day, in the evening west of UTC', () => {
		// 2026-01-16T05:30Z is still 2026-01-15 21:30 in America/Los_Angeles.
		const d = new Date(Date.UTC(2026, 0, 16, 5, 30, 0));
		expect(d.toISOString().slice(0, 10)).toBe('2026-01-16');

		expect(localDate(d)).toBe('2026-01-15');
		expect(localTime(d)).toBe('21:30');
		expect(localTimestamp(d)).toBe('20260115213000');
	});

	it('never returns a UTC-derived value across a full day of instants', () => {
		for (let hour = 0; hour < 24; hour++) {
			const d = new Date(Date.UTC(2026, 6, 4, hour, 0, 0));
			expect(localDate(d)).toBe(
				`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
			);
			expect(localTimestamp(d).slice(0, 8)).toBe(localDate(d).replace(/-/g, ''));
			expect(localTimestamp(d).slice(8)).toBe(localTime(d).replace(':', '') + '00');
		}
	});

	it('agrees with itself: timestamp is date + time + seconds', () => {
		const d = new Date(2026, 11, 31, 23, 59, 59);
		expect(localTimestamp(d)).toBe(
			localDate(d).replace(/-/g, '') + localTime(d).replace(':', '') + '59'
		);
	});
});
