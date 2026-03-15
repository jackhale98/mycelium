import { Parser, NodeType, NodeSet, Tree, TreeFragment, NodeProp, type Input, type PartialParse } from '@lezer/common';
import { styleTags, tags as t } from '@lezer/highlight';

/// Node type IDs for our org grammar
const enum Type {
	Document = 1,
	Headline1,
	Headline2,
	Headline3,
	Headline4,
	HeadlineDeeper,
	TodoKeyword,
	Priority,
	HeadlineTags,
	MetadataLine,
	PropertyDrawer,
	PropertyKey,
	PropertyValue,
	Block,
	BlockMarker,
	BlockContent,
	Link,
	LinkTarget,
	LinkDescription,
	BoldMarkup,
	ItalicMarkup,
	UnderlineMarkup,
	StrikeMarkup,
	CodeMarkup,
	VerbatimMarkup,
	ListItem,
	ListBullet,
	Checkbox,
	Table,
	TableRule,
	TableCell,
	Timestamp,
	PlanningLine,
	Drawer,
	DrawerMarker,
	Comment,
	Paragraph,
}

const nodeTypes = [
	NodeType.none,
	/* 1  */ NodeType.define({ id: Type.Document, name: 'Document', top: true }),
	/* 2  */ NodeType.define({ id: Type.Headline1, name: 'Headline1' }),
	/* 3  */ NodeType.define({ id: Type.Headline2, name: 'Headline2' }),
	/* 4  */ NodeType.define({ id: Type.Headline3, name: 'Headline3' }),
	/* 5  */ NodeType.define({ id: Type.Headline4, name: 'Headline4' }),
	/* 6  */ NodeType.define({ id: Type.HeadlineDeeper, name: 'HeadlineDeeper' }),
	/* 7  */ NodeType.define({ id: Type.TodoKeyword, name: 'TodoKeyword' }),
	/* 8  */ NodeType.define({ id: Type.Priority, name: 'Priority' }),
	/* 9  */ NodeType.define({ id: Type.HeadlineTags, name: 'HeadlineTags' }),
	/* 10 */ NodeType.define({ id: Type.MetadataLine, name: 'MetadataLine' }),
	/* 11 */ NodeType.define({ id: Type.PropertyDrawer, name: 'PropertyDrawer' }),
	/* 12 */ NodeType.define({ id: Type.PropertyKey, name: 'PropertyKey' }),
	/* 13 */ NodeType.define({ id: Type.PropertyValue, name: 'PropertyValue' }),
	/* 14 */ NodeType.define({ id: Type.Block, name: 'Block' }),
	/* 15 */ NodeType.define({ id: Type.BlockMarker, name: 'BlockMarker' }),
	/* 16 */ NodeType.define({ id: Type.BlockContent, name: 'BlockContent' }),
	/* 17 */ NodeType.define({ id: Type.Link, name: 'Link' }),
	/* 18 */ NodeType.define({ id: Type.LinkTarget, name: 'LinkTarget' }),
	/* 19 */ NodeType.define({ id: Type.LinkDescription, name: 'LinkDescription' }),
	/* 20 */ NodeType.define({ id: Type.BoldMarkup, name: 'BoldMarkup' }),
	/* 21 */ NodeType.define({ id: Type.ItalicMarkup, name: 'ItalicMarkup' }),
	/* 22 */ NodeType.define({ id: Type.UnderlineMarkup, name: 'UnderlineMarkup' }),
	/* 23 */ NodeType.define({ id: Type.StrikeMarkup, name: 'StrikeMarkup' }),
	/* 24 */ NodeType.define({ id: Type.CodeMarkup, name: 'CodeMarkup' }),
	/* 25 */ NodeType.define({ id: Type.VerbatimMarkup, name: 'VerbatimMarkup' }),
	/* 26 */ NodeType.define({ id: Type.ListItem, name: 'ListItem' }),
	/* 27 */ NodeType.define({ id: Type.ListBullet, name: 'ListBullet' }),
	/* 28 */ NodeType.define({ id: Type.Checkbox, name: 'Checkbox' }),
	/* 29 */ NodeType.define({ id: Type.Table, name: 'Table' }),
	/* 30 */ NodeType.define({ id: Type.TableRule, name: 'TableRule' }),
	/* 31 */ NodeType.define({ id: Type.TableCell, name: 'TableCell' }),
	/* 32 */ NodeType.define({ id: Type.Timestamp, name: 'Timestamp' }),
	/* 33 */ NodeType.define({ id: Type.PlanningLine, name: 'PlanningLine' }),
	/* 34 */ NodeType.define({ id: Type.Drawer, name: 'Drawer' }),
	/* 35 */ NodeType.define({ id: Type.DrawerMarker, name: 'DrawerMarker' }),
	/* 36 */ NodeType.define({ id: Type.Comment, name: 'Comment' }),
	/* 37 */ NodeType.define({ id: Type.Paragraph, name: 'Paragraph' }),
];

