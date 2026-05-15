// =================================================================
// SEARCH — debounced input + infinite scroll results
// =================================================================
import { State } from './state.js';
import { Dom } from './dom.js';
import { Api } from './api.js';
import { Icon } from './icons.js';
import { openPlayer } from './player.js';
import { openImageViewer } from './image-viewer.js';

let searchJobId = 0;

export async function performSearch() {
    const query = Dom.searchInput.value.trim();
    const job = ++searchJobId;
    State.set('searchQuery', query);
    State.set('searchResults', []);
    State.set('searchPage', 0);
    State.set('searchTotal', 0);
    Dom.searchLoadMore.classList.add('hidden');

    if (!query) { Dom.searchGrid.innerHTML = ''; Dom.searchEmpty.classList.remove('hidden'); Dom.searchEmptyText.textContent = '搜索视频标题'; return; }

    await new Promise(r => setTimeout(r, 300));
    if (job !== searchJobId) return;

    try {
        const data = await Api.listVideos({ query, page: 0, size: 20 });
        if (job !== searchJobId) return;
        const items = data.items || [];
        State.set('searchResults', items);
        State.set('searchPage', 0);
        State.set('searchTotal', data.total || 0);
        if (!items.length) { Dom.searchGrid.innerHTML = ''; Dom.searchEmpty.classList.remove('hidden'); Dom.searchEmptyText.textContent = `没有找到 "${query}" 的结果`; return; }
        Dom.searchEmpty.classList.add('hidden');
        renderSearchGrid(items);
    } catch (err) {
        if (job !== searchJobId) return;
        Dom.searchGrid.innerHTML = '';
        Dom.searchEmpty.classList.remove('hidden');
        Dom.searchEmptyText.textContent = `搜索失败: ${err.message}`;
    }
}

export function renderSearchGrid(items) {
    Dom.searchGrid.innerHTML = '';
    items.forEach(item => {
        const div = document.createElement('div');
        div.className = 'grid-item';
        div.dataset.id = item.id;
        div.setAttribute('role', 'button');
        div.setAttribute('tabindex', '0');
        div.setAttribute('aria-label', item.title || '未知');
        const isImage = item.sourceType && item.sourceType.includes('image');
        const u = (item.coverUrl && item.coverUrl.trim()) ? item.coverUrl : (isImage ? item.streamUrl : null);
        const a = u ? Api.toAbsoluteUrl(u) : null;

        const thumb = document.createElement('div');
        thumb.className = 'grid-thumb';
        if (a) {
            const img = document.createElement('img');
            img.loading = 'lazy';
            img.onerror = function() { this.style.display = 'none'; };
            img.onload = function() {
                const fb = this.parentNode.querySelector('.img-fallback');
                if (fb) fb.style.display = 'none';
            };
            img.src = a;
            thumb.appendChild(img);
        }
        const fb = document.createElement('div');
        fb.className = 'img-fallback';
        fb.innerHTML = Icon[isImage ? 'image' : 'movie'];
        thumb.appendChild(fb);
        const badge = document.createElement('span');
        badge.className = 'grid-type-badge';
        badge.innerHTML = isImage ? Icon.image : Icon.movie;
        thumb.appendChild(badge);
        div.appendChild(thumb);

        const titleBar = document.createElement('div');
        titleBar.className = 'grid-title';
        titleBar.textContent = item.title || '未知';
        div.appendChild(titleBar);

        div.addEventListener('click', () => {
            if (isImage) { const r = State.get('searchResults'); const i = r.findIndex(v => v.id === item.id); openImageViewer(r, i >= 0 ? i : 0); }
            else { openPlayer(item.id, item.title, item.streamUrl, items); }
        });
        div.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); div.click(); }
        });
        Dom.searchGrid.appendChild(div);
    });
    const t = State.get('searchTotal');
    Dom.searchLoadMore.classList.toggle('hidden', items.length >= t);
}

export async function loadMoreSearch() {
    if (State.get('isLoadingMore')) return;
    const results = State.get('searchResults');
    if (results.length >= State.get('searchTotal')) return;
    State.set('isLoadingMore', true);
    Dom.searchLoadMore.textContent = '加载中...';
    try {
        const np = (State.get('searchPage') || 0) + 1;
        const d = await Api.listVideos({ query: State.get('searchQuery'), page: np, size: 20 });
        const all = [...results, ...(d.items || [])];
        State.set('searchResults', all);
        State.set('searchPage', np);
        State.set('searchTotal', d.total || 0);
        renderSearchGrid(all);
    } catch (err) { console.warn('loadMoreSearch failed:', err); State.set('isLoadingMore', false); }
    Dom.searchLoadMore.textContent = '加载更多...';
    State.set('isLoadingMore', false);
}
