// =================================================================
// NAVIGATION — page switching with player/viewer cleanup
// =================================================================
import { State } from './state.js';
import { Dom } from './dom.js';
import { cleanupPlayer } from './player.js';
import { closeImageViewer } from './image-viewer.js';
import { closeSlideshow, loadSlideshow } from './slideshow.js';
import { closeDrawer } from './drawer.js';

export function navigateTo(page) {
    State.set('currentPage', page);

    if (!Dom.pagePlayer.classList.contains('hidden')) {
        cleanupPlayer();
    }
    if (!Dom.pageImageViewer.classList.contains('hidden')) {
        closeImageViewer();
    }
    if (!Dom.pageSlideshowFull.classList.contains('hidden')) {
        closeSlideshow();
    }

    Dom.allPages().forEach(p => p.classList.remove('active'));
    const map = { home: Dom.pageHome, search: Dom.pageSearch, slideshow: Dom.pageSlideshow, settings: Dom.pageSettings };
    if (map[page]) map[page].classList.add('active');

    if (page === 'slideshow') loadSlideshow();

    Dom.navItems().forEach(i => i.classList.toggle('active', i.dataset.page === page));
    Dom.drawerItems().forEach(i => i.classList.toggle('active', i.dataset.page === page));
    const titles = { home: '首页', search: '搜索', slideshow: '幻灯片', settings: '设置' };
    Dom.pageTitle.textContent = titles[page] || '首页';
    Dom.bottomNav.classList.remove('hidden');
    closeDrawer();
}
