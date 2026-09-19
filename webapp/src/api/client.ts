// API 客户端核心
//
// ─────────────────────────────────────────────────────────────
// 缓存失效约定（唯一入口：invalidateCacheForPath）
// ─────────────────────────────────────────────────────────────
// 项目存在两层响应缓存：
//   1. LRU 响应缓存（下方 cache Map，GET 30s TTL，键含登录态标识）
//   2. react-query 查询缓存（../lib/queryClient.ts，staleTime 60s）
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
//     对应视频的 /playback/history 与 /videos/{id}，并标记历史类查询
//     （my-history / recent-videos）陈旧，不扫全表，避免看视频期间
//     /playback 缓存永远打不中
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
const RETRY_DELAY = 1000;
// 幂等方法可安全重试；写请求（POST/PUT/PATCH/DELETE）默认不重试，避免重复副作用
const IDEMPOTENT_METHODS: readonly string[] = ['GET', 'HEAD', 'OPTIONS'];

export function getCsrfToken(): string | null {
  const match = document.cookie.match(/(?:^|;\s*)csrf_token=([^;]*)/);
  return match?.[1] ? decodeURIComponent(match[1]) : null;
}

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
  inFlightRequests.clear();
}

function sanitizePath(path: string): string {
  // 只校验 pathname：查询串中允许出现 ".."（如搜索词），否则搜索会直接抛错。
  // NUL 字符对整条 URL 都非法，保留全串检查。
  const pathname = path.split('?')[0] ?? '';
  if (pathname.includes('..') || path.includes('\0')) {
    throw new APIError('Invalid request path', 0);
  }
  return path;
}

