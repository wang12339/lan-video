// =================================================================
// API SERVICE — fetch/XHR wrappers for all backend calls
// =================================================================
import { Config } from './config.js';

function authHeaders(extra = {}) {
    const h = { 'X-Username': Config.getDeviceId(), ...extra };
    const p = Config.getAccessPassword();
    if (p) h['X-Password'] = p;
    const t = Config.getAdminToken();
    if (t) h['X-Admin-Token'] = t;
    return h;
}

export const Api = {
    async request(path, options = {}) {
        const base = Config.getBaseUrlTrimmed();
        const url = `${base}${path.startsWith('/') ? path : '/' + path}`;
        const headers = authHeaders(options.headers);
        delete options.headers;

        try {
            const resp = await fetch(url, { ...options, headers });
            if (resp.status === 403) {
                const body = await resp.json().catch(() => ({}));
                const msg = body.error || '';
                if (msg.includes('admin token')) throw new Error('Admin Token 无效或未配置，请在设置页面配置');
                throw new Error(msg || '权限不足，请检查访问密码');
            }
            if (!resp.ok) {
                const body = await resp.json().catch(() => ({}));
                throw new Error(body.error || `HTTP ${resp.status}`);
            }
            return resp;
        } catch (e) {
            if (e.name === 'TypeError' && e.message.includes('fetch')) {
                throw new Error(`无法连接服务器 ${base}，请检查地址或网络`);
            }
            throw e;
        }
    },

    async listVideos({ query, type, page = 0, size = 20 } = {}) {
        const params = new URLSearchParams();
        if (query) params.set('query', query);
        if (type) params.set('type', type);
        if (page != null) params.set('page', page);
        if (size != null) params.set('size', size);
        const resp = await this.request(`/videos?${params}`);
        return resp.json();
    },

    async getVideo(id) { const r = await this.request(`/videos/${id}`); return r.json(); },
    async getPlaybackHistory(videoId) { const r = await this.request(`/playback/history/${videoId}`); return r.json(); },
    async updatePlaybackHistory(videoId, positionMs, durationMs) {
        await this.request('/playback/history', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ video_id: videoId, position_ms: positionMs, duration_ms: durationMs }) });
    },
    async updateVideo(id, { title, description, category } = {}) {
        const r = await this.request(`/admin/videos/${id}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ title, description, category }) });
        return r.json();
    },
    async deleteVideo(id) { const r = await this.request(`/admin/videos/${id}`, { method: 'DELETE' }); return r.json(); },
    async deleteVideos(ids) { const r = await this.request('/admin/videos/batch', { method: 'DELETE', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(ids) }); return r.json(); },
    async checkAccess(password) {
        try { const r = await this.request('/auth/access', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ password }) }); const d = await r.json(); return d.ok === true; } catch { return false; }
    },
    async getServerInfo() { try { const r = await this.request('/server/info'); return r.json(); } catch { return null; } },

    async checkFiles(files) {
        const r = await this.request('/admin/videos/check-files', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(files) });
        const d = await r.json();
        return d.existing_indices || [];
    },

    uploadFile(file, category = 'local', onProgress) {
        return new Promise((resolve, reject) => {
            const base = Config.getBaseUrlTrimmed();
            const url = `${base}/admin/videos/upload?category=${encodeURIComponent(category)}`;
            const xhr = new XMLHttpRequest();
            xhr.upload.addEventListener('progress', (e) => { if (e.lengthComputable && onProgress) onProgress(e.loaded, e.total); });
            xhr.timeout = 600000; // 10 minute timeout for large files
            xhr.addEventListener('load', () => {
                if (xhr.status >= 200 && xhr.status < 300) { try { resolve(JSON.parse(xhr.responseText)); } catch { resolve({ id: null }); } }
                else { let m = `HTTP ${xhr.status}`; try { const d = JSON.parse(xhr.responseText); m = d.error || m; } catch {} reject(new Error(m)); }
            });
            xhr.addEventListener('error', () => reject(new Error('网络错误')));
            xhr.addEventListener('abort', () => reject(new Error('上传取消')));
            xhr.addEventListener('timeout', () => reject(new Error('上传超时，请检查网络或分次上传')));
            const fd = new FormData();
            fd.append('category', category);
            fd.append('file', file);
            xhr.open('POST', url);
            const h = authHeaders();
            Object.entries(h).forEach(([k, v]) => xhr.setRequestHeader(k, v));
            xhr.send(fd);
        });
    },

    toAbsoluteUrl(streamUrl) {
        const t = streamUrl.trim();
        if (!t || t.startsWith('http://') || t.startsWith('https://')) return t;
        const base = Config.getBaseUrlTrimmed();
        return t.startsWith('/') ? base + t : base + '/' + t;
    },

    async checkConnect() {
        try { await this.request('/videos?page=0&size=1'); return { ok: true }; }
        catch (e) {
            if (e.message.includes('密码') || e.message.includes('password') || e.message.includes('forbidden') || e.message.includes('403')) return { ok: true, needPassword: true };
            return { ok: false, error: e.message };
        }
    },
};
