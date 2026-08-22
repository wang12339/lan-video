// API 客户端核心
//
// ─────────────────────────────────────────────────────────────
// 缓存失效约定（唯一入口：invalidateCacheForPath）
// ─────────────────────────────────────────────────────────────
// 项目存在两层响应缓存：
//   1. LRU 响应缓存（下方 cache Map，GET 30s TTL，键含登录态标识）
//   2. react-query 查询缓存（../lib/queryClient.ts，staleTime 30s）
// 规则：任何 POST/PUT/DELETE 成功返回后，统一由 invalidateCacheForPath()
// 让"可能受影响的读数据"失效 —— 同一规则表同时作用于两层：
//   - LRU：按 INVALIDATION_RULES.lruPrefixes 前缀清除（跨登录态一并清）
//   - react-query：按 INVALIDATION_RULES.queryKeyPrefixes 调
//     queryClient.invalidateQueries（queryKey 前缀匹配）
// 页面/组件层不要再单独清 LRU；页面自带的 queryClient 操作（乐观更新、
// 局部 invalidate）保留，与这里重复触发是无害的。
// 特殊约定：
//   - POST /auth/logout：会话边界，两层缓存整体清空（cache.clear +
//     queryClient.clear），避免登出后读到他人残留数据
//   - POST /playback/session/*（心跳/启停）：不改变任何可缓存 GET，跳过失效
//   - POST /playback/history（进度上报，播放中每 10s 一次）：只精确失效
//     对应视频的 /playback/history/{id} 与 /videos/{id}，不扫全表，
//     避免看视频期间 /playback 缓存永远打不中
//   - 高频写（分片上传）用 noInvalidate 主动跳过本流程
//   - 页面自建 sessionCache（auth.ts）与 rq 无关，不受本约定约束
// ─────────────────────────────────────────────────────────────
import i18n from '../i18n'
import { queryClient } from '../lib/queryClient'

const API_TIMEOUT = 15000;
const CACHE_TTL = 30000;
const ERROR_LOG_KEY = 'atmos_error_log';
const MAX_ERRORS = 50;
const MAX_RETRIES = 3;
const RETRY_DELAY = 1000; // 1秒基础延迟