const nodeSet = new NodeSet(nodeTypes).extend(
	styleTags({
		Headline1: t.heading1,
		Headline2: t.heading2,
		Headline3: t.heading3,
		Headline4: t.heading4,
		HeadlineDeeper: t.heading5,
		TodoKeyword: t.keyword,
		Priority: t.atom,
		HeadlineTags: t.tagName,
		MetadataLine: t.meta,
		PropertyDrawer: t.special(t.meta),
		PropertyKey: t.attributeName,
		PropertyValue: t.attributeValue,
		Block: t.blockComment,
		BlockMarker: t.processingInstruction,
		BlockContent: t.content,
		Link: t.link,
		LinkTarget: t.url,
		LinkDescription: t.labelName,
		BoldMarkup: t.strong,
		ItalicMarkup: t.emphasis,
		UnderlineMarkup: t.special(t.emphasis),
		StrikeMarkup: t.strikethrough,
		CodeMarkup: t.monospace,
		VerbatimMarkup: t.literal,
		ListItem: t.list,
		ListBullet: t.punctuation,
		Checkbox: t.atom,
		Table: t.content,
		TableRule: t.separator,
		TableCell: t.content,
		Timestamp: t.special(t.string),
		PlanningLine: t.keyword,
		Drawer: t.special(t.meta),
		DrawerMarker: t.brace,
		Comment: t.lineComment,
		Paragraph: t.content,
	})
);

interface BufferEntry {
	type: number;
	from: number;
	to: number;
	children: BufferEntry[];
}

/// Line-by-line incremental parser for org-mode
class OrgParser extends Parser {
	createParse(input: Input, fragments: readonly TreeFragment[], ranges: readonly { from: number; to: number }[]): PartialParse {
		return new OrgPartialParse(input, this);
	}

	get nodeSet() {
		return nodeSet;
	}
}

class OrgPartialParse implements PartialParse {
	private input: Input;
	private parser: OrgParser;
	stoppedAt: number | null = null;

	constructor(input: Input, parser: OrgParser) {
		this.input = input;
		this.parser = parser;
	}

	advance(): Tree | null {
		try {
			const text = this.input.read(0, this.input.length);
			const root = this.parseDocument(text);
			return this.buildTree(root, this.input.length);
		} catch (e) {
			console.warn('Org parser error, returning empty tree:', e);
			return Tree.build({ buffer: [], nodeSet, topID: 1, length: this.input.length });
		}
	}

	stopAt(pos: number) {
		this.stoppedAt = pos;
	}

	get parsedPos() {
		return this.input.length;
	}

