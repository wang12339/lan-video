// =================================================================
// CONFIG — localStorage-backed configuration
// =================================================================
const STORAGE_KEYS = {
    SERVER_URL: 'lan_video_server_url',
    ADMIN_TOKEN: 'lan_video_admin_token',
    ACCESS_PASSWORD: 'lan_video_access_password',
    DEVICE_ID: 'lan_video_device_id',
};

export const Config = {
    DEFAULT_SERVER: 'http://192.168.66.186:8082/',
    PAGE_SIZE_MAX: 999999,

    getDeviceId() {
        let id = localStorage.getItem(STORAGE_KEYS.DEVICE_ID);
        if (!id) {
            id = 'web_' + Math.random().toString(36).substring(2, 15) + Date.now().toString(36);
            localStorage.setItem(STORAGE_KEYS.DEVICE_ID, id);
        }
        return id;
    },

    getBaseUrl() {
        return localStorage.getItem(STORAGE_KEYS.SERVER_URL) || this.DEFAULT_SERVER;
    },
    getBaseUrlTrimmed() {
        return this.getBaseUrl().replace(/\/+$/, '');
    },
    saveBaseUrl(url) {
        const norm = url.endsWith('/') ? url : url + '/';
        localStorage.setItem(STORAGE_KEYS.SERVER_URL, norm);
    },

    getAdminToken() { return localStorage.getItem(STORAGE_KEYS.ADMIN_TOKEN) || ''; },
    hasAdminToken() { return !!localStorage.getItem(STORAGE_KEYS.ADMIN_TOKEN); },
    saveAdminToken(token) { localStorage.setItem(STORAGE_KEYS.ADMIN_TOKEN, token); },
    getAccessPassword() { return localStorage.getItem(STORAGE_KEYS.ACCESS_PASSWORD) || ''; },
    saveAccessPassword(password) { localStorage.setItem(STORAGE_KEYS.ACCESS_PASSWORD, password); },
    hasAccessPassword() { return !!localStorage.getItem(STORAGE_KEYS.ACCESS_PASSWORD); },
};
