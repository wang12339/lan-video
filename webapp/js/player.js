// =================================================================
// PLAYER — video playback, fullscreen, swipe-to-switch
// =================================================================
import { State } from './state.js';
import { Dom, gridItemById } from './dom.js';
import { Api } from './api.js';
import { Icon } from './icons.js';

export let playerSaveTimer = null;
export let playerEndedHandler = null;
export let rateChangeHandler = null;
export let controlsSetup = false;
let videoSwitching = false;

export function cleanupPlayer() {
    const v = Dom.videoPlayer;
    const id = State.get('playerVideoId');
    if (id && v.duration > 0) Api.updatePlaybackHistory(id, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {});
    if (playerSaveTimer) { clearInterval(playerSaveTimer); playerSaveTimer = null; }
    if (playerEndedHandler) { Dom.videoPlayer.removeEventListener('ended', playerEndedHandler); playerEndedHandler = null; }
    if (rateChangeHandler) { Dom.videoPlayer.removeEventListener('ratechange', rateChangeHandler); rateChangeHandler = null; }
    controlsSetup = false;
    v.pause(); v.src = ''; Dom.speedIndicator.classList.add('hidden'); Dom.videoPlayer.playbackRate = 1;
    Dom.pagePlayer.classList.remove('controls-hidden');
    Dom.pagePlayer.classList.add('hidden'); Dom.bottomNav.classList.remove('hidden');
    if (id) {
        const item = gridItemById(id);
        if (item) item.scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

export function closePlayer() {
    cleanupPlayer();
}

export function openPlayer(videoId, title, streamUrl, videoList) {
    Dom.playerTitle.textContent = title || '播放';
    Dom.pagePlayer.classList.remove('hidden');
    Dom.bottomNav.classList.add('hidden');
    const video = Dom.videoPlayer;
    video.src = Api.toAbsoluteUrl(streamUrl);
    State.set('playerVideoId', videoId);

    if (videoList && videoList.length) {
        State.set('playerList', videoList);
        const idx = videoList.findIndex(v => v.id === videoId);
        State.set('playerIndex', idx >= 0 ? idx : -1);
    }

    if (playerSaveTimer) clearInterval(playerSaveTimer);
    playerSaveTimer = setInterval(() => {
        if (Dom.pagePlayer.classList.contains('hidden')) { clearInterval(playerSaveTimer); return; }
        const v = Dom.videoPlayer;
        const pid = State.get('playerVideoId');
        if (pid && v.duration > 0) Api.updatePlaybackHistory(pid, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {});
    }, 5000);

    if (rateChangeHandler) {
        Dom.videoPlayer.removeEventListener('ratechange', rateChangeHandler);
    }
    rateChangeHandler = () => {
        Dom.speedIndicator.classList.toggle('hidden', Dom.videoPlayer.playbackRate <= 1);
    };
    Dom.videoPlayer.addEventListener('ratechange', rateChangeHandler);

    Api.getPlaybackHistory(videoId).then(d => {
        if (d && d.position_ms > 1000) video.currentTime = d.position_ms / 1000;
        video.play();
    }).catch(() => video.play());

    if (playerEndedHandler) {
        Dom.videoPlayer.removeEventListener('ended', playerEndedHandler);
    }
    playerEndedHandler = function() { navigatePlayer(1); };
    Dom.videoPlayer.addEventListener('ended', playerEndedHandler);

    if (!controlsSetup) {
        controlsSetup = true;
        let ctrlTimer = null;

        function resetControlsTimer() {
            if (ctrlTimer) clearTimeout(ctrlTimer);
            ctrlTimer = setTimeout(function() {
                if (!Dom.videoPlayer.paused && (document.fullscreenElement === Dom.pagePlayer || document.webkitFullscreenElement === Dom.pagePlayer)) {
                    Dom.pagePlayer.classList.add('controls-hidden');
                }
            }, 3000);
        }

        Dom.videoPlayer.addEventListener('play', resetControlsTimer);
        Dom.videoPlayer.addEventListener('pause', function() {
            Dom.pagePlayer.classList.remove('controls-hidden');
            if (ctrlTimer) clearTimeout(ctrlTimer);
        });
        Dom.pagePlayer.addEventListener('mousemove', function() {
            Dom.pagePlayer.classList.remove('controls-hidden');
            resetControlsTimer();
        });
        Dom.pagePlayer.addEventListener('click', function() {
            if (document.fullscreenElement === Dom.pagePlayer || document.webkitFullscreenElement === Dom.pagePlayer) {
                if (Dom.pagePlayer.classList.contains('controls-hidden')) {
                    Dom.pagePlayer.classList.remove('controls-hidden');
                    resetControlsTimer();
                }
            }
        });
    }
}

function switchToVideo(index) {
    const list = State.get('playerList') || [];
    if (index < 0 || index >= list.length) return;
    const item = list[index];
    if (!item) return;
    if (item.sourceType && item.sourceType.includes('image')) return;

    const v = Dom.videoPlayer;
    const curId = State.get('playerVideoId');
    if (curId && v.duration > 0) {
        Api.updatePlaybackHistory(curId, Math.floor(v.currentTime * 1000), Math.floor(v.duration * 1000)).catch(() => {});
    }

    State.set('playerVideoId', item.id);
    State.set('playerIndex', index);
    Dom.playerTitle.textContent = item.title || '播放';

    v.src = Api.toAbsoluteUrl(item.streamUrl);
    v.load();
    v.play().catch(() => {});

    Api.getPlaybackHistory(item.id).then(d => {
        if (d && d.position_ms > 1000) v.currentTime = d.position_ms / 1000;
    }).catch(() => {});
}

export function navigatePlayer(dir) {
    if (videoSwitching) return;
    videoSwitching = true;
    setTimeout(() => { videoSwitching = false; }, 500);

    const list = State.get('playerList') || [];
    const idx = State.get('playerIndex') || 0;
    let newIdx = idx + dir;

    while (newIdx >= 0 && newIdx < list.length) {
        const item = list[newIdx];
        if (!item) break;
        if (!(item.sourceType && item.sourceType.includes('image'))) break;
        newIdx += dir;
    }

    if (newIdx < 0 || newIdx >= list.length) return;
    switchToVideo(newIdx);
}

export function toggleFullscreen() {
    if (document.fullscreenElement || document.webkitFullscreenElement) {
        if (document.exitFullscreen) document.exitFullscreen();
        else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
    } else {
        const el = Dom.pagePlayer;
        if (el.requestFullscreen) el.requestFullscreen();
        else if (el.webkitRequestFullscreen) el.webkitRequestFullscreen();
    }
}

export function updateFullscreenIcon() {
    const isFs = document.fullscreenElement || document.webkitFullscreenElement;
    const btn = Dom.btnPlayerFullscreen;
    if (btn) btn.dataset.icon = isFs ? 'fullscreen_exit' : 'fullscreen';
    const name = btn ? btn.dataset.icon : 'fullscreen';
    const svg = btn ? btn.querySelector('svg') : null;
    if (svg && Icon[name]) svg.outerHTML = Icon[name];
}

document.addEventListener('fullscreenchange', updateFullscreenIcon);
document.addEventListener('webkitfullscreenchange', updateFullscreenIcon);
