/**
 * 视频 API 模块
 *
 * 提供视频列表查询、详情获取、播放量统计、收藏管理、转码、搜索和断点续传等功能。
 *
 * @module videos
 */

import { request } from './client';
import type { VideoListResponse, Video, TranscodeStatusResponse, PlaybackHistory } from './types';

/** 分页查询单页最大条目数 */
const MAX_PAGE_SIZE = 1000;
/** 后端 /videos/search 的 size 上限 */
const MAX_SEARCH_SIZE = 100;

/**
 * 视频列表查询参数
 */
interface ListVideosParams {
  /** 搜索关键词，用于模糊匹配视频标题 */
  query?: string;
  /** 视频类型过滤（如 'movie', 'series' 等） */
  type?: string;
  /** 视频分类过滤（如 'tech', 'gaming' 等） */
  category?: string;
  /** 页码，从 0 开始，默认 0 */
  page?: number;
  /** 每页条目数，默认 20，最大 1000 */
  size?: number;
  /** 上传者 ID，用于筛选特定用户的视频 */
  uploaderId?: string;
  /** 排序方式（如 'newest', 'oldest', 'popular' 等） */
  sort?: string;
}

/**
 * 获取视频列表（分页、筛选、排序）
 *
 * 支持按关键词搜索、类型/分类/上传者筛选、自定义排序。自动处理分页边界：
 * - page 最小为 0
 * - size 最大为 1000
 *
 * @param params - 查询参数对象
 * @returns 包含视频列表和分页信息的响应
 * @throws {RequestError} 当网络请求失败或后端返回错误时抛出
 *
 * @example
 * ```typescript
 * // 获取第一页 20 条视频（默认）
 * const defaultList = await listVideos();
 *
 * // 按关键词搜索
 * const searchResult = await listVideos({ query: '教程', page: 0, size: 50 });
 *
 * // 按分类筛选并排序
 * const techVideos = await listVideos({
 *   category: 'tech',
 *   sort: 'newest',
 *   page: 1,
 *   size: 10
 * });
 *
 * // 获取特定用户的视频
 * const userVideos = await listVideos({ uploaderId: 'user123' });
 * ```
 */
export async function listVideos({
  query,
  type,
  category,
  page = 0,
  size = 20,
  uploaderId,
  sort,
}: ListVideosParams = {}): Promise<VideoListResponse> {
  const params = new URLSearchParams();
  if (query) params.set('query', query);
  if (type) params.set('type', type);
  if (category) params.set('category', category);
  if (uploaderId !== undefined) params.set('uploader_id', String(uploaderId));
  if (sort) params.set('sort', sort);
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(size, MAX_PAGE_SIZE)));
  return request<VideoListResponse>(`/videos?${params}`);
}

/**
 * 获取单个视频详情
 *
 * 根据视频 ID 获取完整视频信息，包括标题、描述、分类、上传者、播放量等。
 *
 * @param id - 视频唯一标识符（Hash ID 格式）
 * @returns 视频详细信息对象
 * @throws {RequestError} 当视频不存在或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * const video = await getVideo('abc123');
 * console.log(video.title, video.views);
 * ```
 */
export async function getVideo(id: string): Promise<Video> {
  return request<Video>(`/videos/${id}`);
}

/**
 * 增加视频播放量
 *
 * 每次调用该接口会为指定视频的播放计数 +1。建议在视频开始播放时调用一次，
 * 避免重复调用导致播放量虚高。
 *
 * @param id - 视频唯一标识符（Hash ID 格式）
 * @throws {RequestError} 当视频不存在或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * await incrementViews('abc123');
 * console.log('播放量已更新');
 * ```
 */
export async function incrementViews(id: string): Promise<void> {
  await request(`/videos/${id}/view`, { method: 'POST' });
}

