/// `#+FILETAGS:` parsing and editing.
///
/// Org accepts both `:a:b:` and whitespace-separated values on this line, so
/// `#+FILETAGS: foo bar` is two tags — not one tag named "foo bar".

const FILETAGS_RE = /^([ \t]*#\+FILETAGS:)([ \t]*)(.*)$/i;
const TAG_CHAR_RE = /^[\p{L}\p{N}_@#%]+$/u;

function splitEol(line: string): [string, string] {
	return line.endsWith('\r') ? [line.slice(0, -1), '\r'] : [line, ''];
}

/** `true` when `tag` may be written to a tag block unquoted. */
export function isValidTag(tag: string): boolean {
	return TAG_CHAR_RE.test(tag);
}

/**
 * Normalise user input into a legal org tag, or `null` when nothing usable
 * remains. Spaces and other illegal characters become underscores.
 */
export function normaliseTag(tag: string): string | null {
	const cleaned = tag.trim().replace(/[^\p{L}\p{N}_@#%]+/gu, '_').replace(/^_+|_+$/g, '');
	return cleaned.length > 0 ? cleaned : null;
}

/** Split a `#+FILETAGS:` value written in either supported form. */
export function parseFiletagsValue(value: string): string[] {
	return value
		.split(/[:\s]+/)
		.map((t) => t.trim())
		.filter((t) => t.length > 0);
}

/** Index of the `#+FILETAGS:` line, or `null`. */
export function findFiletagsIndex(lines: readonly string[]): number | null {
	for (let i = 0; i < lines.length; i += 1) {
		if (FILETAGS_RE.test(splitEol(lines[i])[0])) return i;
	}
	return null;
}

/** Every file tag declared in the document. */
export function getFiletags(lines: readonly string[]): string[] {
	const index = findFiletagsIndex(lines);
	if (index === null) return [];
	const m = FILETAGS_RE.exec(splitEol(lines[index])[0])!;
	return parseFiletagsValue(m[3]);
}

/**
 * Write the file tags, in org's `:a:b:` form. An empty list removes the line.
 * A new line is inserted after the leading `#+` keyword block so it stays with
 * the file's other metadata.
 */
export function setFiletags(lines: readonly string[], tags: readonly string[]): string[] {
	const next = [...lines];
	const clean: string[] = [];
	for (const tag of tags) {
		const normalised = normaliseTag(tag);
		if (normalised && !clean.includes(normalised)) clean.push(normalised);
	}

	const index = findFiletagsIndex(next);

	if (clean.length === 0) {
		if (index !== null) next.splice(index, 1);
		return next;
	}

	const value = `:${clean.join(':')}:`;

	if (index === null) {
		const eol = next.length > 0 ? splitEol(next[0])[1] : '';
		next.splice(metadataInsertIndex(next), 0, `#+FILETAGS: ${value}${eol}`);
		return next;
	}

	const [text, eol] = splitEol(next[index]);
	const m = FILETAGS_RE.exec(text)!;
	next[index] = `${m[1]} ${value}${eol}`;
	return next;
}

/** Add a tag if absent, remove it if present. */
export function toggleFiletag(lines: readonly string[], tag: string): string[] {
	const normalised = normaliseTag(tag);
	if (!normalised) return [...lines];
	const current = getFiletags(lines);
	const next = current.includes(normalised)
		? current.filter((t) => t !== normalised)
		: [...current, normalised];
	return setFiletags(lines, next);
}

/** Position for a new metadata line: after the leading `#+` keyword block. */
function metadataInsertIndex(lines: readonly string[]): number {
	let index = 0;
	if (lines[0] !== undefined && splitEol(lines[0])[0].trim() === ':PROPERTIES:') {
		while (index < lines.length && splitEol(lines[index])[0].trim() !== ':END:') index += 1;
		index = Math.min(index + 1, lines.length);
	}
	while (index < lines.length && /^[ \t]*#\+\w/.test(splitEol(lines[index])[0])) index += 1;
	return index;
}
