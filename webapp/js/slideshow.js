// =================================================================
// SLIDESHOW — image grid + auto-advancing fullscreen viewer
// =================================================================
import { State } from './state.js';
import { Dom } from './dom.js';
import { Api } from './api.js';
import { Config } from './config.js';
import { Icon } from './icons.js';

let slideshowTimer = null;

function updatePlayButtonIcon(playing) {
    Dom.btnSlideshowPlay.innerHTML = playing
        ? '<svg width="24" height="24" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" fill="currentColor"/></svg>'
        : '<svg width="24" height="24" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" fill="currentColor"/></svg>';
}

function stopSlideshowAutoPlay() {
    if (slideshowTimer) { clearInterval(slideshowTimer); slideshowTimer = null; }
    Dom.slideshowProgressFill.style.width = '0%';
}

function startSlideshowAutoPlay() {
    stopSlideshowAutoPlay();
    const INTERVAL_MS = 2000;
    const TICK_MS = 100;
    let elapsed = 0;

    Dom.slideshowProgressFill.style.width = '0%';
    slideshowTimer = setInterval(() => {
        elapsed += TICK_MS;
        const pct = Math.min(100, (elapsed / INTERVAL_MS) * 100);
        Dom.slideshowProgressFill.style.width = pct + '%';
        if (elapsed >= INTERVAL_MS) {
            elapsed = 0;
            const list = State.get('slideshowList') || [];
            const idx = (State.get('slideshowIndex') || 0) + 1;
            if (idx < list.length) {
                State.set('slideshowIndex', idx);
                updateSlideshowDisplay();
            } else {
                State.set('slideshowIndex', 0);
                updateSlideshowDisplay();
            }
        }
    }, TICK_MS);
}

function updateSlideshowDisplay() {
    const list = State.get('slideshowList') || [];
    const idx = State.get('slideshowIndex') || 0;
    if (!list.length) return;
    const item = list[idx];
    Dom.slideshowTitle.textContent = item.title || '';
    Dom.slideshowCounter.textContent = `${idx + 1} / ${list.length}`;
    Dom.btnSlideshowPrev.style.visibility = list.length > 1 ? 'visible' : 'hidden';
    Dom.btnSlideshowNext.style.visibility = list.length > 1 ? 'visible' : 'hidden';
    const playing = State.get('slideshowPlaying');
    updatePlayButtonIcon(playing);
    if (Dom.slideshowDisplay) {
        const url = Api.toAbsoluteUrl(item.streamUrl || item.coverUrl || '');
        if (url) {
            Dom.slideshowDisplay.src = url;
            Dom.slideshowDisplay.alt = item.title || '';
            Dom.slideshowDisplay.onerror = function() {
                this.style.display = 'none';
                const fb = this.parentNode.querySelector('.slideshow-fallback');
                if (fb) fb.style.display = 'flex';
            };
            Dom.slideshowDisplay.onload = function() {
                this.style.display = '';
                const fb = this.parentNode.querySelector('.slideshow-fallback');
                if (fb) fb.style.display = 'none';
            };
        }
    }
}

export function loadSlideshow() {
    Dom.slideshowGrid.innerHTML = '';
    Dom.slideshowGrid.classList.add('hidden');
    Dom.slideshowEmpty.classList.remove('hidden');
    Dom.slideshowEmpty.querySelector('p').textContent = '正在加载...';

    Api.listVideos({ type: 'local_image', page: 0, size: Config.PAGE_SIZE_MAX }).then(data => {
        const items = data.items || [];

        if (!items.length) {
            Dom.slideshowGrid.classList.add('hidden');
            Dom.slideshowEmpty.classList.remove('hidden');
            Dom.slideshowEmpty.querySelector('p').textContent = '暂无图片';
            return;
        }
        Dom.slideshowEmpty.classList.add('hidden');
        Dom.slideshowGrid.classList.remove('hidden');

        items.forEach((item, idx) => {
            const div = document.createElement('div');
            div.className = 'grid-item';
            div.setAttribute('role', 'button');
            div.setAttribute('tabindex', '0');
            div.setAttribute('aria-label', item.title || '未知');
            const u = (item.coverUrl && item.coverUrl.trim()) ? item.coverUrl : item.streamUrl;
            const a = u ? Api.toAbsoluteUrl(u) : null;
            const thumb = document.createElement('div'); thumb.className = 'grid-thumb';
            if (a) {
                const img = document.createElement('img');
                img.loading = 'lazy';
                img.onerror = function() { this.style.display = 'none'; };
                img.src = a;
                thumb.appendChild(img);
            }
            const fallback = document.createElement('div');
            fallback.className = 'img-fallback';
            fallback.innerHTML = Icon.image;
            thumb.appendChild(fallback);
            div.appendChild(thumb);
            const titleBar = document.createElement('div');
            titleBar.className = 'grid-title';
            titleBar.textContent = item.title || '未知';
            div.appendChild(titleBar);

            let lpTimer = null, lpFired = false;
            div.addEventListener('pointerdown', (e) => {
                lpFired = false;
                div.setPointerCapture(e.pointerId);
                lpTimer = setTimeout(async () => {
                    lpFired = true;
                    try {
                        if (!confirm('确认删除"' + (item.title || '未知') + '"?')) return;
                        await Api.deleteVideo(item.id);
                        loadSlideshow();
                    } catch (err) {
                        alert('删除失败: ' + err.message);
                    }
                }, 500);
            });
            div.addEventListener('pointerup', () => { clearTimeout(lpTimer); });
            div.addEventListener('pointercancel', () => { clearTimeout(lpTimer); });
            div.addEventListener('click', () => {
                if (lpFired) return;
                openSlideshow(items, idx);
            });
            div.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); div.click(); }
            });

            Dom.slideshowGrid.appendChild(div);
        });
    }).catch(() => {
        Dom.slideshowEmpty.querySelector('p').textContent = '加载失败';
    });
}

export function openSlideshow(items, startIndex) {
    if (!Dom.pageSlideshowFull) { alert('幻灯片组件未加载'); return; }
    if (!items || !items.length) return;
    State.set('slideshowList', items);
    State.set('slideshowIndex', startIndex || 0);
    State.set('slideshowPlaying', true);
    Dom.pageSlideshowFull.classList.remove('hidden');
    Dom.bottomNav.classList.add('hidden');
    updateSlideshowDisplay();
    startSlideshowAutoPlay();
}

export function closeSlideshow() {
    stopSlideshowAutoPlay();
    if (Dom.pageSlideshowFull) Dom.pageSlideshowFull.classList.add('hidden');
    Dom.bottomNav.classList.remove('hidden');
    if (Dom.slideshowDisplay) Dom.slideshowDisplay.src = '';
}

export function navigateSlideshow(dir) {
    const list = State.get('slideshowList') || [];
    const idx = (State.get('slideshowIndex') || 0) + dir;
    if (idx >= 0 && idx < list.length) {
        State.set('slideshowIndex', idx);
        updateSlideshowDisplay();
        const playing = State.get('slideshowPlaying');
        if (playing) startSlideshowAutoPlay();
    }
}

export function toggleSlideshowPlay() {
    const playing = !State.get('slideshowPlaying');
    State.set('slideshowPlaying', playing);
    updatePlayButtonIcon(playing);
    if (playing) startSlideshowAutoPlay();
    else stopSlideshowAutoPlay();
}