// 缓存键格式为 METHOD:token:url。JWT 不含 ':'（base64url + '.'），
// 因此按第二个 ':' 之后取出 url 是安全的。BASE 非空（file: 协议下为
// http://localhost:8082）时先剥掉还原为 path，再做路径前缀匹配，
// 避免 URL 中段包含相同片段时被误删（如 /admin/videos 命中 /videos）。
function cacheInvalidatePrefix(urlPrefix: string) {
  if (!urlPrefix) { cache.clear(); return; }
  for (const key of cache.keys()) {
    if (!key.startsWith('GET:')) continue;
    const urlStart = key.indexOf(':', 4);
    if (urlStart === -1) continue;
    let path = key.slice(urlStart + 1);
    if (BASE && path.startsWith(BASE)) path = path.slice(BASE.length);
    if (path === urlPrefix || path.startsWith(urlPrefix + '/') || path.startsWith(urlPrefix + '?')) {
      cache.delete(key);
    }
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
  // 评论删除 → 清 /comments LRU；评论列表由 Comments 组件乐观更新并自行 invalidate
  { writePrefix: '/comments', lruPrefixes: ['/comments'], queryKeyPrefixes: [] },
  // 视频域写操作（增删改/播放量/评论/标签/分享/转码/扫描）：
  //   - my-history / recent-videos：删除/焚毁级联清理播放历史，编辑改变历史中的标题
  //   - my-shares：/videos/{id}/share 创建/删除分享，删视频级联清理分享记录
  //   - /auth/user/shares、/admin/stats 的 LRU 必须同步清，否则 rq 重取仍命中旧 LRU
  { writePrefix: '/admin/videos', lruPrefixes: ['/videos', '/playback', '/admin/videos', '/auth/user/shares', '/admin/stats'], queryKeyPrefixes: ['home-videos', 'trending-videos', 'my-works', 'my-favorites', 'my-history', 'recent-videos', 'my-shares', 'admin-stats'] },
  { writePrefix: '/videos', lruPrefixes: ['/videos', '/playback', '/auth/user/shares', '/admin/stats'], queryKeyPrefixes: ['home-videos', 'trending-videos', 'my-works', 'my-favorites', 'my-history', 'recent-videos', 'my-shares', 'admin-stats'] },
  // 标签管理 → 标签列表
  { writePrefix: '/admin/tags', lruPrefixes: ['/tags'], queryKeyPrefixes: ['admin-tags'] },
  // 用户管理 → 用户列表（UsersTab 亦手动 invalidate）；删除/审批改变
  // admin-stats 的 userCount/pendingCount，需连 /admin/stats 的 LRU 一起清
  { writePrefix: '/admin/users', lruPrefixes: ['/admin/users', '/admin/stats'], queryKeyPrefixes: ['admin-stats'] },
  // 日志清空（日志页非 react-query，只需清 LRU）
  { writePrefix: '/admin/logs', lruPrefixes: ['/admin/logs'], queryKeyPrefixes: [] },
  // 聊天室消息清空 → 聊天统计 + 已缓存的聊天历史
  { writePrefix: '/admin/chat', lruPrefixes: ['/admin/chat', '/chat/messages'], queryKeyPrefixes: ['admin-chat-stats'] },
  // 注册开关
  { writePrefix: '/admin/config/registration', lruPrefixes: ['/admin/config/registration'], queryKeyPrefixes: ['admin-registration-enabled'] },
];

// 未列入规则表的 react-query 键（有意不失效）：
//   - favorite-status：useFavoriteHandler 写后 setQueryData 直接改缓存
//   - admin-users：UsersTab 每次写操作后手动 invalidateQueries
//   - health / admin-system-info：SystemTab（30s）与 DashboardTab（60s）
//     轮询自动刷新，写后无需立即失效
//   - my-playlists：/playlists 写规则已覆盖；视频删除对 item_count 的影响
//     由组件挂载重取兜底
//   - user-profile：头像/邮箱有专属规则；观看统计字段的变化源是播放历史，
//     Profile 查询 staleTime 60s 且窗口聚焦自动重取，不为其扩大视频域规则

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

// 写事件注册表：写请求成功后由 invalidateCacheForPath 统一广播。
// 供无法用 INVALIDATION_RULES 表达的模块私有缓存（如 playback.ts 的
// historyCache）自行失效 —— 监听方从 client 单向 import，避免反向依赖成环。
export type WriteListener = (path: string, body?: unknown) => void;

const writeListeners = new Set<WriteListener>();

/** 注册写操作监听，返回退订函数；监听器异常不影响请求主流程 */
export function registerWriteListener(cb: WriteListener): () => void {
  writeListeners.add(cb);
  return () => {
    writeListeners.delete(cb);
  };
}

function notifyWriteListeners(path: string, body?: unknown) {
  for (const cb of writeListeners) {
    try {
      cb(path, body);
    } catch {
      // 监听器失败不应影响写请求本身
    }
  }
}

function invalidateCacheForPath(path: string, body?: unknown) {
  // 播放会话心跳/启停：不改变任何可缓存的 GET 响应，跳过失效与事件广播
  if (path.startsWith('/playback/session/')) return;

  // 先广播写事件，再按规则表失效两层响应缓存；
  // 调用方传 noInvalidate 的高频写不会进入本函数，无需额外屏蔽
  notifyWriteListeners(path, body);

  // 会话边界：登出后两层缓存整体作废，后续读取从服务器取最新数据
  if (path === '/auth/logout') {
    cache.clear();
    queryClient.clear();
    return;
  }

  // 播放进度上报（播放中每 10s 一次）：只精确失效对应视频的进度/详情，
  // 不扫全表，避免看视频期间 /playback 缓存永远打不中
  if (path === '/playback/history') {
    const videoId = extractVideoId(body);
    cacheInvalidatePrefix('/playback/history');
    cacheInvalidatePrefix(typeof videoId === 'number' ? `/videos/${videoId}` : '/videos');
    // my-history（个人历史）与 recent-videos（首页最近观看）同源，必须一起标记
    invalidateReactQuery(['my-history', 'recent-videos']);
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
  /** 后端返回的机器可读错误码（如 duplicate / quota_exceeded / offset_mismatch） */
  code?: string;
  /** 后端返回的结构化错误数据（如 offset_mismatch 的 { received }） */
  data?: unknown;
  constructor(message: string, status: number, code?: string, data?: unknown) {
    super(message);
    this.name = 'APIError';
    this.status = status;
    this.code = code;
    this.data = data;
  }
}

export class AuthError extends APIError {
  constructor(message?: string) {
    super(message || resolveErrorMessage(401), 401);
    this.name = 'AuthError';
  }
}

export class ValidationError extends APIError {
  field?: string;
  constructor(message: string, field?: string) {
    super(message, 422);
    this.name = 'ValidationError';
    this.field = field;
  }
}

export class RateLimitError extends APIError {
  retryAfter?: number;
  constructor(message?: string, retryAfter?: number) {
    super(message || resolveErrorMessage(429), 429);
    this.name = 'RateLimitError';
    this.retryAfter = retryAfter;
  }
}

export class NotFoundError extends APIError {
  constructor(message?: string) {
    super(message || resolveErrorMessage(404), 404);
    this.name = 'NotFoundError';
  }
}

export class NetworkError extends APIError {
  constructor(message?: string) {
    super(message || i18n.t?.('errors.network') || '网络连接失败', 0);
    this.name = 'NetworkError';
  }
}

export class TimeoutError extends APIError {
  constructor(message?: string) {
    super(message || i18n.t?.('errors.timeout') || '请求超时', 0);
    this.name = 'TimeoutError';
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

// 请求去重：相同 GET 请求在飞行中时复用同一个 Promise
const inFlightRequests = new Map<string, Promise<unknown>>();

// 请求核心
interface RequestOptions {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  auth?: boolean;
  skipCache?: boolean;
  timeout?: number;
  signal?: AbortSignal;
  silent?: boolean;
  noInvalidate?: boolean;
  retries?: number;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  sanitizePath(path);
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
    retries
  } = options;

  // 仅幂等方法默认重试；写请求默认 0 次，显式传入的 retries 始终优先
  const maxRetries = retries ?? (IDEMPOTENT_METHODS.includes(method.toUpperCase()) ? MAX_RETRIES : 0);

  const cacheKey = method === 'GET' && !skipCache ? getCacheKey(url, method) : null;
  if (cacheKey) {
    const cached = cacheGet(cacheKey);
    if (cached !== undefined) return cached as T;

    const inflight = inFlightRequests.get(cacheKey);
    if (inflight) return inflight as Promise<T>;
  }

  const isRawBody = body instanceof Blob || body instanceof FormData;
  const requestHeaders: Record<string, string> = {
    'X-Requested-With': 'XMLHttpRequest',
    'Accept': 'application/json',
    ...(isRawBody ? {} : { 'Content-Type': 'application/json' }),
    ...headers,
  };
  if (method !== 'GET' && method !== 'HEAD') {
    const csrf = getCsrfToken();
    if (csrf) requestHeaders['X-CSRF-Token'] = csrf;
  }
  if (auth) {
    const token = getToken();
    if (token) requestHeaders['Authorization'] = 'Bearer ' + token;
  }

  const execRequest = async (): Promise<T> => {
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      const controller = new AbortController();
      let timedOut = false;
      const effectiveTimeout = timeout || API_TIMEOUT;
      const timer = setTimeout(() => {
        timedOut = true;
        controller.abort();
      }, effectiveTimeout);
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
        fetchOpts.body = isRawBody ? body : JSON.stringify(body);
      }

      try {
        let res: Response;
        try {
          res = await fetch(url, fetchOpts);
        } catch (e) {
          if (signal?.aborted) throw e;
          if (timedOut) throw new TimeoutError();
          if ((e as Error)?.name === 'AbortError') throw e;
          throw new NetworkError();
        }

        if (res.status === 204 || res.status === 205) {
          if (method !== 'GET' && !noInvalidate) invalidateCacheForPath(path, body);
          return null as T;
        }

        let data: unknown;
        try {
          data = await res.json();
        } catch (e) {
          if (signal?.aborted || ((e as Error)?.name === 'AbortError' && !timedOut)) throw e;
          if (timedOut) throw new TimeoutError();
          const errorMsg = resolveErrorMessage(res.status);
          if (res.status === 401 && auth) {
            clearToken();
            if (onAuthRequiredCb) onAuthRequiredCb();
            throw new AuthError();
          }
          if (!silent) logError({ message: errorMsg, url, status: res.status });
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
          const code =
            dataObj && typeof dataObj.code === 'string' ? dataObj.code : undefined;
          const errData = dataObj ? dataObj.data : undefined;

          if (res.status === 429) {
            const retryAfter = res.headers?.get?.('Retry-After');
            const delay = retryAfter
              ? Math.min(Number(retryAfter) * 1000, 30000)
              : getRetryDelay(attempt);
            if (attempt < maxRetries) {
              await new Promise(resolve => setTimeout(resolve, delay));
              continue;
            }
            throw new RateLimitError(msg, retryAfter ? Number(retryAfter) : undefined);
          }

          if (attempt < maxRetries && shouldRetry(res.status, attempt)) {
            await new Promise(resolve => setTimeout(resolve, getRetryDelay(attempt)));
            continue;
          }

          if (!silent) logError({ message: msg, url, status: res.status });
          const apiErr = new APIError(msg, res.status, code, errData);
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

    throw new NetworkError();
  };

  const promise = execRequest();

  if (cacheKey) {
    inFlightRequests.set(cacheKey, promise);
    promise.then(
      () => inFlightRequests.delete(cacheKey),
      () => inFlightRequests.delete(cacheKey)
    );
  }

  return promise;
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
