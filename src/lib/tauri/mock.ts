/// Mock data for browser preview mode (when Tauri backend is not available).
/// Models a realistic org-roam vault where:
///   - Each file has a file-level :ID: property
///   - Links use [[id:uuid][description]] format
///   - Backlinks are derived from which files contain links TO a given node
///   - Forward links are the [[id:...]] links found inside a file's content

import type { NodeRecord, BacklinkRecord, ForwardLink, GraphData, SearchResult, TagCount } from '$lib/types/node';
import type { FileRecord, SyncResult } from '$lib/types/vault';

// ── File contents (the actual .org text) ──────────────────────────────────
// Each file is a self-contained org-roam note with :ID: and [[id:...]] links.

const FILES: Record<string, string> = {
	'/vault/rust.org': `:PROPERTIES:
:ID: node-rust
:ROAM_ALIASES: "Rust Lang"
:END:
#+TITLE: Rust Programming
#+FILETAGS: :programming:rust:

Rust is a systems programming language focused on *safety*, /performance/, and ~concurrency~.

It was created by Mozilla and is now maintained by the Rust Foundation.
See [[id:node-cargo][Cargo]] for the build system.

* Ownership
:PROPERTIES:
:ID: node-ownership
:END:

Every value in Rust has a single /owner/. When the owner goes out of scope, the value is dropped.

- Moves transfer ownership
- =Copy= types are duplicated instead
- [ ] Read the Rustonomicon chapter on ownership
- [X] Understand move semantics

See also [[id:node-borrowing][Borrowing]] for how to reference values without taking ownership.

** Borrowing
:PROPERTIES:
:ID: node-borrowing
:END:

References let you /borrow/ a value without taking ownership:

- Shared references: =&T= (many readers)
- Mutable references: =&mut T= (one writer)

#+BEGIN_SRC rust
fn print_length(s: &str) {
    println!("Length: {}", s.len());
}
#+END_SRC

* Error Handling
:PROPERTIES:
:ID: node-errors
:END:
SCHEDULED: <2024-02-01 Thu>

Rust uses ~Result<T, E>~ and ~Option<T>~ for error handling — no exceptions.

| Pattern         | Use case               |
|-----------------+------------------------|
| Result<T, E>    | Operations that can fail |
| Option<T>       | Values that may be absent |
| panic!()        | Unrecoverable errors     |

Related: [[id:node-mycelium][Mycelium]] uses these patterns extensively.
`,

	'/vault/cargo.org': `:PROPERTIES:
:ID: node-cargo
:END:
#+TITLE: Cargo
#+FILETAGS: :rust:tools:

Cargo is Rust's build system and package manager.

Every [[id:node-rust][Rust]] project uses Cargo for building, testing, and dependency management.

* Key Commands

| Command         | Description            |
|-----------------+------------------------|
| cargo build     | Compile the project    |
| cargo test      | Run test suite         |
| cargo run       | Build and execute      |
| cargo clippy    | Lint the code          |

The [[id:node-mycelium][Mycelium]] project uses a Cargo workspace with multiple crates.
`,

	'/vault/emacs.org': `:PROPERTIES:
:ID: node-emacs
:END:
#+TITLE: Emacs & Org Mode
#+FILETAGS: :emacs:tools:

Emacs is an extensible text editor with a rich ecosystem.

* Org Mode

Org mode is Emacs' *killer feature* for note-taking, task management, and literate programming.

- Plain text format
- Hierarchical structure with headlines
- TODO tracking and scheduling
- Export to HTML, LaTeX, PDF

* Org Roam
:PROPERTIES:
:ID: node-org-roam
:END:

Org-roam brings Zettelkasten-style networked notes to Org mode.
It stores a SQLite index of all =:ID:= properties and =[[id:...]]= links.

The [[id:node-mycelium][Mycelium]] app is an open-source mobile client for org-roam vaults.
It parses .org files using a custom [[id:node-rust][Rust]] parser.
`,

	'/vault/mycelium.org': `:PROPERTIES:
:ID: node-mycelium
:END:
#+TITLE: Mycelium
#+FILETAGS: :project:app:

Mycelium is an open-source mobile knowledge base for org-roam vaults.

* Architecture

Built with:
- [[id:node-rust][Rust]] backend (Tauri v2)
- Svelte 5 frontend
- SQLite database (org-roam v2 schema)
- [[id:node-cargo][Cargo]] workspace for the Rust crates

* Status
DEADLINE: <2024-06-01 Sat>

- [X] Design architecture
- [X] Implement org parser
- [X] Build database layer
- [ ] Mobile polish
- [ ] App store release

See [[id:node-org-roam][Org Roam]] for the protocol this implements.
`,

	'/vault/daily/2024-01-15.org': `:PROPERTIES:
:ID: node-daily-0115
:END:
#+TITLE: 2024-01-15

* Morning

Worked on the [[id:node-mycelium][Mycelium]] parser today.
Fixed a bug in [[id:node-borrowing][borrowing]] detection in the org-parser crate.

* Afternoon

Read about [[id:node-org-roam][Org Roam]] v2 schema changes.
Updated the [[id:node-cargo][Cargo]] workspace dependencies.

* TODO Review [[id:node-errors][error handling]] patterns
SCHEDULED: <2024-01-16 Tue>
`,
};

