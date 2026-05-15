// =================================================================
// MAIN — entry point, event wiring, initialization
// =================================================================
import { initIcons } from './icons.js';
import { Config } from './config.js';
import { State } from './state.js';
import { Dom } from './dom.js';
import { Api } from './api.js';
import { setConnectionStatus } from './connection.js';
import { navigateTo } from './navigation.js';
import { openDrawer, closeDrawer } from './drawer.js';
import { closePlayer, toggleFullscreen, navigatePlayer } from './player.js';
import { closeImageViewer, navigateImage } from './image-viewer.js';
import { closeSlideshow, navigateSlideshow, toggleSlideshowPlay } from './slideshow.js';
import { switchTab, loadHomeFeed, renderGrid, enterSelectMode, exitSelectMode, updateSelectionUI } from './home.js';
import { performSearch, loadMoreSearch } from './search.js';
import { loadSettings } from './settings.js';
import { startUpload } from './upload.js';
import { checkAccessGate } from './access-gate.js';
import { initKeyboard } from './keyboard.js';

const CONTENT_SCROLL = () => document.querySelector('.content');

function setupInfiniteScroll() {
    CONTENT_SCROLL().addEventListener('scroll', () => {
        if (State.get('currentPage') !== 'search') return;
        const { scrollTop, scrollHeight, clientHeight } = CONTENT_SCROLL();
        if (scrollTop + clientHeight >= scrollHeight - 200) loadMoreSearch();
    });
}

