// =================================================================
// CONNECTION STATUS — server connectivity indicator
// =================================================================
import { State } from './state.js';
import { Dom } from './dom.js';

export function setConnectionStatus(state) {
    State.set('connectionStatus', state);
    const { connectionStatus: status, statusDot: dot, statusText: text } = Dom;
    status.classList.remove('hidden', 'connected');
    if (state === 'connected') {
        status.classList.add('connected');
        dot.className = 'status-dot green';
        text.textContent = '已连接';
    } else if (state === 'scanning') {
        dot.className = 'status-dot yellow';
        text.textContent = '正在扫描服务器...';
    } else {
        dot.className = 'status-dot red';
        text.textContent = '未连接，点击重试';
    }
}
