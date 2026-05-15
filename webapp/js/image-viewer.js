// =================================================================
// IMAGE VIEWER — fullscreen single-image viewer with prev/next
// =================================================================
import { State } from './state.js';
import { Dom } from './dom.js';
import { Api } from './api.js';

function updateImageDisplay() {
    const images = State.get('imageList') || [];
    const idx = State.get('imageIndex') || 0;
    if (!images.length) return;
    const item = images[idx];
    Dom.imageTitle.textContent = item.title || '';
    Dom.imageCounter.textContent = `${idx + 1} / ${images.length}`;
    Dom.imageDisplay.style.opacity = '0';
    setTimeout(() => {
        Dom.imageDisplay.src = Api.toAbsoluteUrl(item.streamUrl);
        Dom.imageDisplay.alt = item.title || '';
        Dom.imageDisplay.style.opacity = '1';
    }, 50);
    Dom.btnImagePrev.style.visibility = idx > 0 ? 'visible' : 'hidden';
    Dom.btnImageNext.style.visibility = idx < images.length - 1 ? 'visible' : 'hidden';
}

export function openImageViewer(allImages, startIndex) {
    State.set('imageList', allImages);
    State.set('imageIndex', startIndex || 0);
    Dom.pageImageViewer.classList.remove('hidden');
    Dom.bottomNav.classList.add('hidden');
    updateImageDisplay();
}

export function closeImageViewer() {
    Dom.pageImageViewer.classList.add('hidden');
    Dom.bottomNav.classList.remove('hidden');
    Dom.imageDisplay.src = '';
}

export function navigateImage(dir) {
    const idx = (State.get('imageIndex') || 0) + dir;
    const images = State.get('imageList') || [];
    if (idx >= 0 && idx < images.length) { State.set('imageIndex', idx); updateImageDisplay(); }
}