// 错误日志（仅存储消息和状态码，不包含URL和堆栈等敏感信息）
function logError({ message, url, status }: {
  message: string;
  url: string;
  status: number;
}) {
  const entry = {
    message: message || i18n.t?.('errors.unknownError') || '发生未知错误',
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

// 后端错误消息本地化：中文业务消息直接透传；
// 英文/缺失的通用消息按状态码回退到本地化文案，避免英文漏到界面
function resolveErrorMessage(status: number, backendMsg?: string): string {
  if (backendMsg && /[\u4e00-\u9fff]/.test(backendMsg)) return backendMsg;
  switch (status) {
    case 401: return i18n.t?.('errors.unauthorized') || '请登录后继续';
    case 403: return i18n.t?.('errors.forbidden') || '无权操作';
    case 404: return i18n.t?.('errors.notFound') || '资源不存在';
    case 429: return i18n.t?.('errors.rateLimit') || '请求过于频繁，请稍后再试';
    default:
      if (status >= 500) return i18n.t?.('errors.serverError') || '服务器内部错误';
      return backendMsg || i18n.t?.('errors.unknownError') || '发生未知错误';
  }
}

// 401 时传递给认证回调的消息：保留后端中文业务语义（强制下线/登录过期），
// 非中文的通用消息（如 authentication failed）本地化
function localizeAuthMessage(msg?: string): string | undefined {
  if (msg && !/[\u4e00-\u9fff]/.test(msg)) return resolveErrorMessage(401);
  return msg;
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
// 缓存键包含登录态标识，避免登出/切换账号后读到他人私有数据
const MAX_CACHE_ENTRIES = 200;
const cache = new Map<string, { data: unknown; ts: number }>();

function getCacheKey(url: string, method: string): string {
  return `${method}:${getToken() ?? 'anon'}:${url}`;
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

function cacheInvalidatePrefix(urlPrefix: string) {
  if (!urlPrefix) { cache.clear(); return; }
  // key 形如 "GET:<auth>:<path>…"，路径部分包含前缀即失效（跨登录态一并清理）
  for (const key of cache.keys()) {
    if (key.startsWith('GET:') && key.includes(urlPrefix)) cache.delete(key);
  }
}

// 写路径前缀 → 受影响数据域（LRU GET 前缀 + react-query 键前缀）
interface InvalidationRule {
  writePrefix: string;
  lruPrefixes: readonly string[];
  queryKeyPrefixes: readonly string[];
}

const INVALIDATION_RULES: readonly InvalidationRule[] = [
  // 用户资料（头像/邮箱变更 → 个人资料接口）
  { writePrefix: '/auth/user/avatar', lruPrefixes: ['/auth/user/profile'], queryKeyPrefixes: ['user-profile'] },
  { writePrefix: '/auth/user/email', lruPrefixes: ['/auth/user/profile'], queryKeyPrefixes: ['user-profile'] },
  // 分享撤销 → 我的分享列表
  { writePrefix: '/auth/user/shares', lruPrefixes: ['/auth/user/shares'], queryKeyPrefixes: ['my-shares'] },
  // 播放列表增删改 → 我的播放列表（含 item_count 变化）
  { writePrefix: '/playlists', lruPrefixes: ['/playlists'], queryKeyPrefixes: ['my-playlists'] },
  // 评论删除 → 回复列表
  { writePrefix: '/comments', lruPrefixes: ['/comments'], queryKeyPrefixes: [] },
  // 视频域写操作（增删改/播放量/评论/标签/分享/转码/扫描）
  { writePrefix: '/admin/videos', lruPrefixes: ['/videos', '/playback', '/admin/videos'], queryKeyPrefixes: ['home-videos', 'trending-videos', 'my-works', 'my-favorites', 'admin-stats'] },
  { writePrefix: '/videos', lruPrefixes: ['/videos', '/playback'], queryKeyPrefixes: ['home-videos', 'trending-videos', 'my-works', 'my-favorites', 'admin-stats'] },
  // 标签管理 → 标签列表
  { writePrefix: '/admin/tags', lruPrefixes: ['/tags'], queryKeyPrefixes: ['admin-tags'] },
  // 用户管理 → 用户列表
  { writePrefix: '/admin/users', lruPrefixes: ['/admin/users'], queryKeyPrefixes: [] },
  // 日志清空
  { writePrefix: '/admin/logs', lruPrefixes: ['/admin/logs'], queryKeyPrefixes: [] },
  // 注册开关
  { writePrefix: '/admin/config/registration', lruPrefixes: ['/admin/config/registration'], queryKeyPrefixes: ['admin-registration-enabled'] },
];

function invalidateReactQuery(keyPrefixes: readonly string[]) {
  for (const prefix of keyPrefixes) {
    void queryClient.invalidateQueries({ queryKey: [prefix] });
  }
}

function extractVideoId(body: unknown): number | undefined {
  if (body && typeof body === 'object') {
    const id = (body as Record<string, unknown>).video_id;
    if (typeof id === 'number') return id;
  }
  return undefined;
}

function invalidateCacheForPath(path: string, body?: unknown) {
  // 会话边界：登出后两层缓存整体作废，后续读取从服务器取最新数据
  if (path === '/auth/logout') {
    cache.clear();
    queryClient.clear();
    return;
  }

  // 播放会话心跳/启停：不改变任何可缓存的 GET 响应，跳过失效
  if (path.startsWith('/playback/session/')) return;

  // 播放进度上报（播放中每 10s 一次）：只精确失效对应视频的进度/详情，
  // 不扫全表，避免看视频期间 /playback 缓存永远打不中
  if (path === '/playback/history') {
    const videoId = extractVideoId(body);
    cacheInvalidatePrefix('/playback/history');
    cacheInvalidatePrefix(typeof videoId === 'number' ? `/videos/${videoId}` : '/videos');
    invalidateReactQuery(['my-history']);
    return;
  }

  // 其余写操作：按前缀表同时失效 LRU 与 react-query 两层缓存
  for (const rule of INVALIDATION_RULES) {
    if (!path.startsWith(rule.writePrefix)) continue;
    for (const prefix of rule.lruPrefixes) cacheInvalidatePrefix(prefix);
    invalidateReactQuery(rule.queryKeyPrefixes);
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
    super(message || resolveErrorMessage(401), 401);
    this.name = 'AuthError';
  }
}

// 认证回调（支持错误消息）
let onAuthRequiredCb: ((msg?: string) => void) | null = null;
export function setOnAuthRequired(cb: (msg?: string) => void) { onAuthRequiredCb = cb; }

// 重试延迟计算（指数退避）
function getRetryDelay(attempt: number): number {
  return RETRY_DELAY * Math.pow(2, attempt) + Math.random() * 1000;
}

// 判断是否应该重试
function shouldRetry(status: number, attempt: number): boolean {
  if (attempt >= MAX_RETRIES) return false;
  // 只重试网络错误和服务器错误
  if (status === 0 || status >= 500) return true;
  // 429 Too Many Requests 也重试
  if (status === 429) return true;
  return false;
}

// 请求核心
interface RequestOptions {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  auth?: boolean;
  skipCache?: boolean;
  timeout?: number;
  signal?: AbortSignal;
  // 静默失败：不触发全局 onErrorCb（后台轮询/心跳等场景，仅记日志）
  silent?: boolean;
  // 写请求后跳过缓存失效（分片上传等高频写不清理 /videos 缓存）
  noInvalidate?: boolean;
  // 重试次数（默认 3 次）
  retries?: number;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const url = BASE + path;
  const { 
    method = 'GET', 
    body, 
    headers = {}, 
    auth = true, 
    skipCache = false, 
    timeout, 
    signal, 
    silent = false, 
    noInvalidate = false,
    retries = MAX_RETRIES 
  } = options;

  const cacheKey = method === 'GET' && !skipCache ? getCacheKey(url, method) : null;
  if (cacheKey) {
    const cached = cacheGet(cacheKey);
    if (cached !== undefined) return cached as T;
  }

  // Blob/FormData 直传（分片、multipart 上传）：原样透传，且不能预设 JSON Content-Type
  const isRawBody = body instanceof Blob || body instanceof FormData;
  // 复制 header，不污染调用方传入的对象
  const requestHeaders: Record<string, string> = {
    'X-Requested-With': 'XMLHttpRequest',
    ...(isRawBody ? {} : { 'Content-Type': 'application/json' }),
    ...headers,
  };
  if (auth) {
    const token = getToken();
    if (token) requestHeaders['Authorization'] = 'Bearer ' + token;
  }

  // 重试逻辑
  for (let attempt = 0; attempt <= retries; attempt++) {
    // 超时 + 调用方取消信号组合：手动 AbortController，
    // 可区分"超时"与"主动取消"两种 AbortError
    const controller = new AbortController();
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      controller.abort();
    }, timeout || API_TIMEOUT);
    const onAbort = () => controller.abort();
    if (signal) {
      if (signal.aborted) controller.abort();
      else signal.addEventListener('abort', onAbort, { once: true });
    }

    const fetchOpts: RequestInit = {
      method,
      headers: requestHeaders,
      credentials: 'same-origin',
      signal: controller.signal,
    };
    if (body !== undefined) {
      fetchOpts.body = body instanceof Blob || body instanceof FormData ? body : JSON.stringify(body);
    }

    try {
      let res: Response;
      try {
        res = await fetch(url, fetchOpts);
      } catch (e) {
        // 调用方主动取消：原样抛出 AbortError，由 react-query/组件决定如何处理
        if (signal?.aborted) throw e;
        if (timedOut) {
          const msg = i18n.t?.('errors.timeout') || '请求超时';
          if (!silent) logError({ message: msg, url, status: 0 });
          throw new APIError(msg, 0);
        }
        if ((e as Error)?.name === 'AbortError') throw e;
        const msg = i18n.t?.('errors.network') || '网络连接失败';
        if (!silent) logError({ message: msg, url, status: 0 });
        throw new APIError(msg, 0);
      }

      // 204 No Content (or 205 Reset Content) — short-circuit before JSON parse.
      if (res.status === 204 || res.status === 205) {
        if (method !== 'GET' && !noInvalidate) invalidateCacheForPath(path, body);
        return null as T;
      }

      let data: unknown;
      try {
        data = await res.json();
      } catch (e) {
        // 读取响应体期间超时/取消
        if (signal?.aborted || ((e as Error)?.name === 'AbortError' && !timedOut)) throw e;
        if (timedOut) {
          const msg = i18n.t?.('errors.timeout') || '请求超时';
          if (!silent) logError({ message: msg, url, status: 0 });
          throw new APIError(msg, 0);
        }
        // Response is not JSON
        if (res.status === 401 && auth) {
          clearToken();
          if (onAuthRequiredCb) onAuthRequiredCb();
          throw new AuthError();
        }
        const errorMsg = resolveErrorMessage(res.status);
        logError({ message: errorMsg, url, status: res.status });
        const apiErr = new APIError(errorMsg, res.status);
        if (!silent && onErrorCb) onErrorCb(apiErr);
        throw apiErr;
      }

      if (res.status === 401 && auth) {
        clearToken();
        const dataObj = data as Record<string, unknown>;
        const rawMsg = dataObj && typeof dataObj.error === 'string' ? dataObj.error : undefined;
        const msg = localizeAuthMessage(rawMsg);
        if (onAuthRequiredCb) onAuthRequiredCb(msg);
        throw new AuthError(msg);
      }

      if (!res.ok) {
        const dataObj = data as Record<string, unknown>;
        const rawMsg = dataObj && typeof dataObj.error === 'string' ? dataObj.error : undefined;
        const msg = resolveErrorMessage(res.status, rawMsg);
        
        // 判断是否重试
        if (shouldRetry(res.status, attempt)) {
          const delay = getRetryDelay(attempt);
          await new Promise(resolve => setTimeout(resolve, delay));
          continue;
        }
        
        logError({ message: msg, url, status: res.status });
        const apiErr = new APIError(msg, res.status);
        if (!silent && onErrorCb) onErrorCb(apiErr);
        throw apiErr;
      }

      if (cacheKey) cacheSet(cacheKey, data);
      if (method !== 'GET' && !noInvalidate) invalidateCacheForPath(path, body);
      return data as T;
    } finally {
      clearTimeout(timer);
      if (signal) signal.removeEventListener('abort', onAbort);
    }
  }

  // 如果所有重试都失败，抛出最后一个错误
  throw new APIError(i18n.t?.('errors.network') || '网络连接失败', 0);
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
