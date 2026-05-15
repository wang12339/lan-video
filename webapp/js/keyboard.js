// =================================================================
// KEYBOARD — global keyboard shortcuts
// =================================================================
import { Dom } from './dom.js';
import { closePlayer, navigatePlayer } from './player.js';
import { closeImageViewer, navigateImage } from './image-viewer.js';
import { closeSlideshow, navigateSlideshow, toggleSlideshowPlay } from './slideshow.js';
import { closeDrawer } from './drawer.js';

let speedActive = false;

export function initKeyboard() {
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            if (!Dom.pagePlayer.classList.contains('hidden')) closePlayer();
            else if (!Dom.pageImageViewer.classList.contains('hidden')) closeImageViewer();
            else if (!Dom.drawer.classList.contains('hidden')) closeDrawer();
        }
        if (e.key === ' ' && !Dom.pagePlayer.classList.contains('hidden')) {
            e.preventDefault();
            Dom.videoPlayer.paused ? Dom.videoPlayer.play() : Dom.videoPlayer.pause();
        }
        if (!Dom.pageImageViewer.classList.contains('hidden')) {
            if (e.key === 'ArrowLeft') navigateImage(-1);
            if (e.key === 'ArrowRight') navigateImage(1);
        }
        if (!Dom.pageSlideshowFull.classList.contains('hidden')) {
            if (e.key === 'ArrowLeft') navigateSlideshow(-1);
            if (e.key === 'ArrowRight') navigateSlideshow(1);
            if (e.key === ' ' || e.key === 'Space') { e.preventDefault(); toggleSlideshowPlay(); }
        }
        if (e.key === 'Shift' && !Dom.pagePlayer.classList.contains('hidden')) {
            Dom.videoPlayer.playbackRate = 2;
            speedActive = true;
        }
        if (!Dom.pagePlayer.classList.contains('hidden')) {
            if (e.key === 'ArrowUp') { e.preventDefault(); navigatePlayer(-1); }
            if (e.key === 'ArrowDown') { e.preventDefault(); navigatePlayer(1); }
        }
    });

    document.addEventListener('keyup', (e) => {
        if (e.key === 'Shift' && speedActive) {
            Dom.videoPlayer.playbackRate = 1;
            speedActive = false;
        }
    });
}
