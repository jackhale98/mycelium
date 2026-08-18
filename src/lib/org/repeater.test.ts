import { describe, expect, it } from 'vitest';
import { applyRepeaterOnDone, hasRepeater, nextOccurrence, parseRepeater, repeatKeyword } from './repeater';
import { getPlanning } from './planning';
import { makeTimestamp, parseTimestamp } from './timestamp';

const now = { date: '2026-08-17', time: '10:00' };

describe('parseRepeater', () => {
	it('recognises all three flavours', () => {
		expect(parseRepeater('+1w')?.type).toBe('cumulate');
		expect(parseRepeater('++1w')?.type).toBe('catch-up');
		expect(parseRepeater('.+1w')?.type).toBe('restart');
	});

	it('reads value and unit', () => {
		const cookie = parseRepeater('++3m');
		expect(cookie?.value).toBe(3);
		expect(cookie?.unit).toBe('m');
	});

	it('rejects warnings, junk and zero intervals', () => {
		expect(parseRepeater('-2d')).toBeNull();
		expect(parseRepeater('+0d')).toBeNull();
		expect(parseRepeater('+1x')).toBeNull();
		expect(parseRepeater(null)).toBeNull();
	});
});

describe('nextOccurrence', () => {
	const ts = (raw: string) => parseTimestamp(raw)!;

	it('+1w shifts one interval from the stored date', () => {
		expect(nextOccurrence(ts('<2026-08-17 Mon +1w>'), now).date).toBe('2026-08-24');
	});

	it('+1w shifts only once even when far in the past', () => {
		expect(nextOccurrence(ts('<2026-01-01 Thu +1w>'), now).date).toBe('2026-01-08');
	});

	it('++1w catches up to the first date after today', () => {
		expect(nextOccurrence(ts('<2026-08-03 Mon ++1w>'), now).date).toBe('2026-08-24');
	});

	it('++1w lands strictly after today, never on it', () => {
		expect(nextOccurrence(ts('<2026-08-10 Mon ++1w>'), now).date).toBe('2026-08-24');
	});

	it('.+1w restarts from today', () => {
		expect(nextOccurrence(ts('<2026-01-05 Mon .+1w>'), now).date).toBe('2026-08-24');
	});

	it('keeps the time of day and refreshes the day name', () => {
		const next = nextOccurrence(ts('<2026-08-17 Mon 09:30 +1w>'), now);
		expect(next.time).toBe('09:30');
		expect(next.raw).toBe('<2026-08-24 Mon 09:30 +1w>');
	});

	it('preserves the warning cookie', () => {
		expect(nextOccurrence(ts('<2026-08-17 Mon +1w -2d>'), now).warning).toBe('-2d');
	});

	it('clamps month arithmetic to the end of a short month', () => {
		expect(nextOccurrence(ts('<2026-01-31 Sat +1m>'), now).date).toBe('2026-02-28');
	});

	it('shifts by hours across midnight', () => {
		const next = nextOccurrence(ts('<2026-08-17 Mon 23:00 +2h>'), now);
		expect(next.date).toBe('2026-08-18');
		expect(next.time).toBe('01:00');
	});

	it('returns the timestamp unchanged when there is no repeater', () => {
		const plain = ts('<2026-08-17 Mon>');
		expect(nextOccurrence(plain, now)).toEqual(plain);
	});
});

describe('hasRepeater', () => {
	it('detects a repeating timestamp', () => {
		expect(hasRepeater(makeTimestamp({ date: '2026-08-17', repeater: '+1w' }))).toBe(true);
		expect(hasRepeater(makeTimestamp({ date: '2026-08-17' }))).toBe(false);
		expect(hasRepeater(null)).toBe(false);
	});
});

describe('applyRepeaterOnDone', () => {
	it('reschedules instead of completing a repeating task', () => {
		const lines = ['* TODO Water plants', 'SCHEDULED: <2026-08-17 Mon +1w>'];
		const result = applyRepeaterOnDone(lines, 0, now);
		expect(result.repeated).toBe(true);
		expect(result.shifted).toEqual(['SCHEDULED']);
		expect(result.lines[1]).toBe('SCHEDULED: <2026-08-24 Mon +1w>');
	});

	it('shifts scheduled and deadline together on a combined line', () => {
		const lines = ['* TODO Rent', 'SCHEDULED: <2026-08-17 Mon +1m> DEADLINE: <2026-08-20 Thu +1m>'];
		const result = applyRepeaterOnDone(lines, 0, now);
		expect(result.shifted).toEqual(['SCHEDULED', 'DEADLINE']);
		const planning = getPlanning(result.lines, 0);
		expect(planning.SCHEDULED?.date).toBe('2026-09-17');
		expect(planning.DEADLINE?.date).toBe('2026-09-20');
	});

	it('leaves a non-repeating task alone so the caller closes it normally', () => {
		const lines = ['* TODO One off', 'SCHEDULED: <2026-08-17 Mon>'];
		const result = applyRepeaterOnDone(lines, 0, now);
		expect(result.repeated).toBe(false);
		expect(result.lines).toEqual(lines);
	});

	it('shifts only the entry that repeats', () => {
		const lines = ['* TODO Mixed', 'SCHEDULED: <2026-08-17 Mon +1w> DEADLINE: <2026-08-20 Thu>'];
		const planning = getPlanning(applyRepeaterOnDone(lines, 0, now).lines, 0);
		expect(planning.SCHEDULED?.date).toBe('2026-08-24');
		expect(planning.DEADLINE?.date).toBe('2026-08-20');
	});

	it('clears a stale CLOSED stamp, since a rescheduled task is not closed', () => {
		const lines = ['* TODO Water', 'CLOSED: [2026-08-10 Mon] SCHEDULED: <2026-08-17 Mon +1w>'];
		const result = applyRepeaterOnDone(lines, 0, now);
		expect(getPlanning(result.lines, 0).CLOSED).toBeNull();
	});

	it('handles a headline with no planning at all', () => {
		const lines = ['* TODO Bare'];
		expect(applyRepeaterOnDone(lines, 0, now).repeated).toBe(false);
	});
});

describe('repeatKeyword', () => {
	it('returns the first configured TODO state', () => {
		expect(repeatKeyword({ todoKeywords: ['TODO', 'NEXT'], doneKeywords: ['DONE'] })).toBe('TODO');
		expect(repeatKeyword({ todoKeywords: [], doneKeywords: ['DONE'] })).toBeNull();
	});
});