	private parseDocument(text: string): BufferEntry {
		const children: BufferEntry[] = [];
		const lines = text.split('\n');
		let pos = 0;

		let i = 0;
		while (i < lines.length) {
			const line = lines[i];
			const lineStart = pos;
			const lineEnd = pos + line.length;

			// Headlines
			const headlineMatch = line.match(/^(\*+)\s/);
			if (headlineMatch) {
				const level = headlineMatch[1].length;
				const type =
					level === 1 ? Type.Headline1 :
					level === 2 ? Type.Headline2 :
					level === 3 ? Type.Headline3 :
					level === 4 ? Type.Headline4 : Type.HeadlineDeeper;

				const headlineChildren: BufferEntry[] = [];

				// Parse TODO keyword
				const afterStars = line.substring(level + 1);
				const todoMatch = afterStars.match(/^(TODO|DONE|NEXT|WAITING|HOLD|CANCELLED|CANCELED)\s/);
				if (todoMatch) {
					const todoStart = lineStart + level + 1;
					headlineChildren.push({
						type: Type.TodoKeyword,
						from: todoStart,
						to: todoStart + todoMatch[1].length,
						children: [],
					});
				}

				// Parse priority [#X]
				const prioMatch = afterStars.match(/\[#([A-Z])\]/);
				if (prioMatch && prioMatch.index !== undefined) {
					const prioStart = lineStart + level + 1 + prioMatch.index;
					headlineChildren.push({
						type: Type.Priority,
						from: prioStart,
						to: prioStart + 4,
						children: [],
					});
				}

				// Parse tags :tag1:tag2:
				const tagMatch = line.match(/\s(:[a-zA-Z0-9_@:]+:)\s*$/);
				if (tagMatch && tagMatch.index !== undefined) {
					headlineChildren.push({
						type: Type.HeadlineTags,
						from: lineStart + tagMatch.index + 1,
						to: lineEnd,
						children: [],
					});
				}

				children.push({
					type,
					from: lineStart,
					to: lineEnd,
					children: headlineChildren,
				});

				pos = lineEnd + 1;
				i++;
				continue;
			}

			// Metadata: #+KEY: value
			if (/^#\+\w+:/.test(line) && !/^#\+(BEGIN|END)_/i.test(line)) {
				children.push({ type: Type.MetadataLine, from: lineStart, to: lineEnd, children: [] });
				pos = lineEnd + 1;
				i++;
				continue;
			}

			// Comment: # text
			if (/^#\s/.test(line) || line === '#') {
				children.push({ type: Type.Comment, from: lineStart, to: lineEnd, children: [] });
				pos = lineEnd + 1;
				i++;
				continue;
			}

			// Property drawer: :PROPERTIES: ... :END:
			if (line.trim() === ':PROPERTIES:') {
				const drawerStart = lineStart;
				const drawerChildren: BufferEntry[] = [];
				drawerChildren.push({ type: Type.DrawerMarker, from: lineStart, to: lineEnd, children: [] });
				pos = lineEnd + 1;
				i++;

				while (i < lines.length) {
					const propLine = lines[i];
					const propStart = pos;
					const propEnd = pos + propLine.length;

					if (propLine.trim() === ':END:') {
						drawerChildren.push({ type: Type.DrawerMarker, from: propStart, to: propEnd, children: [] });
						pos = propEnd + 1;
						i++;
						break;
					}

					// Parse :KEY: value
					const propMatch = propLine.match(/^\s*:([^:]+):\s*(.*)/);
					if (propMatch) {
						const keyStart = propStart + propLine.indexOf(':' + propMatch[1] + ':');
						const keyEnd = keyStart + propMatch[1].length + 2;
						drawerChildren.push({ type: Type.PropertyKey, from: keyStart, to: keyEnd, children: [] });
						if (propMatch[2]) {
							drawerChildren.push({ type: Type.PropertyValue, from: keyEnd + 1, to: propEnd, children: [] });
						}
					}

					pos = propEnd + 1;
					i++;
				}

				children.push({ type: Type.PropertyDrawer, from: drawerStart, to: pos - 1, children: drawerChildren });
				continue;
			}

			// Generic drawer: :NAME: ... :END:
			const drawerMatch = line.match(/^\s*:([A-Z_-]+):\s*$/);
			if (drawerMatch && drawerMatch[1] !== 'END') {
				const drawerStart = lineStart;
				const drawerChildren: BufferEntry[] = [];
				drawerChildren.push({ type: Type.DrawerMarker, from: lineStart, to: lineEnd, children: [] });
				pos = lineEnd + 1;
				i++;

				while (i < lines.length) {
					const dLine = lines[i];
					const dStart = pos;
					const dEnd = pos + dLine.length;

					if (dLine.trim() === ':END:') {
						drawerChildren.push({ type: Type.DrawerMarker, from: dStart, to: dEnd, children: [] });
						pos = dEnd + 1;
						i++;
						break;
					}
					pos = dEnd + 1;
					i++;
				}

				children.push({ type: Type.Drawer, from: drawerStart, to: pos - 1, children: drawerChildren });
				continue;
			}

			// Block: #+BEGIN_xxx ... #+END_xxx
			const blockMatch = line.match(/^#\+(BEGIN_\w+)(.*)/i);
			if (blockMatch) {
				const blockStart = lineStart;
				const blockChildren: BufferEntry[] = [];
				blockChildren.push({ type: Type.BlockMarker, from: lineStart, to: lineEnd, children: [] });
				const endMarker = '#+END_' + blockMatch[1].substring(6);
				const contentStart = lineEnd + 1;
				pos = lineEnd + 1;
				i++;

				let contentEnd = contentStart;
				while (i < lines.length) {
					const bLine = lines[i];
					const bStart = pos;
					const bEnd = pos + bLine.length;

					if (bLine.trim().toUpperCase() === endMarker.toUpperCase()) {
						if (contentEnd > contentStart) {
							blockChildren.push({ type: Type.BlockContent, from: contentStart, to: contentEnd, children: [] });
						}
						blockChildren.push({ type: Type.BlockMarker, from: bStart, to: bEnd, children: [] });
						pos = bEnd + 1;
						i++;
						break;
					}

					contentEnd = bEnd;
					pos = bEnd + 1;
					i++;
				}

				children.push({ type: Type.Block, from: blockStart, to: pos - 1, children: blockChildren });
				continue;
			}

			// Planning line: SCHEDULED: DEADLINE: CLOSED:
			if (/^\s*(SCHEDULED|DEADLINE|CLOSED):/.test(line)) {
				const planChildren = this.parseTimestampsInLine(line, lineStart);
				children.push({ type: Type.PlanningLine, from: lineStart, to: lineEnd, children: planChildren });
				pos = lineEnd + 1;
				i++;
				continue;
			}

			// Table
			if (line.trimStart().startsWith('|')) {
				if (/^\s*\|[-+]+\|?\s*$/.test(line)) {
					children.push({ type: Type.TableRule, from: lineStart, to: lineEnd, children: [] });
				} else {
					const cellChildren: BufferEntry[] = [];
					const cellRegex = /\|([^|]*)/g;
					let cm;
					while ((cm = cellRegex.exec(line)) !== null) {
						if (cm[1] && cm[1].trim()) {
							cellChildren.push({
								type: Type.TableCell,
								from: lineStart + cm.index + 1,
								to: lineStart + cm.index + 1 + cm[1].length,
								children: [],
							});
						}
					}
					children.push({ type: Type.Table, from: lineStart, to: lineEnd, children: cellChildren });
				}
				pos = lineEnd + 1;
				i++;
				continue;
			}

			// List item
			const listMatch = line.match(/^(\s*)([-+]|\d+[.)]) /);
			if (listMatch) {
				const listChildren: BufferEntry[] = [];
				const bulletStart = lineStart + listMatch[1].length;
				const bulletEnd = bulletStart + listMatch[2].length;
				listChildren.push({ type: Type.ListBullet, from: bulletStart, to: bulletEnd, children: [] });

				// Checkbox
				const afterBullet = line.substring(listMatch[0].length);
				const cbMatch = afterBullet.match(/^\[[ Xx-]\] /);
				if (cbMatch) {
					const cbStart = bulletEnd + 1;
					listChildren.push({ type: Type.Checkbox, from: cbStart, to: cbStart + 3, children: [] });
				}

				// Inline content
				this.parseInlineContent(line, lineStart, listChildren);

				children.push({ type: Type.ListItem, from: lineStart, to: lineEnd, children: listChildren });
				pos = lineEnd + 1;
				i++;
				continue;
			}

			// Regular paragraph line with inline parsing
			if (line.trim().length > 0) {
				const paraChildren: BufferEntry[] = [];
				this.parseInlineContent(line, lineStart, paraChildren);
				children.push({ type: Type.Paragraph, from: lineStart, to: lineEnd, children: paraChildren });
			}

			pos = lineEnd + 1;
			i++;
		}

		return { type: Type.Document, from: 0, to: text.length, children };
	}

	private parseInlineContent(line: string, lineStart: number, children: BufferEntry[]) {
		// Links: [[...][...]] or [[...]]
		const linkRegex = /\[\[([^\]]+?)(?:\]\[([^\]]*))?\]\]/g;
		let lm;
		while ((lm = linkRegex.exec(line)) !== null) {
			const linkFrom = lineStart + lm.index;
			const linkTo = linkFrom + lm[0].length;
			const linkChildren: BufferEntry[] = [];

			// Target
			const targetStart = linkFrom + 2;
			const targetEnd = targetStart + lm[1].length;
			linkChildren.push({ type: Type.LinkTarget, from: targetStart, to: targetEnd, children: [] });

			// Description
			if (lm[2] !== undefined) {
				const descStart = targetEnd + 2; // ][
				const descEnd = descStart + lm[2].length;
				linkChildren.push({ type: Type.LinkDescription, from: descStart, to: descEnd, children: [] });
			}

			children.push({ type: Type.Link, from: linkFrom, to: linkTo, children: linkChildren });
		}

		// Markup: *bold*, /italic/, _underline_, +strike+, ~code~, =verbatim=
		const markupPatterns: [RegExp, number][] = [
			[/(?:^|[\s(])(\*[^\s*](?:[^*]*[^\s*])?\*)(?:[\s.,;:!?)]|$)/g, Type.BoldMarkup],
			[/(?:^|[\s(])(\/[^\s/](?:[^/]*[^\s/])?\/)(?:[\s.,;:!?)]|$)/g, Type.ItalicMarkup],
			[/(?:^|[\s(])(_[^\s_](?:[^_]*[^\s_])?_)(?:[\s.,;:!?)]|$)/g, Type.UnderlineMarkup],
			[/(?:^|[\s(])(\+[^\s+](?:[^+]*[^\s+])?\+)(?:[\s.,;:!?)]|$)/g, Type.StrikeMarkup],
			[/(?:^|[\s(])(~[^\s~](?:[^~]*[^\s~])?~)(?:[\s.,;:!?)]|$)/g, Type.CodeMarkup],
			[/(?:^|[\s(])(=[^\s=](?:[^=]*[^\s=])?=)(?:[\s.,;:!?)]|$)/g, Type.VerbatimMarkup],
		];

		for (const [re, type] of markupPatterns) {
			let mm;
			while ((mm = re.exec(line)) !== null) {
				const content = mm[1];
				const contentStart = lineStart + mm.index + (mm[0].length - mm[0].trimStart().length);
				const idx = line.indexOf(content, mm.index);
				if (idx >= 0) {
					children.push({ type, from: lineStart + idx, to: lineStart + idx + content.length, children: [] });
				}
			}
		}

		// Timestamps
		this.parseTimestampsInLine(line, lineStart).forEach((ts) => children.push(ts));
	}

