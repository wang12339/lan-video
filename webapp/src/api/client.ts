// API 客户端核心
import i18n from '../i18n'

const API_TIMEOUT = 15000;
const CACHE_TTL = 30000;
const ERROR_LOG_KEY = 'atmos_error_log';
const MAX_ERRORS = 50;

// 错误日志（仅存储消息和状态码，不包含URL和堆栈等敏感信息）
function logError({ message, url, status }: {
  message: string;
  url: string;
  status: number;
  stack?: string;
}) {
  const entry = {
    message: message || 'Unknown error',
    status: status || 0,
    timestamp: new Date().toISOString(),
  };

  try {
    const logs = JSON.parse(localStorage.getItem(ERROR_LOG_KEY) || '[]');
    logs.push(entry);
    if (logs.length > MAX_ERRORS) {
      logs.splice(0, logs.length - MAX_ERRORS);
    }
    localStorage.setItem(ERROR_LOG_KEY, JSON.stringify(logs));
  } catch {
    // localStorage may be full or unavailable
  }

  console.error('[Atmos]', entry.message, url);
}

// BASE URL
export const BASE = (() => {
  if (location.protocol === 'file:') return 'http://localhost:8082';
  return '';
})();

// Token 管理 — 存储在内存中而非 localStorage
// 页面的 httpOnly cookie 提供跨刷新持久化
let _token: string | null = null;

export function getToken(): string | null {
  return _token;
}

export function saveToken(token: string) {
  _token = token;
}

export function clearToken() {
  _token = null;
}

export function mediaUrl(path: string | null): string | null {
  if (!path) return null;
  return BASE + path;
}

// 响应缓存（LRU，最大 200 条目）
const MAX_CACHE_ENTRIES = 200;
const cache = new Map<string, { data: unknown; ts: number }>();

function getCacheKey(url: string, method: string): string {
  return `${method}:${url}`;
}

function cacheTouch(key: string) {
  const entry = cache.get(key);
  if (entry) {
    cache.delete(key);
    cache.set(key, entry);
  }
}

function cacheGet(key: string): unknown | undefined {
  const entry = cache.get(key);
  if (entry && Date.now() - entry.ts < CACHE_TTL) {
    cacheTouch(key);
    return entry.data;
  }
  cache.delete(key);
  return undefined;
}

function cacheSet(key: string, data: unknown) {
  if (cache.has(key)) cache.delete(key);
  cache.set(key, { data, ts: Date.now() });
  if (cache.size > MAX_CACHE_ENTRIES) {
    const oldest = cache.keys().next();
    if (!oldest.done) cache.delete(oldest.value);
  }
}

export function cacheClear() {
  cache.clear();
}

function cacheInvalidate(urlPrefix: string) {
  if (!urlPrefix) { cache.clear(); return; }
  for (const key of cache.keys()) {
    if (key.startsWith('GET:' + urlPrefix)) cache.delete(key);
  }
}

// 全局错误回调 — 用于 Toast 通知
let onErrorCb: ((error: APIError) => void) | null = null
export function setOnError(cb: (error: APIError) => void) { onErrorCb = cb }

// 错误类型
export class APIError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.name = 'APIError';
    this.status = status;
  }
}

export class AuthError extends APIError {
  constructor(message?: string) {
    super(message || 'auth_required', 401);
    this.name = 'AuthError';
  }
}

// 认证回调（支持错误消息）
let onAuthRequiredCb: ((msg?: string) => void) | null = null;
export function setOnAuthRequired(cb: (msg?: string) => void) { onAuthRequiredCb = cb; }

// 请求核心
interface RequestOptions {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  auth?: boolean;
  skipCache?: boolean;
  timeout?: number;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const url = BASE + path;
  const { method = 'GET', body, headers = {}, auth = true, skipCache = false } = options;

  if (auth) {
    const token = getToken();
    if (token) headers['Authorization'] = 'Bearer ' + token;
  }

  const cacheKey = method === 'GET' && !skipCache ? getCacheKey(url, method) : null;
  if (cacheKey) {
    const cached = cacheGet(cacheKey);
    if (cached !== undefined) return cached as T;
  }

  const fetchOpts: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'XMLHttpRequest', ...headers },
    credentials: 'same-origin',
    signal: AbortSignal.timeout(options.timeout || API_TIMEOUT),
  };
  if (body) fetchOpts.body = JSON.stringify(body);

  let res: Response;
  try {
    res = await fetch(url, fetchOpts);
  } catch (e) {
    const err = e as Error;
    const msg = (err.name === 'TimeoutError' || err.name === 'AbortError') ? (i18n.t?.('errors.timeout') || '请求超时') : (i18n.t?.('errors.network') || '网络连接失败');
    logError({ message: msg, url, status: 0, stack: err.stack });
    throw new APIError(msg, 0);
  }

  // 204 No Content (or 205 Reset Content) — short-circuit before JSON parse.
  if (res.status === 204 || res.status === 205) {
    if (method !== 'GET') invalidateCacheForPath(path);
    return null as T;
  }

  let data: unknown;
  try {
    data = await res.json();
  } catch {
    // Response is not JSON
    if (res.status === 401 && auth) {
      clearToken();
      if (onAuthRequiredCb) onAuthRequiredCb();
      throw new AuthError();
    }
    const errorMsg = `服务器响应异常 (${res.status})`;
    logError({ message: errorMsg, url, status: res.status });
    const apiErr = new APIError(errorMsg, res.status);
    if (onErrorCb) onErrorCb(apiErr);
    throw apiErr;
  }

  if (res.status === 401 && auth) {
    clearToken();
    const dataObj = data as Record<string, unknown>;
    const msg = dataObj && typeof dataObj.error === 'string' ? dataObj.error : undefined;
    if (onAuthRequiredCb) onAuthRequiredCb(msg);
    throw new AuthError(msg);
  }

  if (!res.ok) {
    const dataObj = data as Record<string, unknown>;
    const msg = dataObj && typeof dataObj.error === 'string' ? dataObj.error : `HTTP ${res.status}`;
    logError({ message: msg, url, status: res.status });
    const apiErr = new APIError(msg, res.status);
    if (onErrorCb) onErrorCb(apiErr);
    throw apiErr;
  }

  if (cacheKey) cacheSet(cacheKey, data);
  if (method !== 'GET') invalidateCacheForPath(path);
  return data as T;
}

function invalidateCacheForPath(path: string) {
  if (path.startsWith('/admin/videos') || path.startsWith('/videos')) {
    cacheInvalidate('/videos');
    cacheInvalidate('/playback');
  } else if (path.startsWith('/playback')) {
    cacheInvalidate('/playback');
  } else {
    cacheInvalidate(path);
  }
}

// 健康检查
export async function health(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/health`, { method: 'GET', signal: AbortSignal.timeout(3000) });
    return res.ok;
  } catch { return false; }
}

// 导出 request 供其他模块使用
export { request };
