// =================================================================
// ACCESS GATE — password dialog
// =================================================================
import { Config } from './config.js';
import { Dom } from './dom.js';
import { Api } from './api.js';

export function checkAccessGate() {
    Api.listVideos({ page: 0, size: 1 }).catch(err => {
        if ((err.message.includes('密码') || err.message.includes('password') || err.message.includes('403') || err.message.includes('forbidden')) && !Config.hasAccessPassword()) {
            Dom.accessGateOverlay.classList.remove('hidden');
        }
    });
}