	private parseTimestampsInLine(line: string, lineStart: number): BufferEntry[] {
		const results: BufferEntry[] = [];
		const tsRegex = /[<\[][0-9]{4}-[0-9]{2}-[0-9]{2}[^\]>]*[>\]]/g;
		let tm;
		while ((tm = tsRegex.exec(line)) !== null) {
			results.push({
				type: Type.Timestamp,
				from: lineStart + tm.index,
				to: lineStart + tm.index + tm[0].length,
				children: [],
			});
		}
		return results;
	}

	private buildTree(root: BufferEntry, length: number): Tree {
		const buffer = this.flattenToBuffer(root);
		return Tree.build({
			buffer,
			nodeSet,
			topID: Type.Document,
			length,
		});
	}

	private flattenToBuffer(entry: BufferEntry): number[] {
		const buffer: number[] = [];
		this.writeEntry(buffer, entry);
		return buffer;
	}

	private writeEntry(buffer: number[], entry: BufferEntry) {
		// Write children first (they appear before parent in the buffer)
		for (const child of entry.children) {
			this.writeEntry(buffer, child);
		}
		// type, from, to, size (4 = leaf node, larger includes children)
		// For the buffer format: each node is [type, start, end, size]
		// size is 4 for leaves, or 4 + total children buffer size
		const childrenSize = entry.children.reduce((sum, c) => sum + this.entrySize(c), 0);
		buffer.push(entry.type, entry.from, entry.to, 4 + childrenSize);
	}

	private entrySize(entry: BufferEntry): number {
		return 4 + entry.children.reduce((sum, c) => sum + this.entrySize(c), 0);
	}
}

export const parser = new OrgParser();
