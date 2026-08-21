/// Keyword categories.
///
/// Org itself knows only two states: not-done and done, split by the `|` in a
/// `#+TODO:` line. "Waiting" is not a third org state — a `WAITING` headline is
/// an ordinary not-done task, and it must stay on the agenda and out of every
/// done check. The category here is a *presentation* split of the not-done set,
/// so the user can tell an active task from a blocked one at a glance.
///
/// This means `CategoryConfig.waitingKeywords` is always a subset of the
/// keywords the parser is given as not-done; see `orgConfig.keywordConfig`.

import type { KeywordConfig } from './types';

/** How a keyword is presented. `none` is a headline with no keyword at all. */
export type KeywordCategory = 'todo' | 'waiting' | 'done' | 'none';

/** Keyword sets split three ways for display. */
export interface CategoryConfig {
	/** Active not-done states, e.g. `TODO`, `NEXT`. */
	todoKeywords: readonly string[];
	/** Blocked not-done states, e.g. `WAITING`, `HOLD`. Still not-done to org. */
	waitingKeywords?: readonly string[];
	/** Done states, e.g. `DONE`, `CANCELLED`. */
	doneKeywords: readonly string[];
}

export const DEFAULT_CATEGORY_CONFIG: CategoryConfig = {
	todoKeywords: ['TODO'],
	waitingKeywords: [],
	doneKeywords: ['DONE'],
};

/**
 * Which category a keyword displays as.
 *
 * Done is checked first: whether a task is finished drives real behaviour
 * (agenda inclusion, CLOSED stamps), so a keyword listed in two sets must never
 * be shown as open. Unknown keywords — one written by hand, or left over from a
 * `#+TODO:` line the user has not mirrored into settings — count as `todo`,
 * because an unrecognised keyword on a headline still means it is not finished.
 */
export function keywordCategory(
	keyword: string | null | undefined,
	config: CategoryConfig = DEFAULT_CATEGORY_CONFIG
): KeywordCategory {
	if (!keyword) return 'none';
	if (config.doneKeywords.includes(keyword)) return 'done';
	if (config.waitingKeywords?.includes(keyword)) return 'waiting';
	return 'todo';
}

/** The `state-*` class that paints a keyword. Pairs with `.state-chip`. */
export function keywordCategoryClass(
	keyword: string | null | undefined,
	config: CategoryConfig = DEFAULT_CATEGORY_CONFIG
): string {
	return `state-${keywordCategory(keyword, config)}`;
}

/**
 * Every not-done keyword, active before blocked.
 *
 * This is what the parser and `KeywordConfig.todoKeywords` need: leaving the
 * waiting set out would make `* WAITING Ship it` parse as a headline titled
 * "WAITING Ship it".
 */
export function notDoneKeywords(config: CategoryConfig): string[] {
	return [...config.todoKeywords, ...(config.waitingKeywords ?? [])];
}

/** Build the parser's two-way config from a three-way display config. */
export function toKeywordConfig(config: CategoryConfig): KeywordConfig {
	return {
		todoKeywords: notDoneKeywords(config),
		doneKeywords: [...config.doneKeywords],
	};
}