/**
 * 删除单个视频（管理员接口）
 *
 * 需要管理员权限。删除视频及其关联的媒体文件、缩略图等资源。
 * 此操作不可逆，请谨慎使用。
 *
 * @param id - 视频唯一标识符（Hash ID 格式）
 * @throws {RequestError} 当权限不足、视频不存在或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * try {
 *   await deleteVideo('abc123');
 *   console.log('视频已删除');
 * } catch (error) {
 *   console.error('删除失败:', error.message);
 * }
 * ```
 */
export async function deleteVideo(id: string): Promise<void> {
  await request(`/admin/videos/${id}`, { method: 'DELETE', auth: true });
}

/**
 * 批量删除视频（管理员接口）
 *
 * 需要管理员权限。一次性删除多个视频，单次最多 1000 个。
 * 此操作不可逆，请谨慎使用。
 *
 * @param ids - 视频 ID 数组，每个 ID 为 Hash ID 格式
 * @throws {RequestError} 当权限不足、参数无效或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * await deleteVideos(['abc123', 'def456', 'ghi789']);
 * console.log('批量删除成功');
 * ```
 */
export async function deleteVideos(ids: string[]): Promise<void> {
  await request('/admin/videos/batch', { method: 'DELETE', body: ids, auth: true });
}

/**
 * 获取当前用户的收藏视频列表
 *
 * 返回按收藏时间倒序排列的视频列表，包含播放历史信息。
 *
 * @returns 收藏视频列表，每个元素包含视频信息和播放进度
 * @throws {RequestError} 当用户未登录或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * const favorites = await listFavorites();
 * favorites.forEach(video => {
 *   console.log(`已收藏: ${video.title}, 播放进度: ${video.progress}`);
 * });
 * ```
 */
export async function listFavorites(): Promise<PlaybackHistory[]> {
  return request<PlaybackHistory[]>('/videos/favorites');
}

export async function toggleFavorite(videoId: string): Promise<{ favorited: boolean }> {
  return request<{ favorited: boolean }>(`/videos/${videoId}/favorite`, { method: 'POST' });
}

export async function getFavoriteStatus(videoId: string): Promise<{ favorited: boolean }> {
  return request<{ favorited: boolean }>(`/videos/${videoId}/favorite`);
}

// ========== Transcode API ==========

/**
 * 启动视频转码任务（管理员接口）
 *
 * 需要管理员权限。将视频转码为指定分辨率。转码完成后视频会生成对应的
 * 多清晰度文件，供播放器自适应选择。
 *
 * @param videoId - 视频唯一标识符（Hash ID 格式）
 * @param resolutions - 目标分辨率数组，如 ['720p', '1080p', '4k']
 * @returns 转码任务创建结果，包含 success 状态和提示消息
 * @throws {RequestError} 当权限不足、视频不存在、分辨率无效或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * const result = await transcodeVideo('abc123', ['720p', '1080p']);
 * if (result.success) {
 *   console.log('转码任务已创建:', result.message);
 * } else {
 *   console.error('创建失败:', result.message);
 * }
 * ```
 */
export async function transcodeVideo(
  videoId: string,
  resolutions: string[]
): Promise<{ success: boolean; message: string }> {
  return request(`/admin/videos/${videoId}/transcode`, {
    method: 'POST',
    body: { resolutions },
    auth: true,
  });
}

/**
 * 查询视频转码状态（管理员接口）
 *
 * 需要管理员权限。用于轮询转码进度，获取当前各分辨率的转码状态。
 * 此接口使用 silent 模式，失败时不会弹出全局 Toast，由调用方自行处理。
 *
 * @param videoId - 视频唯一标识符（Hash ID 格式）
 * @returns 转码状态响应，包含各分辨率的转码进度和状态
 * @throws {RequestError} 当权限不足、视频不存在或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * const status = await getTranscodeStatus('abc123');
 * if (status.status === 'completed') {
 *   console.log('转码完成');
 * } else if (status.status === 'processing') {
 *   console.log(`转码中，进度: ${status.progress}%`);
 * }
 * ```
 */
