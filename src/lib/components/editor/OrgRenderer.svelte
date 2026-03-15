<script lang="ts">
	import { onMount } from 'svelte';
	import { navigation } from '$lib/stores/navigation.svelte';
	import { editor } from '$lib/stores/editor.svelte';

	let { content = '' }: { content: string } = $props();
	let container: HTMLElement;

	$effect(() => {
		if (container && content) {
			const html = renderOrg(content);
			console.log('[OrgRenderer] $effect setting innerHTML, length:', html.length, 'first 200:', html.substring(0, 200));
			container.innerHTML = html;
		}
	});

	onMount(() => {
		if (content) {
			const html = renderOrg(content);
			console.log('[OrgRenderer] onMount setting innerHTML, length:', html.length, 'first 200:', html.substring(0, 200));
			container.innerHTML = html;
		}

		container.addEventListener('click', (e) => {
			const target = e.target as HTMLElement;

			// Link navigation
			const link = target.closest('[data-nid]') as HTMLElement | null;
			if (link) {
				e.preventDefault();
				const nid = link.getAttribute('data-nid');
				if (nid) navigation.navigateToNode(nid);
				return;
			}

			// Checkbox toggle
			if (target.tagName === 'INPUT' && target.getAttribute('type') === 'checkbox') {
				const li = target.closest('[data-li]');
				if (li) {
					const idx = parseInt(li.getAttribute('data-li') ?? '-1');
					if (idx >= 0) {
						const lines = editor.content.split('\n');
						if ((target as HTMLInputElement).checked) {
							lines[idx] = lines[idx].replace('[ ]', '[X]');
						} else {
							lines[idx] = lines[idx].replace(/\[[Xx]\]/, '[ ]');
						}
						editor.updateContent(lines.join('\n'));
					}
				}
			}
		});
	});

	// ── Styles (inline for Safari compatibility) ──────────────────────────

	const S = {
		link: 'color:#16a34a;text-decoration:underline;text-underline-offset:2px;cursor:pointer;font-weight:500',
		flink: 'color:#0891b2;text-decoration:underline;text-decoration-style:dotted',
		b: 'font-weight:700',
		i: 'font-style:italic',
		u: 'text-decoration:underline',
		del: 'text-decoration:line-through;color:#9ca3af',
		code: 'font-family:ui-monospace,monospace;font-size:0.85em;background:#f1f5f9;padding:0.1em 0.3em;border-radius:3px;color:#be123c',
		verb: 'font-family:ui-monospace,monospace;font-size:0.85em;background:#f1f5f9;padding:0.1em 0.3em;border-radius:3px',
		ts: 'font-size:0.8em;color:#7c3aed;background:#f5f3ff;padding:0.1em 0.25em;border-radius:3px',
		todo: 'font-size:0.7rem;font-weight:700;color:#dc2626;background:#fef2f2;padding:0.1rem 0.35rem;border-radius:4px;margin-right:0.3rem',
		done: 'font-size:0.7rem;font-weight:700;color:#16a34a;background:#f0fdf4;padding:0.1rem 0.35rem;border-radius:4px;margin-right:0.3rem',
		prio: 'font-size:0.7rem;font-weight:700;color:#ea580c;margin-right:0.3rem',
		tag: 'font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:99px;background:#ede9fe;color:#6d28d9;font-weight:500;margin-left:0.2rem',
		plan: 'font-size:0.8rem;color:#6b7280;margin:0.2rem 0',
		codeLang: 'font-size:0.7rem;font-weight:600;color:#6b7280;background:#f8fafc;padding:0.3rem 0.75rem;border-bottom:1px solid #e2e8f0;text-transform:uppercase;letter-spacing:0.05em',
		codeWrap: 'margin:0.75rem 0;border-radius:8px;overflow:hidden;border:1px solid #e2e8f0',
		codePre: 'margin:0;padding:0.75rem 1rem;background:#fafbfc;overflow-x:auto;font-family:ui-monospace,monospace;font-size:0.82rem;line-height:1.6',
		quote: 'border-left:3px solid #d1d5db;padding-left:1rem;margin:0.75rem 0;color:#6b7280;font-style:italic',
		tblWrap: 'overflow-x:auto;margin:0.75rem 0;border-radius:8px;border:1px solid #e2e8f0',
		tbl: 'border-collapse:collapse;width:100%;font-size:0.85rem',
		th: 'background:#f8fafc;font-weight:600;text-align:left;padding:0.5rem 0.75rem;border-bottom:2px solid #e2e8f0',
		td: 'padding:0.5rem 0.75rem;border-bottom:1px solid #f1f5f9',
		cb: 'margin-top:0.2rem;accent-color:#16a34a;cursor:pointer;margin-right:0.3rem',
	} as const;

	const hlStyles: Record<number, string> = {
		1: 'font-size:1.4rem;font-weight:700;color:#15803d;margin:0',
		2: 'font-size:1.15rem;font-weight:700;color:#166534;margin:0',
		3: 'font-size:1rem;font-weight:600;color:#14532d;margin:0',
		4: 'font-size:0.95rem;font-weight:600;margin:0',
	};

	// ── Rendering ─────────────────────────────────────────────────────────

	function renderOrg(text: string): string {
		const lines = text.split('\n');
		const parts: string[] = [];
		let i = 0;

		while (i < lines.length) {
			const t = lines[i].trim();
			if (t === '') { i++; continue; }

			// Property drawers — hide
			if (t === ':PROPERTIES:') { i++; while (i < lines.length && lines[i].trim() !== ':END:') i++; i++; continue; }
			// Other drawers — hide
			if (/^:[A-Z_-]+:\s*$/.test(t) && t !== ':END:') { i++; while (i < lines.length && lines[i].trim() !== ':END:') i++; i++; continue; }
			// Metadata — hide
			if (t.startsWith('#+') && !/^#\+BEGIN_/i.test(t)) { i++; continue; }

			// Headlines
			const hm = t.match(/^(\*+)\s+(.*)/);
			if (hm) {
				const stars = hm[1].length;
				const lvl = Math.min(stars + 1, 6);
				let rest = hm[2];
				let pre = '';
				const tm = rest.match(/^(TODO|DONE|NEXT|WAITING|HOLD|CANCELLED)\s+/);
				if (tm) { pre += `<span style="${tm[1] === 'DONE' ? S.done : S.todo}">${tm[1]}</span>`; rest = rest.slice(tm[0].length); }
				const pm = rest.match(/^\[#([A-Z])\]\s*/);
				if (pm) { pre += `<span style="${S.prio}">[#${pm[1]}]</span>`; rest = rest.slice(pm[0].length); }
				let tagHtml = '';
				const tgm = rest.match(/\s+(:[a-zA-Z0-9_:]+:)\s*$/);
				if (tgm) { tagHtml = tgm[1].split(':').filter(Boolean).map(x => `<span style="${S.tag}">${esc(x)}</span>`).join(''); rest = rest.slice(0, tgm.index!).trim(); }
				const hs = hlStyles[stars] || hlStyles[4];
				parts.push(`<div style="display:flex;align-items:baseline;gap:0.4rem;flex-wrap:wrap;margin-top:${stars === 1 ? '1.5rem' : '0.8rem'}">${pre}<h${lvl} style="${hs}">${mk(rest)}</h${lvl}>${tagHtml ? `<span style="display:inline-flex;gap:0.2rem;margin-left:auto">${tagHtml}</span>` : ''}</div>`);
				i++; continue;
			}

			// Blocks
			const bm = t.match(/^#\+BEGIN_(\w+)\s*(.*)/i);
			if (bm) {
				const btype = bm[1].toUpperCase(); const lang = bm[2] || '';
				const bl: string[] = []; i++;
				while (i < lines.length && !/^#\+END_/i.test(lines[i].trim())) { bl.push(lines[i]); i++; } i++;
				if (btype === 'QUOTE') parts.push(`<blockquote style="${S.quote}">${mk(bl.join('\n'))}</blockquote>`);
				else parts.push(`<div style="${S.codeWrap}">${lang ? `<div style="${S.codeLang}">${esc(lang)}</div>` : ''}<pre style="${S.codePre}">${esc(bl.join('\n'))}</pre></div>`);
				continue;
			}

			// Tables
			if (t.startsWith('|')) {
				const tl: string[] = [];
				while (i < lines.length && lines[i].trim().startsWith('|')) { tl.push(lines[i].trim()); i++; }
				const rows: string[][] = []; let hasRule = false;
				for (const r of tl) { if (/^\|[-+]+/.test(r)) { hasRule = true; continue; } rows.push(r.replace(/^\|/, '').replace(/\|$/, '').split('|').map(c => c.trim())); }
				let h = `<div style="${S.tblWrap}"><table style="${S.tbl}">`;
				if (hasRule && rows.length > 1) {
					h += '<thead><tr>' + rows[0].map(c => `<th style="${S.th}">${mk(c)}</th>`).join('') + '</tr></thead>';
					h += '<tbody>' + rows.slice(1).map(r => '<tr>' + r.map(c => `<td style="${S.td}">${mk(c)}</td>`).join('') + '</tr>').join('') + '</tbody>';
				} else { h += '<tbody>' + rows.map(r => '<tr>' + r.map(c => `<td style="${S.td}">${mk(c)}</td>`).join('') + '</tr>').join('') + '</tbody>'; }
				h += '</table></div>';
				parts.push(h); continue;
			}

			// Lists
			if (/^[-+]\s|^\d+[.)]\s/.test(t)) {
				const items: { text: string; lineIdx: number }[] = [];
				while (i < lines.length) {
					const lt = lines[i].trim();
					if (/^[-+]\s|^\d+[.)]\s/.test(lt)) { items.push({ text: lt, lineIdx: i }); i++; }
					else if (lt !== '' && lines[i].startsWith(' ') && items.length > 0) { items[items.length - 1].text += ' ' + lt; i++; }
					else break;
				}
				const lis = items.map(item => {
					let r = item.text;
					const bMatch = r.match(/^([-+]|\d+[.)]) /);
					if (bMatch) r = r.slice(bMatch[0].length);
					let cb = '';
					if (r.startsWith('[ ] ')) { cb = `<input type="checkbox" style="${S.cb}">`; r = r.slice(4); }
					else if (/^\[[Xx]\] /.test(r)) { cb = `<input type="checkbox" checked style="${S.cb}">`; r = r.slice(4); }
					return `<li style="margin:0.3rem 0;display:flex;align-items:flex-start" data-li="${item.lineIdx}">${cb}<span>${mk(r)}</span></li>`;
				}).join('');
				parts.push(`<ul style="margin:0.5rem 0;padding-left:1.25rem">${lis}</ul>`); continue;
			}

			// Planning
			if (/^(SCHEDULED|DEADLINE|CLOSED):/.test(t)) { parts.push(`<p style="${S.plan}">${mk(t)}</p>`); i++; continue; }

			// Paragraph
			const pl: string[] = [];
			while (i < lines.length) {
				const p = lines[i].trim();
				if (p === '' || p.startsWith('*') || p.startsWith('|') || p.startsWith('#+') || p.startsWith(':PROPERTIES:') || /^[-+]\s|^\d+[.)]\s/.test(p) || /^(SCHEDULED|DEADLINE|CLOSED):/.test(p)) break;
				pl.push(p); i++;
			}
			if (pl.length > 0) parts.push(`<p style="margin:0.4rem 0">${mk(pl.join(' '))}</p>`);
			else i++;
		}
		return parts.join('\n');
	}

	function mk(text: string): string {
		let s = esc(text);
		// Links [[id:xxx][desc]]
		s = s.replace(/\[\[id:([^\]]+?)\]\[([^\]]*?)\]\]/g, `<span data-nid="$1" style="${S.link}">$2</span>`);
		// Links [[id:xxx]]
		s = s.replace(/\[\[id:([^\]]+?)\]\]/g, `<span data-nid="$1" style="${S.link}">$1</span>`);
		// Other links
		s = s.replace(/\[\[[^\]]+?\]\[([^\]]*?)\]\]/g, `<span style="${S.flink}">$1</span>`);
		// Bold
		s = s.replace(/(^|[\s(])\*(\S[^*]*?\S|\S)\*([\s.,;:!?)]|$)/g, `$1<span style="${S.b}">$2</span>$3`);
		// Italic
		s = s.replace(/(^|[\s(])\/(\S[^/]*?\S|\S)\/([\s.,;:!?)]|$)/g, `$1<span style="${S.i}">$2</span>$3`);
		// Underline
		s = s.replace(/(^|[\s(])_(\S[^_]*?\S|\S)_([\s.,;:!?)]|$)/g, `$1<span style="${S.u}">$2</span>$3`);
		// Strikethrough
		s = s.replace(/(^|[\s(])\+(\S[^+]*?\S|\S)\+([\s.,;:!?)]|$)/g, `$1<span style="${S.del}">$2</span>$3`);
		// Code ~text~
		s = s.replace(/~(\S[^~]*?\S|\S)~/g, `<span style="${S.code}">$1</span>`);
		// Verbatim =text=
		s = s.replace(/=(\S[^=]*?\S|\S)=/g, `<span style="${S.verb}">$1</span>`);
		// Timestamps
		s = s.replace(/(&lt;)(\d{4}-\d{2}-\d{2}[^&]*?)(&gt;)/g, `<span style="${S.ts}">$2</span>`);
		s = s.replace(/\[(\d{4}-\d{2}-\d{2}[^\]]*?)\]/g, `<span style="${S.ts}">$1</span>`);
		return s;
	}

	function esc(s: string): string {
		return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	}
</script>

<article bind:this={container} style="font-family:system-ui,-apple-system,sans-serif;line-height:1.75;"></article>
