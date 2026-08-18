/// Shared types for the pure org-mode editing logic in `$lib/org`.
/// Nothing in this directory may import Svelte, Tauri or the DOM.

/** The three planning keywords org recognises on a planning line. */
export type PlanningKind = 'SCHEDULED' | 'DEADLINE' | 'CLOSED';

/** Units accepted by repeater and warning cookies. */
export type TimeUnit = 'h' | 'd' | 'w' | 'm' | 'y';

/**
 * Repeater flavours:
 * - `cumulate`  — `+1w`  shift from the stored date
 * - `catch-up`  — `++1w` shift from the stored date until strictly after today
 * - `restart`   — `.+1w` shift from today
 */
export type RepeaterType = 'cumulate' | 'catch-up' | 'restart';

/** A parsed org timestamp such as `<2024-01-15 Mon 09:00-10:30 ++1w -2d>`. */
export interface OrgTimestamp {
	/** Exact source text, delimiters included. */
	raw: string;
	/** `true` for `<…>`, `false` for `[…]`. */
	active: boolean;
	/** `YYYY-MM-DD`. */
	date: string;
	/** Day-name token as written, e.g. `Mon`. `null` when absent. */
	dayName: string | null;
	/** `HH:MM` start time, or `null`. */
	time: string | null;
	/** `HH:MM` end time of a same-day time range, or `null`. */
	endTime: string | null;
	/** Repeater cookie text, e.g. `+1w`, `++1w`, `.+1d`. */
	repeater: string | null;
	/** Warning cookie text, e.g. `-2d`, `--1d`. */
	warning: string | null;
}

/** A parsed repeater or warning cookie. */
export interface RepeaterCookie {
	raw: string;
	type: RepeaterType;
	value: number;
	unit: TimeUnit;
}

/** User-configurable TODO/DONE keyword sets (mirrors `orgConfig`). */
export interface KeywordConfig {
	todoKeywords: string[];
	doneKeywords: string[];
}

/** User-configurable priority values, ordered highest first (mirrors `orgConfig`). */
export interface PriorityConfig {
	priorities: string[];
	/** Priority assumed for headlines without a cookie. Defaults to the middle value. */
	defaultPriority?: string | null;
}

/** Injected "current moment" — logic in this directory never calls `new Date()`. */
export interface OrgNow {
	/** `YYYY-MM-DD` in the caller's local time. */
	date: string;
	/** `HH:MM` in the caller's local time. Optional; only used for hour repeaters and CLOSED stamps. */
	time?: string | null;
}

export const DEFAULT_KEYWORD_CONFIG: KeywordConfig = {
	todoKeywords: ['TODO'],
	doneKeywords: ['DONE'],
};

export const DEFAULT_PRIORITY_CONFIG: PriorityConfig = {
	priorities: ['A', 'B', 'C'],
};

/** Order org itself writes planning entries in. */
export const PLANNING_ORDER: readonly PlanningKind[] = ['CLOSED', 'DEADLINE', 'SCHEDULED'];