export async function getTranscodeStatus(
  videoId: string
): Promise<TranscodeStatusResponse> {
  // silent：转码状态查询由页面自行展示错误，避免轮询/刷新失败弹全局 Toast
  return request(`/admin/videos/${videoId}/transcode/status`, { auth: true, silent: true });
}

// ========== Search API ==========

/**
 * 搜索结果项
 */
export interface SearchResult {
  /** 视频 ID（Hash ID 格式） */
  id: string;
  /** 视频标题 */
  title: string;
  /** 视频描述，可能为 null */
  description: string | null;
  /** 视频分类，可能为 null */
  category: string | null;
  /** 搜索相关度评分，值越高越相关 */
  rank: number;
  /** 高亮片段，包含搜索关键词的上下文，可能为 null */
  headline: string | null;
}

/**
 * 搜索响应结构
 */
export interface SearchResponse {
  /** 搜索结果列表 */
  items: SearchResult[];
  /** 符合条件的总记录数 */
  total: number;
  /** 当前页码 */
  page: number;
  /** 每页条目数 */
  size: number;
}

/**
 * 全文搜索视频
 *
 * 使用 PostgreSQL 全文搜索引擎，支持标题和描述的模糊匹配。
 * 返回结果按相关度排序，包含高亮片段。
 *
 * @param query - 搜索关键词
 * @param page - 页码，从 0 开始，默认 0
 * @param size - 每页条目数，默认 20，最大 100
 * @returns 包含搜索结果和分页信息的响应
 * @throws {RequestError} 当查询为空或网络请求失败时抛出
 *
 * @example
 * ```typescript
 * // 基础搜索
 * const results = await searchVideos('教程');
 * console.log(`找到 ${results.total} 个结果`);
 *
 * // 带分页搜索
 * const page2 = await searchVideos('游戏', 1, 10);
 *
 * // 展示高亮片段
 * results.items.forEach(item => {
 *   console.log(`${item.title}: ${item.headline}`);
 * });
 * ```
 */
export async function searchVideos(
  query: string,
  page = 0,
  size = 20
): Promise<SearchResponse> {
  const trimmed = query.trim();
  if (!trimmed) return { items: [], total: 0, page: 0, size };
  const params = new URLSearchParams();
  params.set('q', trimmed);
  params.set('page', String(Math.max(0, page)));
  params.set('size', String(Math.min(Math.max(1, size), MAX_SEARCH_SIZE)));
  return request(`/videos/search?${params}`);
}

/**
 * 搜索建议（自动补全）
 *
 * 根据输入的关键词返回搜索建议词列表，用于实现搜索框的自动补全功能。
 * 建议词基于视频标题的前缀匹配和历史搜索频率生成。
 *
 * @param query - 搜索关键词（建议至少 2 个字符）
 * @returns 建议词字符串数组，按相关度排序
 * @throws {RequestError} 当网络请求失败时抛出
 *
 * @example
 * ```typescript
 * // 监听输入框变化
 * const suggestions = await searchSuggest('游戏');
 * // 返回: ['游戏', '游戏攻略', '游戏直播', ...]
 *
 * // 渲染建议列表
 * suggestions.forEach(suggestion => {
 *   dropdown.addOption(suggestion);
 * });
 * ```
 */
export async function searchSuggest(
  query: string
): Promise<string[]> {
  const trimmed = query.trim();
  if (!trimmed) return [];
  const params = new URLSearchParams();
  params.set('q', trimmed);
  return request(`/videos/search/suggest?${params}`);
}

// ========== Upload resume API ==========

