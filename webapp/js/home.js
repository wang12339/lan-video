// =================================================================
// HOME — grid feed, tab switching, batch delete mode
// =================================================================
import { Config } from './config.js';
import { State } from './state.js';
import { Dom } from './dom.js';
import { Api } from './api.js';
import { setConnectionStatus } from './connection.js';
import { navigateTo } from './navigation.js';

export function switchTab(index) {
    State.set('homeTab', index);
    Dom.tabs().forEach((t, i) => {
        t.classList.toggle('active', i === index);
        t.setAttribute('aria-selected', i === index ? 'true' : 'false');
    });
    const indicator = Dom.tabIndicator;
    if (indicator) {
        indicator.style.width = `${100 / Dom.tabs().length}%`;
        indicator.style.transform = `translateX(${index * 100}%)`;
    }
    loadHomeFeed();
}

export async function loadHomeFeed() {
    // Don't attempt load if we know we're disconnected
    if (State.get('connectionStatus') === 'disconnected') {
        Dom.skeletonGrid.classList.add('hidden');
        Dom.emptyState.classList.remove('hidden');
        Dom.emptyText.textContent = '无法连接服务器，请检查设置';
        return;
    }

    const tab = State.get('homeTab');
    const typeFilter = tab === 1 ? 'local_image' : '!local_image';
    const pageSize = Config.PAGE_SIZE_MAX;

    Dom.skeletonGrid.classList.remove('hidden');
    Dom.grid.classList.add('hidden');
    Dom.emptyState.classList.add('hidden');
    Dom.loadingSpinner.classList.add('hidden');

    try {
        const data = await Api.listVideos({ type: typeFilter, page: 0, size: pageSize });
        Dom.skeletonGrid.classList.add('hidden');
        const items = data.items || [];
        if (!items.length) {
            Dom.emptyState.classList.remove('hidden');
            Dom.emptyText.textContent = tab === 1 ? '暂无图片' : '暂无视频';
            return;
        }
        State.set('allVideos', items);
        renderGrid(Dom.grid, items);
        Dom.grid.classList.remove('hidden');
    } catch (err) {
        Dom.skeletonGrid.classList.add('hidden');
        Dom.loadingSpinner.classList.add('hidden');
        Dom.emptyState.classList.remove('hidden');
        Dom.emptyText.textContent = `加载失败: ${err.message}`;
        setConnectionStatus('disconnected');
    }
}

export function renderGrid(container, items) {
    container.innerHTML = '';
    const isSelectMode = State.get('isSelectMode');
    const selectedIds = State.get('selectedIds');

    items.forEach(item => {
        const div = document.createElement('div');
        div.className = 'grid-item' + (selectedIds.has(item.id) ? ' selected' : '');
        div.dataset.id = item.id;
        div.setAttribute('role', 'button');
        div.setAttribute('tabindex', '0');
        div.setAttribute('aria-label', item.title || '未知');

        const isImage = item.sourceType && item.sourceType.includes('image');
        const imgUrl = (item.thumbUrl && item.thumbUrl.trim()) ? item.thumbUrl
            : (item.coverUrl && item.coverUrl.trim()) ? item.coverUrl
            : (isImage ? item.streamUrl : null);
        const absUrl = imgUrl ? Api.toAbsoluteUrl(imgUrl) : null;

        const thumb = document.createElement('div');
        thumb.className = 'grid-thumb';

        if (absUrl) {
            const img = document.createElement('img');
            img.loading = 'lazy';
            img.onerror = function() { this.style.display = 'none'; };
            img.onload = function() {
                const fb = this.parentNode.querySelector('.img-fallback');
                if (fb) fb.style.display = 'none';
            };
            img.src = absUrl;
            thumb.appendChild(img);
        }

        const fallback = document.createElement('div');
        fallback.className = 'img-fallback';
        fallback.setAttribute('data-icon', isImage ? 'image' : 'video_file');
        thumb.appendChild(fallback);

        const info = document.createElement('div');
        info.className = 'grid-info';

        const title = document.createElement('div');
        title.className = 'grid-title';
        title.textContent = item.title || '未知';
        info.appendChild(title);

        if (item.duration > 0) {
            const dur = document.createElement('div');
            dur.className = 'grid-duration';
            const m = Math.floor(item.duration / 60);
            const s = item.duration % 60;
            dur.textContent = `${m}:${s.toString().padStart(2, '0')}`;
            thumb.appendChild(dur);
        }

        // Playback progress bar
        if (item.watchPosition > 0 && item.duration > 0) {
            const pct = Math.min(item.watchPosition / item.duration, 1);
            const bar = document.createElement('div');
            bar.className = 'grid-progress';
            const fill = document.createElement('div');
            fill.style.width = `${Math.round(pct * 100)}%`;
            bar.appendChild(fill);
            thumb.appendChild(bar);
        }

        if (isSelectMode) {
            const sel = document.createElement('div');
            sel.className = 'grid-select';
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.checked = selectedIds.has(item.id);
            cb.addEventListener('change', (e) => {
                if (e.target.checked) selectedIds.add(item.id);
                else selectedIds.delete(item.id);
                updateSelectionUI();
                renderGrid(Dom.grid, State.get('allVideos'));
            });
            sel.appendChild(cb);
            thumb.appendChild(sel);
        }

        div.appendChild(thumb);
        div.appendChild(info);
        container.appendChild(div);
    });
}

export function enterSelectMode(targetId) {
    State.set('isSelectMode', true);
    const all = State.get('allVideos');
    State.set('selectedIds', new Set(targetId != null ? [targetId] : []));
    Dom.selectionBar.classList.remove('hidden');
    updateSelectionUI();
    renderGrid(Dom.grid, all);
}

export function exitSelectMode() {
    State.set('isSelectMode', false);
    State.set('selectedIds', new Set());
    Dom.selectionBar.classList.add('hidden');
    State.get('currentPage') === 'home' && loadHomeFeed();
}

export function updateSelectionUI() {
    const ids = State.get('selectedIds');
    const all = State.get('allVideos');
    const c = ids.size;
    Dom.selectionCount.textContent = c > 0 ? `> SELECTED: ${c}` : '';
    Dom.btnSelectAll.textContent = c === all.length ? '取消全选' : '全选';
}