// ── Derived data (extracted from file contents, like org-roam's indexer does) ──

interface ParsedLink {
	sourceFile: string;
	sourceNodeId: string;
	targetNodeId: string;
}

// Extract all [[id:...]] links from all files
function extractAllLinks(): ParsedLink[] {
	const links: ParsedLink[] = [];
	const linkRegex = /\[\[id:([^\]]+?)(?:\]\[[^\]]*)?]]/g;

	for (const [filePath, content] of Object.entries(FILES)) {
		// Find the file-level node ID
		const idMatch = content.match(/:ID:\s+(\S+)/);
		if (!idMatch) continue;
		const fileNodeId = idMatch[1];

		let m;
		while ((m = linkRegex.exec(content)) !== null) {
			const targetId = m[1];
			if (targetId !== fileNodeId) {
				links.push({ sourceFile: filePath, sourceNodeId: fileNodeId, targetNodeId: targetId });
			}
		}
	}
	return links;
}

// Build node records from file contents
function buildNodes(): NodeRecord[] {
	const nodes: NodeRecord[] = [];
	const idRegex = /:ID:\s+(\S+)/g;
	const titleRegex = /#\+TITLE:\s+(.+)/;

	for (const [filePath, content] of Object.entries(FILES)) {
		const fileTitle = content.match(titleRegex)?.[1] ?? filePath.split('/').pop()?.replace('.org', '') ?? '';
		let m;
		let isFirst = true;

		// Reset regex
		idRegex.lastIndex = 0;
		while ((m = idRegex.exec(content)) !== null) {
			const id = m[1];
			const pos = m.index;

			// Determine level and title
			let level = 0;
			let title = fileTitle;
			let todo: string | null = null;

			if (!isFirst) {
				// Find the nearest headline before this :ID:
				const before = content.substring(0, pos);
				const headlines = before.split('\n').filter(l => /^\*+ /.test(l));
				const lastHeadline = headlines[headlines.length - 1];
				if (lastHeadline) {
					const stars = lastHeadline.match(/^(\*+)/);
					level = stars ? stars[1].length : 1;
					// Extract title from headline
					let hlText = lastHeadline.replace(/^\*+\s*/, '');
					// Check for TODO keyword
					const todoMatch = hlText.match(/^(TODO|DONE|NEXT)\s+/);
					if (todoMatch) {
						todo = todoMatch[1];
						hlText = hlText.substring(todoMatch[0].length);
					}
					title = hlText.replace(/\s+:[\w:]+:\s*$/, '').trim();
				}
			}

			nodes.push({
				id,
				file: filePath,
				level,
				pos,
				todo,
				priority: null,
				scheduled: null,
				deadline: null,
				title,
				properties: `{"ID":"${id}"}`,
				olp: '[]',
			});
			isFirst = false;
		}
	}
	return nodes;
}