/**
 * 查询上传状态（断点续传）
 *
 * 根据文件 hash 查询已上传的字节数，用于实现断点续传功能。
 * 此接口使用 skipCache 和 silent 模式：
 * - skipCache: 上传进度随时变化，不能命中 30s 响应缓存
 * - silent: 断点查询失败不弹全局 Toast（页面自行回退到从头上传）
 *
 * @param hash - 文件的 SHA-256 哈希值，用于唯一标识文件
 * @returns 包含已接收字节数的对象
 * @throws {RequestError} 当网络请求失败时抛出（但 silent 模式不会弹 Toast）
 *
 * @example
 * ```typescript
 * // 计算文件 hash
 * const hash = await calculateFileHash(file);
 *
 * // 查询已上传进度
 * const { received } = await getUploadStatus(hash);
 *
 * if (received > 0) {
 *   console.log(`已上传 ${received} 字节，从 ${received} 处继续`);
 *   // 继续上传剩余部分
 *   const remaining = file.slice(received);
 *   await uploadResumeChunk(hash, file.name, file.size, category, remaining);
 * } else {
 *   console.log('从头开始上传');
 * }
 * ```
 */
export async function getUploadStatus(hash: string): Promise<{ received: number }> {
  // skipCache：上传进度随时在变，不能命中 30s 响应缓存
  // silent：断点查询失败不弹全局 Toast（页面自行回退到从头上传）
  return request<{ received: number }>(
    `/admin/videos/upload-status?hash=${encodeURIComponent(hash)}`,
    { auth: true, skipCache: true, silent: true }
  );
}

/**
 * 清理 HTTP Header 值中的换行符和首尾空格
 *
 * 防止 HTTP Header 注入攻击，移除 \r 和 \n 字符。
 *
 * @param value - 原始 header 值
 * @returns 清理后的安全字符串
 * @internal
 */
function sanitizeHeaderValue(value: string): string {
  return value.replace(/[\r\n]/g, '').trim();
}

/**
 * 上传文件分片（断点续传）
 *
 * 将文件分片上传到服务器，支持断点续传。此接口使用 silent 和 noInvalidate 模式：
 * - silent: 分片失败由上传页按文件展示 errorMsg，不弹 Toast
 * - noInvalidate: 分片请求是高频写，不能每个分片都清空 /videos 缓存
 *
 * @param hash - 文件的 SHA-256 哈希值，用于唯一标识文件
 * @param fileName - 原始文件名，会经过安全清理
 * @param totalSize - 文件总大小（字节）
 * @param category - 视频分类
 * @param chunk - 文件分片数据（Blob 对象）
 * @returns 包含已接收字节数和可选视频 ID 的对象
 * @throws {RequestError} 当权限不足、参数无效或网络请求失败时抛出（但 silent 模式不会弹 Toast）
 *
 * @example
 * ```typescript
 * const file = event.target.files[0];
 * const hash = await calculateFileHash(file);
 * const chunkSize = 5 * 1024 * 1024; // 5MB 分片
 * const category = 'tech';
 *
 * // 查询已上传进度
 * const { received } = await getUploadStatus(hash);
 *
 * // 从断点继续上传
 * for (let offset = received; offset < file.size; offset += chunkSize) {
 *   const chunk = file.slice(offset, Math.min(offset + chunkSize, file.size));
 *   const result = await uploadResumeChunk(hash, file.name, file.size, category, chunk);
 *
 *   if (result.id) {
 *     console.log(`上传完成，视频 ID: ${result.id}`);
 *     break;
 *   }
 * }
 * ```
 */
export async function uploadResumeChunk(
  hash: string,
  fileName: string,
  totalSize: number,
  category: string,
  chunk: Blob
): Promise<{ received: number; id?: string }> {
   const headers: Record<string, string> = {
    'x-upload-hash': hash,
    'x-upload-name': sanitizeHeaderValue(fileName),
    'x-upload-size': String(totalSize),
    'x-upload-category': sanitizeHeaderValue(category),
  };
  // Blob 直传复用 request()：统一超时、后端中文错误本地化（保留中文原文）、
  // 401 触发全局登出。silent：分片失败由上传页按文件展示 errorMsg，不弹 Toast；
  // noInvalidate：分片请求是高频写，不能每个分片都清空 /videos 缓存。
  return request<{ received: number; id?: string }>('/admin/videos/upload-resume', {
    method: 'POST',
    body: chunk,
    headers,
    silent: true,
    noInvalidate: true,
  });
}