function init() {
    initIcons();
    initKeyboard();

    // Register Service Worker for PWA support
    if ('serviceWorker' in navigator) {
        navigator.serviceWorker.register('sw.js').catch(function(err) {
            console.warn('SW registration failed:', err);
        });
    }

    // ── Drawer ──
    Dom.btnMenu.addEventListener('click', openDrawer);
    Dom.drawerOverlay.addEventListener('click', closeDrawer);
    Dom.drawerItems().forEach(i => i.addEventListener('click', () => { if (i.dataset.page) navigateTo(i.dataset.page); }));
    Dom.drawerUpload.addEventListener('click', () => { closeDrawer(); Dom.filePicker.click(); });
    Dom.drawerFolderUpload.addEventListener('click', () => { closeDrawer(); Dom.folderPicker.click(); });
    Dom.drawerBatchDelete.addEventListener('click', () => {
        closeDrawer(); navigateTo('home');
        setTimeout(() => {
            const tab = State.get('homeTab');
            Api.listVideos({ type: tab === 1 ? 'local_image' : '!local_image', page: 0, size: Config.PAGE_SIZE_MAX })
                .then(d => { State.set('allVideos', d.items || []); enterSelectMode(null); Dom.grid.classList.remove('hidden'); Dom.emptyState.classList.add('hidden'); })
                .catch(() => alert('加载失败'));
        }, 300);
    });

    // ── Bottom Nav ──
    Dom.navItems().forEach(i => i.addEventListener('click', () => navigateTo(i.dataset.page)));

    // ── Tabs ──
    Dom.tabs().forEach((t, i) => t.addEventListener('click', () => switchTab(i)));

    // ── Selection ──
    Dom.btnSelectAll.addEventListener('click', () => {
        const all = State.get('allVideos');
        const s = State.get('selectedIds');
        s.size === all.length ? s.clear() : all.forEach(v => s.add(v.id));
        updateSelectionUI();
        renderGrid(Dom.grid, all);
    });
    Dom.btnCancelSelect.addEventListener('click', exitSelectMode);
    Dom.btnDeleteSelected.addEventListener('click', async () => {
        const ids = [...State.get('selectedIds')];
        if (!ids.length) return;
        try { await Api.deleteVideos(ids); exitSelectMode(); loadHomeFeed(); } catch (err) { alert(`删除失败: ${err.message}`); }
    });

    // ── Search ──
    let st = null;
    Dom.searchInput.addEventListener('input', () => { clearTimeout(st); st = setTimeout(performSearch, 300); });
    Dom.searchLoadMore.addEventListener('click', loadMoreSearch);

    // ── Player ──
    Dom.btnPlayerBack.addEventListener('click', closePlayer);
    Dom.btnPlayerFullscreen.addEventListener('click', toggleFullscreen);

    // Touch swipe for video navigation
    let touchStartY = 0, touchStartX = 0;
    const playerContainer = Dom.pagePlayer.querySelector('.player-container');
    if (playerContainer) {
        playerContainer.addEventListener('touchstart', (e) => {
            touchStartY = e.touches[0].clientY;
            touchStartX = e.touches[0].clientX;
        }, { passive: true });
        playerContainer.addEventListener('touchmove', (e) => {
            const dy = touchStartY - e.touches[0].clientY;
            const dx = touchStartX - e.touches[0].clientX;
            if (Math.abs(dy) > Math.abs(dx) && Math.abs(dy) > 10) {
                e.preventDefault();
            }
        }, { passive: false });
        playerContainer.addEventListener('touchend', (e) => {
            const dy = touchStartY - e.changedTouches[0].clientY;
            const dx = touchStartX - e.changedTouches[0].clientX;
            if (Math.abs(dy) > Math.abs(dx) && Math.abs(dy) > 30) {
                navigatePlayer(dy > 0 ? 1 : -1);
            }
        }, { passive: true });
    }

    // ── Image viewer ──
    Dom.btnImageBack.addEventListener('click', closeImageViewer);
    Dom.btnImagePrev.addEventListener('click', () => navigateImage(-1));
    Dom.btnImageNext.addEventListener('click', () => navigateImage(1));

    // ── Slideshow ──
    Dom.btnSlideshowBack.addEventListener('click', closeSlideshow);
    Dom.btnSlideshowPrev.addEventListener('click', () => navigateSlideshow(-1));
    Dom.btnSlideshowNext.addEventListener('click', () => navigateSlideshow(1));
    Dom.btnSlideshowPlay.addEventListener('click', toggleSlideshowPlay);

    // ── Settings ──
    Dom.btnSaveServer.addEventListener('click', () => {
        const url = Dom.inputServerUrl.value.trim();
        if (!url) return alert('请输入服务器地址');
        if (!url.startsWith('http://') && !url.startsWith('https://')) return alert('地址必须以 http:// 或 https:// 开头');
        try { new URL(url); } catch { return alert('服务器地址格式无效，请输入完整地址（如 http://192.168.1.100:8082）'); }
        Config.saveBaseUrl(url);
        setConnectionStatus('scanning');
        setTimeout(() => {
            Api.checkConnect().then(r => {
                if (r.ok) { setConnectionStatus('connected'); alert('已保存并成功连接'); loadHomeFeed(); }
                else { setConnectionStatus('disconnected'); alert('已保存，但无法连接服务器'); }
            });
        }, 500);
    });
    Dom.btnSaveToken.addEventListener('click', () => { Config.saveAdminToken(Dom.inputAdminToken.value.trim()); alert('Admin Token 已保存'); });

    // ── Access gate ──
    Dom.btnAccessSubmit.addEventListener('click', async () => {
        const pwd = Dom.accessPasswordInput.value.trim();
        if (!pwd) return;
        const ok = await Api.checkAccess(pwd);
        if (ok) { Config.saveAccessPassword(pwd); Dom.accessGateOverlay.classList.add('hidden'); Dom.accessPasswordError.classList.add('hidden'); loadHomeFeed(); }
        else { Dom.accessPasswordError.classList.remove('hidden'); }
    });
    Dom.accessPasswordInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') Dom.btnAccessSubmit.click(); });

    // ── File picker ──
    Dom.filePicker.addEventListener('change', (e) => { if (e.target.files && e.target.files.length) startUpload(e.target.files); e.target.value = ''; });
    Dom.folderPicker.addEventListener('change', (e) => { if (e.target.files && e.target.files.length) startUpload(e.target.files); e.target.value = ''; });

    // ── Connection status ──
    Dom.connectionStatus.addEventListener('click', () => {
        setConnectionStatus('scanning');
        Api.checkConnect().then(r => {
            if (r.ok && r.needPassword) { setConnectionStatus('connected'); checkAccessGate(); }
            else if (r.ok) { setConnectionStatus('connected'); loadHomeFeed(); }
            else setConnectionStatus('disconnected');
        });
    });

    // ── Infinite scroll ──
    setupInfiniteScroll();

    // ── Init state ──
    loadSettings();
    navigateTo('home');

    setConnectionStatus('scanning');
    setTimeout(() => {
        Api.checkConnect().then(r => {
            if (r.ok && r.needPassword) { setConnectionStatus('connected'); checkAccessGate(); }
            else if (r.ok) { setConnectionStatus('connected'); loadHomeFeed(); checkAccessGate(); }
            else { Dom.skeletonGrid.classList.add('hidden'); Dom.emptyState.classList.remove('hidden'); Dom.emptyText.textContent = '无法连接服务器，请检查设置'; setConnectionStatus('disconnected'); }
        });
    }, 500);

    // ── Playback save on hide ──
    document.addEventListener('visibilitychange', () => {
        if (document.hidden && !Dom.pagePlayer.classList.contains('hidden')) {
            const id = State.get('playerVideoId');
            const v = Dom.videoPlayer;
            if (id && v.duration > 0) Api.updatePlaybackHistory(id, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {});
        }
    });

    // ── Playback save on page close ──
    window.addEventListener('beforeunload', () => {
        if (!Dom.pagePlayer.classList.contains('hidden')) {
            const id = State.get('playerVideoId');
            const v = Dom.videoPlayer;
            if (id && v.duration > 0) {
                try {
                    const headers = { 'Content-Type': 'application/json', 'X-Username': Config.getDeviceId() };
                    const pwd = Config.getAccessPassword();
                    if (pwd) headers['X-Password'] = pwd;
                    const tok = Config.getAdminToken();
                    if (tok) headers['X-Admin-Token'] = tok;
                    fetch(Config.getBaseUrlTrimmed() + '/playback/history', {
                        method: 'POST', headers, keepalive: true,
                        body: JSON.stringify({ video_id: id, position_ms: Math.floor(v.currentTime * 1000), duration_ms: Math.floor(v.duration * 1000) })
                    }).catch(() => {});
                } catch (e) {}
            }
        }
    });
}

document.addEventListener('DOMContentLoaded', init);
