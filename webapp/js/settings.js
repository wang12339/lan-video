// =================================================================
// SETTINGS — server URL / admin token configuration
// =================================================================
import { Config } from './config.js';
import { Dom } from './dom.js';
import { Api } from './api.js';

export function loadSettings() {
    Dom.inputServerUrl.value = Config.getBaseUrl();
    Dom.inputAdminToken.value = Config.getAdminToken();
    Api.getServerInfo().then(i => { Dom.serverVersion.textContent = (i && i.version) ? i.version : '-'; }).catch(() => { Dom.serverVersion.textContent = '无法连接'; });
}