function buildFiles(): FileRecord[] {
	return Object.keys(FILES).map(file => ({
		file,
		title: FILES[file].match(/#\+TITLE:\s+(.+)/)?.[1] ?? file.split('/').pop()?.replace('.org', '') ?? '',
		hash: Math.random().toString(36).substring(2),
		mtime: '1705312800',
	}));
}

const ALL_LINKS = extractAllLinks();
const MOCK_NODES = buildNodes();
const MOCK_FILES = buildFiles();

// Build tag map from #+FILETAGS
const MOCK_TAGS: Record<string, string[]> = {};
for (const [file, content] of Object.entries(FILES)) {
	const tagMatch = content.match(/#\+FILETAGS:\s+:(.+):/);
	if (tagMatch) {
		const tags = tagMatch[1].split(':');
		// Find file-level node
		const fileNode = MOCK_NODES.find(n => n.file === file && n.level === 0);
		if (fileNode) {
			MOCK_TAGS[fileNode.id] = tags;
		}
	}
}

// ── Mock command handlers ──────────────────────────────────────────────────

export const mockHandlers: Record<string, (args: Record<string, unknown>) => unknown> = {
	open_vault: () => ({ total_files: MOCK_FILES.length, indexed: MOCK_FILES.length, skipped: 0, removed: 0 } as SyncResult),
	list_files: () => MOCK_FILES,
	list_nodes: () => MOCK_NODES,
	sync_vault: () => ({ total_files: MOCK_FILES.length, indexed: 0, skipped: MOCK_FILES.length, removed: 0 } as SyncResult),

	get_node: (args) => MOCK_NODES.find(n => n.id === args.id) ?? null,

	// Backlinks: find all files that contain a link TO this node
	get_backlinks: (args) => {
		const targetId = args.nodeId as string;
		const inbound = ALL_LINKS.filter(l => l.targetNodeId === targetId);

		return inbound.map(l => {
			const sourceNode = MOCK_NODES.find(n => n.id === l.sourceNodeId);
			// Extract the actual line containing the link for context
			const fileContent = FILES[l.sourceFile] ?? '';
			const contextLine = fileContent.split('\n').find(line => line.includes(`[[id:${targetId}`));

			return {
				source_id: l.sourceNodeId,
				source_title: sourceNode?.title ?? null,
				source_file: l.sourceFile,
				link_type: 'id',
				context: contextLine?.trim() ?? null,
			} as BacklinkRecord;
		});
	},

	// Forward links: find all [[id:...]] links inside this node's file
	get_forward_links: (args) => {
		const sourceId = args.nodeId as string;
		const outbound = ALL_LINKS.filter(l => l.sourceNodeId === sourceId);

		return outbound.map(l => {
			const targetNode = MOCK_NODES.find(n => n.id === l.targetNodeId);
			return {
				dest_id: l.targetNodeId,
				dest_title: targetNode?.title ?? l.targetNodeId,
				dest_file: targetNode?.file ?? null,
				link_type: 'id',
			} as ForwardLink;
		});
	},

	search_nodes: (args) => {
		const q = (args.query as string).toLowerCase();
		return MOCK_NODES.filter(n => n.title?.toLowerCase().includes(q));
	},

	search_full: (args) => {
		const q = (args.query as string).toLowerCase();
		const results: SearchResult[] = [];

		// Title matches
		for (const n of MOCK_NODES) {
			if (n.title?.toLowerCase().includes(q)) {
				results.push({ id: n.id, file: n.file, title: n.title, snippet: null, match_type: 'title' });
			}
		}

		// Body content matches
		for (const [file, content] of Object.entries(FILES)) {
			if (content.toLowerCase().includes(q)) {
				const fileNode = MOCK_NODES.find(n => n.file === file && n.level === 0);
				if (fileNode && !results.some(r => r.id === fileNode.id)) {
					// Extract snippet
					const lines = content.split('\n');
					const matchLine = lines.find(l => l.toLowerCase().includes(q));
					results.push({
						id: fileNode.id,
						file,
						title: fileNode.title,
						snippet: matchLine?.trim() ?? null,
						match_type: 'content',
					});
				}
			}
		}

		return results;
	},

	// Read file: return the actual org content
	read_file: (args) => {
		const filePath = args.filePath as string;
		// Direct file path match
		if (FILES[filePath]) return FILES[filePath];
		// Try matching by node file field
		const node = MOCK_NODES.find(n => n.file === filePath);
		if (node && FILES[node.file]) return FILES[node.file];
		return '#+TITLE: Not Found\n\nFile not found in demo vault.\n';
	},

	save_file: () => undefined,
	create_file: () => '/vault/new-note.org',

	get_graph_data: () => {
		const linkCounts: Record<string, number> = {};
		for (const l of ALL_LINKS) {
			linkCounts[l.sourceNodeId] = (linkCounts[l.sourceNodeId] ?? 0) + 1;
			linkCounts[l.targetNodeId] = (linkCounts[l.targetNodeId] ?? 0) + 1;
		}

		// Only include file-level nodes in the graph (level 0)
		const graphNodes = MOCK_NODES.filter(n => n.level === 0).map(n => ({
			id: n.id,
			title: n.title,
			tags: MOCK_TAGS[n.id] ?? [],
			link_count: linkCounts[n.id] ?? 0,
		}));

		// Deduplicate links (only between file-level nodes)
		const fileNodeIds = new Set(graphNodes.map(n => n.id));
		const seen = new Set<string>();
		const graphLinks = ALL_LINKS
			.filter(l => fileNodeIds.has(l.sourceNodeId) && fileNodeIds.has(l.targetNodeId))
			.filter(l => {
				const key = `${l.sourceNodeId}->${l.targetNodeId}`;
				if (seen.has(key)) return false;
				seen.add(key);
				return true;
			})
			.map(l => ({ source: l.sourceNodeId, target: l.targetNodeId }));

		return { nodes: graphNodes, links: graphLinks } as GraphData;
	},

	get_or_create_daily: (args) => {
		const date = args.date as string;
		const existing = MOCK_NODES.find(n => n.title === date);
		if (existing) return existing;
		// Return the existing daily note as fallback
		return MOCK_NODES.find(n => n.title?.match(/^\d{4}-\d{2}-\d{2}/)) ?? MOCK_NODES[0];
	},

	list_daily_notes: () => MOCK_NODES.filter(n => /^\d{4}-\d{2}-\d{2}/.test(n.title ?? '')),

	get_all_tags: () => {
		const counts: Record<string, number> = {};
		for (const tags of Object.values(MOCK_TAGS)) {
			for (const tag of tags) {
				counts[tag] = (counts[tag] ?? 0) + 1;
			}
		}
		return Object.entries(counts)
			.map(([tag, count]) => ({ tag, count }))
			.sort((a, b) => b.count - a.count) as TagCount[];
	},

	get_nodes_by_tag: (args) => {
		const tag = args.tag as string;
		return MOCK_NODES.filter(n => (MOCK_TAGS[n.id] ?? []).includes(tag));
	},

	export_markdown: (args) => {
		const content = FILES[args.filePath as string] ?? '';
		// Simple org-to-markdown conversion for demo
		return content
			.replace(/:PROPERTIES:[\s\S]*?:END:\n/g, '')
			.replace(/^#\+TITLE:\s+(.+)/m, '# $1')
			.replace(/^#\+\w+:.*\n/gm, '')
			.replace(/^\*\*\*\s+/gm, '### ')
			.replace(/^\*\*\s+/gm, '## ')
			.replace(/^\*\s+/gm, '## ')
			.replace(/\*([^*]+)\*/g, '**$1**')
			.replace(/\/([^/]+)\//g, '_$1_')
			.replace(/~([^~]+)~/g, '`$1`')
			.replace(/=([^=]+)=/g, '`$1`')
			.replace(/\[\[id:[^\]]+\]\[([^\]]*)\]\]/g, '[[$1]]');
	},

	export_html: () => '<html><body><h1>Exported Note</h1><p>HTML export preview.</p></body></html>',
};
