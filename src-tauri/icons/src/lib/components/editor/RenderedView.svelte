<script lang="ts">
	import { onMount } from 'svelte';

	let { content = '', vaultPath = '', onLinkClick, onContentChange }: {
		content: string;
		vaultPath?: string;
		onLinkClick?: (id: string) => void;
		onContentChange?: (newContent: string) => void;
	} = $props();

	const imgExts = new Set(['png','jpg','jpeg','gif','svg','webp','bmp','ico']);
	let el: HTMLDivElement;

	onMount(() => { renderContent(); });
	$effect(() => { if (el && content) renderContent(); });

	function renderContent() {
		while (el.firstChild) el.removeChild(el.firstChild);
		const lines = content.split('\n');
		let i = 0;

		// Track sections for collapsing: each heading gets a body container
		let currentBody: HTMLElement = el;

		while (i < lines.length) {
			const t = lines[i].trim();
			if (!t) { i++; continue; }
			if (t === ':PROPERTIES:') { i++; while (i < lines.length && lines[i].trim() !== ':END:') i++; i++; continue; }
			if (/^:[A-Z_-]+:\s*$/.test(t) && t !== ':END:') { i++; while (i < lines.length && lines[i].trim() !== ':END:') i++; i++; continue; }
			if (t.startsWith('#+') && !/^#\+BEGIN_/i.test(t)) { i++; continue; }

			// Headlines — collapsible
			const hm = t.match(/^(\*+)\s+(.*)/);
			if (hm) {
				const level = hm[1].length;
				// Reset to top level container
				currentBody = el;

				const section = mk('div', '');
				const header = mk('div', 'margin-top:' + (level === 1 ? '20px' : '12px') + ';display:flex;align-items:center;gap:6px;flex-wrap:wrap;cursor:pointer;user-select:none');
				const body = mk('div', '');

				// Collapse triangle
				const arrow = mk('span', 'font-size:11px;color:#94a3b8;transition:transform 0.15s;flex-shrink:0;width:12px;text-align:center');
				arrow.textContent = '\u25BE';
				header.appendChild(arrow);

				let rest = hm[2];
				const tw = rest.match(/^(TODO|DONE|NEXT|WAITING|HOLD|CANCELLED)\s+/);
				if (tw) { const b = mk('span', 'font-size:11px;font-weight:700;padding:1px 5px;border-radius:4px;' + (tw[1]==='DONE'?'color:#16a34a;background:#f0fdf4':'color:#dc2626;background:#fef2f2')); b.textContent = tw[1]; header.appendChild(b); rest = rest.slice(tw[0].length); }
				const pm = rest.match(/^\[#([A-Z])\]\s*/);
				if (pm) { const p = mk('span','font-size:11px;font-weight:700;color:#ea580c'); p.textContent='[#'+pm[1]+']'; header.appendChild(p); rest = rest.slice(pm[0].length); }
				const tgm = rest.match(/\s+(:[a-zA-Z0-9_:]+:)\s*$/);
				let tags: string[] = [];
				if (tgm) { tags = tgm[1].split(':').filter(Boolean); rest = rest.slice(0, tgm.index!).trim(); }
				const sizes: Record<number,string> = {1:'1.4rem',2:'1.15rem',3:'1rem'};
				const clrs: Record<number,string> = {1:'#15803d',2:'#166534',3:'#14532d'};
				const h = mk('span', 'font-size:'+(sizes[level]||'0.95rem')+';font-weight:700;color:'+(clrs[level]||'#14532d'));
				inl(h, rest); header.appendChild(h);
				for (const tag of tags) { const s = mk('span','font-size:10px;padding:1px 6px;border-radius:99px;background:#ede9fe;color:#6d28d9;font-weight:500;margin-left:auto'); s.textContent=tag; header.appendChild(s); }

				let collapsed = false;
				header.addEventListener('click', () => {
					collapsed = !collapsed;
					body.style.display = collapsed ? 'none' : '';
					arrow.style.transform = collapsed ? 'rotate(-90deg)' : '';
				});

				section.appendChild(header);
				section.appendChild(body);
				el.appendChild(section);
				currentBody = body;
				i++; continue;
			}

			// Blocks
			const bm = t.match(/^#\+BEGIN_(\w+)\s*(.*)/i);
			if (bm) {
				const bt=bm[1].toUpperCase(), lang=bm[2]||'';
				const bl: string[]=[]; i++;
				while(i<lines.length && !/^#\+END_/i.test(lines[i].trim())){bl.push(lines[i]);i++;} i++;
				if(bt==='QUOTE'){const bq=mk('blockquote','border-left:3px solid #d1d5db;padding-left:12px;margin:12px 0;color:#6b7280;font-style:italic');inl(bq,bl.join('\n'));currentBody.appendChild(bq);}
				else{const w=mk('div','margin:12px 0;border-radius:8px;overflow:hidden;border:1px solid #e2e8f0;background:#ffffff');if(lang){const l=mk('div','font-size:11px;font-weight:600;color:#6b7280;background:#f8fafc;padding:4px 12px;border-bottom:1px solid #e2e8f0;text-transform:uppercase');l.textContent=lang;w.appendChild(l);}const pre=mk('pre','margin:0;padding:12px;background:#fafbfc;color:#1e293b;overflow-x:auto;font-family:monospace;font-size:13px;line-height:1.6');pre.textContent=bl.join('\n');w.appendChild(pre);currentBody.appendChild(w);}
				continue;
			}

			// Tables
			if(t.startsWith('|')){
				const tl:string[]=[];while(i<lines.length&&lines[i].trim().startsWith('|')){tl.push(lines[i].trim());i++;}
				const rows:string[][]=[];let hr=false;
				for(const r of tl){if(/^\|[-+]+/.test(r)){hr=true;continue;}rows.push(r.replace(/^\|/,'').replace(/\|$/,'').split('|').map(c=>c.trim()));}
				const w=mk('div','overflow-x:auto;margin:12px 0;border-radius:8px;border:1px solid #e2e8f0;background:#ffffff');
				const tbl=document.createElement('table');tbl.style.cssText='border-collapse:collapse;width:100%;font-size:14px';
				if(hr&&rows.length>1){const th=document.createElement('thead');const tr=document.createElement('tr');rows[0].forEach(c=>{const td=mk('th','background:#f8fafc;color:#1e293b;font-weight:600;text-align:left;padding:8px 12px;border-bottom:2px solid #e2e8f0');inl(td,c);tr.appendChild(td);});th.appendChild(tr);tbl.appendChild(th);const tb=document.createElement('tbody');rows.slice(1).forEach(r=>{const tr=document.createElement('tr');r.forEach(c=>{const td=mk('td','padding:8px 12px;border-bottom:1px solid #f1f5f9;color:#1e293b');inl(td,c);tr.appendChild(td);});tb.appendChild(tr);});tbl.appendChild(tb);}
				else{const tb=document.createElement('tbody');rows.forEach(r=>{const tr=document.createElement('tr');r.forEach(c=>{const td=mk('td','padding:8px 12px;border-bottom:1px solid #f1f5f9;color:#1e293b');inl(td,c);tr.appendChild(td);});tb.appendChild(tr);});tbl.appendChild(tb);}
				w.appendChild(tbl);currentBody.appendChild(w);continue;
			}

			// Lists with interactive checkboxes
			if(/^[-+]\s|^\d+[.)]\s/.test(t)){
				const items:{text:string;lineIdx:number}[]=[];
				while(i<lines.length){const lt=lines[i].trim();if(/^[-+]\s|^\d+[.)]\s/.test(lt)){items.push({text:lt,lineIdx:i});i++;}else if(lt&&lines[i].startsWith(' ')&&items.length){items[items.length-1].text+=' '+lt;i++;}else break;}
				const ul=mk('ul','margin:8px 0;padding-left:20px');
				items.forEach(item=>{let r=item.text;const bx=r.match(/^([-+]|\d+[.)]) /);if(bx)r=r.slice(bx[0].length);
				const li=mk('li','margin:4px 0;display:flex;align-items:flex-start');
				const lineIdx = item.lineIdx;
				if(r.startsWith('[ ] ')){
					const cb=document.createElement('input');cb.type='checkbox';cb.style.cssText='accent-color:#16a34a;margin-right:6px;margin-top:4px;cursor:pointer;flex-shrink:0';
					cb.addEventListener('change',()=>{ toggleCheckbox(lineIdx, cb.checked); });
					li.appendChild(cb);r=r.slice(4);
				} else if(/^\[[Xx]\] /.test(r)){
					const cb=document.createElement('input');cb.type='checkbox';cb.checked=true;cb.style.cssText='accent-color:#16a34a;margin-right:6px;margin-top:4px;cursor:pointer;flex-shrink:0';
					cb.addEventListener('change',()=>{ toggleCheckbox(lineIdx, cb.checked); });
					li.appendChild(cb);r=r.slice(4);
				}
				const span = mk('span',''); inl(span,r); li.appendChild(span);
				ul.appendChild(li);});
				currentBody.appendChild(ul);continue;
			}

			// Standalone image: [[file:path.png]] or [[./path.png]]
			const imgMatch = t.match(/^\[\[(?:file:)?([^\]]+?)\]\]$/);
			if (imgMatch) {
				const path = imgMatch[1];
				const ext = path.split('.').pop()?.toLowerCase() ?? '';
				if (imgExts.has(ext)) {
					const imgWrap = mk('div', 'margin:12px 0;text-align:center');
					const img = document.createElement('img');
					img.src = resolveImagePath(path);
					img.alt = path.split('/').pop() ?? path;
					img.style.cssText = 'max-width:100%;border-radius:8px;border:1px solid #e2e8f0';
					img.onerror = () => { img.style.display = 'none'; const fallback = mk('p','font-size:13px;color:#9ca3af;font-style:italic'); fallback.textContent = 'Image not found: ' + path; imgWrap.appendChild(fallback); };
					imgWrap.appendChild(img);
					currentBody.appendChild(imgWrap);
					i++; continue;
				}
			}

			// Planning
			if(/^(SCHEDULED|DEADLINE|CLOSED):/.test(t)){const p=mk('p','font-size:13px;color:#6b7280;margin:4px 0');inl(p,t);currentBody.appendChild(p);i++;continue;}

			// Paragraph
			const pl:string[]=[];
			while(i<lines.length){const x=lines[i].trim();if(!x||x.startsWith('*')||x.startsWith('|')||x.startsWith('#+')||x.startsWith(':PROPERTIES:')||/^[-+]\s|^\d+[.)]\s/.test(x)||/^(SCHEDULED|DEADLINE|CLOSED):/.test(x))break;pl.push(x);i++;}
			if(pl.length){const p=mk('p','margin:6px 0');inl(pl.join(' '),p);currentBody.appendChild(p);}else i++;
		}
	}

	function toggleCheckbox(lineIdx: number, checked: boolean) {
		const lines = content.split('\n');
		if (lineIdx >= 0 && lineIdx < lines.length) {
			if (checked) lines[lineIdx] = lines[lineIdx].replace('[ ]', '[X]');
			else lines[lineIdx] = lines[lineIdx].replace(/\[[Xx]\]/, '[ ]');
			onContentChange?.(lines.join('\n'));
		}
	}

	function mk(tag: string, css: string): HTMLElement {
		const e = document.createElement(tag);
		if (css) e.style.cssText = css;
		return e;
	}

	function resolveImagePath(path: string): string {
		// If absolute, use as-is; if relative, resolve against vault path
		if (path.startsWith('/') || path.startsWith('http')) return path;
		if (vaultPath) return vaultPath.replace(/\/$/, '') + '/' + path.replace(/^\.\//, '');
		return path;
	}

	function isImagePath(path: string): boolean {
		const ext = path.split('.').pop()?.toLowerCase() ?? '';
		return imgExts.has(ext);
	}

	function inl(target: HTMLElement | string, parentOrText?: HTMLElement | string) {
		// Handle both inl(parent, text) and inl(text, parent) for backward compat
		let parent: HTMLElement;
		let text: string;
		if (typeof target === 'string') { text = target; parent = parentOrText as HTMLElement; }
		else { parent = target; text = parentOrText as string; }

		let r = text;
		while (r.length > 0) {
			let best: {idx:number;len:number;fn:()=>Node}|null = null;

			const lm = r.match(/\[\[id:([^\]]+?)\]\[([^\]]*?)\]\]/);
			if (lm?.index !== undefined && (!best || lm.index < best.idx))
				best = {idx:lm.index, len:lm[0].length, fn:()=>{ const s=mk('span','color:#16a34a;text-decoration:underline;text-underline-offset:2px;cursor:pointer;font-weight:500'); s.textContent=lm![2]; s.addEventListener('click',(e)=>{e.preventDefault();e.stopPropagation();onLinkClick?.(lm![1]);}); return s; }};

			const lm2 = r.match(/\[\[id:([^\]]+?)\]\]/);
			if (lm2?.index !== undefined && (!best || lm2.index < best.idx))
				best = {idx:lm2.index, len:lm2[0].length, fn:()=>{ const s=mk('span','color:#16a34a;text-decoration:underline;text-underline-offset:2px;cursor:pointer;font-weight:500'); s.textContent=lm2![1]; s.addEventListener('click',(e)=>{e.preventDefault();e.stopPropagation();onLinkClick?.(lm2![1]);}); return s; }};

			// Inline image: [[file:img.png]] or [[./img.png]]
			const fim = r.match(/\[\[(?:file:)?([^\]]+?)\]\]/);
			if (fim?.index !== undefined && isImagePath(fim[1]) && (!best || fim.index < best.idx))
				best = {idx:fim.index, len:fim[0].length, fn:()=>{ const img=document.createElement('img'); img.src=resolveImagePath(fim![1]); img.alt=fim![1].split('/').pop()??fim![1]; img.style.cssText='max-width:100%;border-radius:6px;display:inline-block;vertical-align:middle;max-height:300px'; return img; }};

			const bm = r.match(/(^|[\s(])\*(\S[^*]*?\S|\S)\*([\s.,;:!?)]|$)/);
			if (bm?.index !== undefined) { const off=bm[1].length; const idx=bm.index+off; if(!best||idx<best.idx) best={idx,len:bm[2].length+2,fn:()=>{const b=document.createElement('b');b.textContent=bm![2];return b;}}; }

			const im = r.match(/(^|[\s(])\/(\S[^/]*?\S|\S)\/([\s.,;:!?)]|$)/);
			if (im?.index !== undefined) { const off=im[1].length; const idx=im.index+off; if(!best||idx<best.idx) best={idx,len:im[2].length+2,fn:()=>{const e=document.createElement('i');e.textContent=im![2];return e;}}; }

			const cm = r.match(/~(\S[^~]*?\S|\S)~/);
			if (cm?.index !== undefined && (!best||cm.index<best.idx)) best={idx:cm.index,len:cm[0].length,fn:()=>{const c=mk('code','font-family:monospace;font-size:0.85em;background:#e2e8f0;padding:1px 4px;border-radius:3px;color:#be123c');c.textContent=cm![1];return c;}};

			const vm = r.match(/(^|[\s(])=(\S[^=]*?\S|\S)=([\s.,;:!?)]|$)/);
			if (vm?.index !== undefined) { const off=vm[1].length; const idx=vm.index+off; if(!best||idx<best.idx) best={idx,len:vm[2].length+2,fn:()=>{const c=mk('code','font-family:monospace;font-size:0.85em;background:#e2e8f0;padding:1px 4px;border-radius:3px;color:#1e293b');c.textContent=vm![2];return c;}}; }

			const sm = r.match(/(^|[\s(])\+(\S[^+]*?\S|\S)\+([\s.,;:!?)]|$)/);
			if (sm?.index !== undefined) { const off=sm[1].length; const idx=sm.index+off; if(!best||idx<best.idx) best={idx,len:sm[2].length+2,fn:()=>{const s=document.createElement('s');s.style.color='#9ca3af';s.textContent=sm![2];return s;}}; }

			const um = r.match(/(^|[\s(])_(\S[^_]*?\S|\S)_([\s.,;:!?)]|$)/);
			if (um?.index !== undefined) { const off=um[1].length; const idx=um.index+off; if(!best||idx<best.idx) best={idx,len:um[2].length+2,fn:()=>{const u=document.createElement('u');u.textContent=um![2];return u;}}; }

			const tsm = r.match(/<(\d{4}-\d{2}-\d{2}[^>]*?)>/);
			if (tsm?.index !== undefined && (!best||tsm.index<best.idx)) best={idx:tsm.index,len:tsm[0].length,fn:()=>{const s=mk('span','font-size:0.8em;color:#7c3aed;background:#f5f3ff;padding:1px 4px;border-radius:3px');s.textContent=tsm![1];return s;}};

			if (best) {
				if (best.idx > 0) parent.appendChild(document.createTextNode(r.slice(0, best.idx)));
				parent.appendChild(best.fn());
				r = r.slice(best.idx + best.len);
			} else {
				parent.appendChild(document.createTextNode(r));
				break;
			}
		}
	}
</script>

<div bind:this={el} style="font-family:system-ui,-apple-system,sans-serif;line-height:1.75"></div>
