// API 类型定义

// 通用 API 响应
export interface ApiResponse<T> {
  data: T;
  message?: string;
}

// 分页响应
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

// 错误响应
export interface ApiError {
  error: string;
  code?: string;
  details?: Record<string, unknown>;
}

// 认证响应
export interface AuthResponse {
  ok: boolean;
  token?: string;
  error?: string;
}

export type SourceType = 'local_video' | 'local_image' | 'external'
export type Category = '科技' | '设计' | '音乐' | '教程' | '娱乐' | '运动' | '记录' | '外部' | '全部' | 'general'

export interface Video {
  id: string;
  title: string;
  description: string;
  sourceType: SourceType;
  coverUrl: string | null;
  streamUrl: string;
  thumbUrl: string | null;
  category: Category | string;
  views: number;
  duration: number;
  createdAt: string;
  watchPosition?: number;
  hasVariants?: boolean;
  uploaderId?: string;
}

export interface VideoVariant {
  resolution: string;
  filePath: string;
  fileSize: number;
  bitrate?: number;
}

export interface TranscodeStatusResponse {
  videoId: string;
  variants: VideoVariant[];
  pendingJobs: Array<{
    id: number;
    resolution: string;
    status: string;
    progress: number;
  }>;
}

export interface VideoListResponse {
  items: Video[];
  total: number;
  page: number;
  size: number;
}

export interface PlaybackHistory {
  videoId: string;
  title: string;
  coverUrl: string | null;
  streamUrl: string;
  sourceType: string;
  category: string;
  positionMs: number;
  durationMs: number;
  updatedAt: string;
}

export interface UserInfo {
  id: string;
  username: string;
  isAdmin: boolean;
  avatarUrl?: string;
  createdAt: string;
  email?: string;
  emailVerified: boolean;
}

export interface UserProfile extends UserInfo {
  totalVideosWatched: number;
  totalWatchTimeMs: number;
  recentHistory: PlaybackHistory[];
}

export interface ServerInfo {
  version: string;
}

export interface HealthResponse {
  status: string;
  db: string;
  uptime_secs: number;
}

// 映射后的类型
export interface MappedVideo {
  id: string;
  title: string;
  category: Category | string;
  description: string;
  thumb: string | null;
  thumbnail_url?: string;  // 可选字段，兼容Video接口
  stream: string | null;
  cover: string | null;
  sourceType: SourceType;
  duration: number;
  views: number;
  date: string;
  progress: number;
  hasVariants?: boolean;
  uploaderId?: string;
}

export interface MappedImage {
  id: string;
  title: string;
  category: Category | string;
  thumb: string | null;
  sourceType: SourceType;
}

export interface MappedHistory {
  id: string;
  title: string;
  category: string;
  thumb: string | null;
  stream: string | null;
  sourceType: string;
  positionMs: number;
  durationMs: number;
  updatedAt: string;
  progress: number;
}

export interface MappedPlaylist {
  id: string;
  name: string;
  description: string | null;
  isPublic: boolean;
  coverUrl: string | null;
  itemCount: number;
  createdAt: string;
  updatedAt: string;
}
