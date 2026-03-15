export interface NodeRecord {
	id: string;
	file: string;
	level: number;
	pos: number;
	todo: string | null;
	priority: string | null;
	scheduled: string | null;
	deadline: string | null;
	title: string | null;
	properties: string | null;
	olp: string | null;
}

export interface BacklinkRecord {
	source_id: string;
	source_title: string | null;
	source_file: string;
	link_type: string;
	context: string | null;
}

export interface ForwardLink {
	dest_id: string;
	dest_title: string | null;
	dest_file: string | null;
	link_type: string;
}

export interface GraphNode {
	id: string;
	title: string | null;
	tags: string[];
	link_count: number;
}

export interface GraphLink {
	source: string;
	target: string;
}

export interface GraphData {
	nodes: GraphNode[];
	links: GraphLink[];
}

export interface SearchResult {
	id: string;
	file: string;
	title: string | null;
	snippet: string | null;
	match_type: string;
}

export interface TagCount {
	tag: string;
	count: number;
}
