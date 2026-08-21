/// Agenda selection and ordering.
///
/// Org shows a scheduled item on its day and every day after until it is done,
/// and warns about a deadline for the days leading up to it. Both behaviours are
/// reproduced here so tasks cannot silently fall out of the view.

import { addDays, compareDates } from './date';
import { parseTimestamp } from './timestamp';
import { DEFAULT_PRIORITY_CONFIG } from './types';
import type { PriorityConfig } from './types';

/** Days before a deadline that org starts showing it. Org's own default. */
export const DEFAULT_DEADLINE_WARNING_DAYS = 14;

const WARNING_RE = /(?:^|\s)(--|-)(\d+)([dwmy])(?=[\s>\]]|$)/;

/** The subset of a headline row the agenda reasons about. */
export interface AgendaItem {
	todo?: string | null;
	priority?: string | null;
	scheduled?: string | null;
	deadline?: string | null;
	closed?: string | null;
}

export type AgendaReason = 'scheduled' | 'deadline' | 'overdue-scheduled' | 'upcoming-deadline';

const UNIT_DAYS: Record<string, number> = { d: 1, w: 7, m: 30, y: 365 };

/** Days of lead time a deadline's `-Nd` cookie asks for, or the default. */
export function deadlineWarningDays(
	timestamp: string | null | undefined,
	fallback = DEFAULT_DEADLINE_WARNING_DAYS
): number {
	if (!timestamp) return fallback;
	const m = WARNING_RE.exec(timestamp);
	if (!m) return fallback;
	return Number(m[2]) * (UNIT_DAYS[m[3]] ?? 1);
}

/** The `YYYY-MM-DD` part of an org timestamp string. */
export function timestampDate(timestamp: string | null | undefined): string | null {
	if (!timestamp) return null;
	const parsed = parseTimestamp(timestamp);
	if (parsed) return parsed.date;
	const m = /(\d{4}-\d{2}-\d{2})/.exec(timestamp);
	return m ? m[1] : null;
}

/** `true` when the item's keyword is one of the configured DONE states. */
export function isDone(item: AgendaItem, doneKeywords: readonly string[]): boolean {
	return item.todo !== null && item.todo !== undefined && doneKeywords.includes(item.todo);
}

/**
 * Why an item belongs in `date`'s agenda block, or `null` when it does not.
 *
 * A scheduled item that is past due keeps appearing on today until it is done —
 * without this an overdue task vanishes from the week entirely.
 */
export function agendaReason(
	item: AgendaItem,
	date: string,
	today: string,
	doneKeywords: readonly string[]
): AgendaReason | null {
	if (isDone(item, doneKeywords)) {
		const closed = timestampDate(item.closed);
		return closed === date ? 'scheduled' : null;
	}

	const scheduled = timestampDate(item.scheduled);
	const deadline = timestampDate(item.deadline);

	if (deadline === date) return 'deadline';
	if (scheduled === date) return 'scheduled';

	const isToday = date === today;
	if (isToday && scheduled && compareDates(scheduled, today) < 0) return 'overdue-scheduled';
	if (isToday && deadline && compareDates(deadline, today) < 0) return 'deadline';

	if (deadline && compareDates(date, deadline) < 0) {
		const warning = deadlineWarningDays(item.deadline);
		if (compareDates(addDays(deadline, -warning), date) <= 0) return 'upcoming-deadline';
	}

	return null;
}

/**
 * `true` when an item is already accounted for by the overdue block.
 *
 * Today's block and the overdue block are both driven off `today`, so without
 * this every late task is drawn twice — once under "Overdue" and again under
 * "Today" wearing a redundant "Sched. 10x" label.
 */
export function isOverdue<T extends AgendaItem>(
	item: T,
	today: string,
	doneKeywords: readonly string[]
): boolean {
	if (isDone(item, doneKeywords)) return false;
	const scheduled = timestampDate(item.scheduled);
	const deadline = timestampDate(item.deadline);
	return (
		(scheduled !== null && compareDates(scheduled, today) < 0) ||
		(deadline !== null && compareDates(deadline, today) < 0)
	);
}

/**
 * Items belonging to one day's agenda block.
 *
 * Pass `excludeOverdue` when the caller renders a separate overdue section, so
 * a late task appears there and not a second time under today.
 */
export function itemsForDate<T extends AgendaItem>(
	items: readonly T[],
	date: string,
	today: string,
	doneKeywords: readonly string[],
	excludeOverdue = false
): T[] {
	return items.filter((item) => {
		if (agendaReason(item, date, today, doneKeywords) === null) return false;
		if (excludeOverdue && date === today && isOverdue(item, today, doneKeywords)) return false;
		return true;
	});
}

/** Open items whose scheduled or deadline date has already passed. */
export function overdueItems<T extends AgendaItem>(
	items: readonly T[],
	today: string,
	doneKeywords: readonly string[]
): T[] {
	return items.filter((item) => isOverdue(item, today, doneKeywords));
}

/**
 * Rank of a priority cookie. Unset priorities sort as org treats them — at the
 * middle of the configured range, not last.
 */
export function priorityRank(
	priority: string | null | undefined,
	config: PriorityConfig = DEFAULT_PRIORITY_CONFIG
): number {
	const priorities = config.priorities.length > 0 ? config.priorities : DEFAULT_PRIORITY_CONFIG.priorities;
	if (priority) {
		const index = priorities.indexOf(priority);
		if (index !== -1) return index;
		return priorities.length;
	}
	const fallback = config.defaultPriority ?? priorities[Math.floor(priorities.length / 2)];
	const index = fallback ? priorities.indexOf(fallback) : -1;
	return index === -1 ? Math.floor(priorities.length / 2) : index;
}

/**
 * Task-list order: deadline first, then scheduled, then undated; ties broken by
 * priority. `Array.prototype.sort` is stable, so equal items keep source order.
 */
export function compareTasks(
	a: AgendaItem,
	b: AgendaItem,
	config: PriorityConfig = DEFAULT_PRIORITY_CONFIG
): number {
	const rank = (item: AgendaItem): number => (item.deadline ? 0 : item.scheduled ? 1 : 2);
	const byGroup = rank(a) - rank(b);
	if (byGroup !== 0) return byGroup;

	const dateOf = (item: AgendaItem): string | null =>
		timestampDate(item.deadline) ?? timestampDate(item.scheduled);
	const aDate = dateOf(a);
	const bDate = dateOf(b);
	if (aDate && bDate) {
		const byDate = compareDates(aDate, bDate);
		if (byDate !== 0) return byDate;
	}

	return priorityRank(a.priority, config) - priorityRank(b.priority, config);
}

/** Sort a task list without mutating the input. */
export function sortTasks<T extends AgendaItem>(
	items: readonly T[],
	config: PriorityConfig = DEFAULT_PRIORITY_CONFIG
): T[] {
	return [...items].sort((a, b) => compareTasks(a, b, config));
}
