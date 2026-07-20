// API 类型定义

export interface Video {
  id: number;
  title: string;
  description: string;
  sourceType: string;
  coverUrl: string | null;
  streamUrl: string;
  thumbUrl: string | null;
  category: string;
  views: number;
  duration: number;
  createdAt: string;
  watchPosition?: number;
  hasVariants?: boolean;
  uploaderId?: number;
}

export interface VideoVariant {
  resolution: string;
  filePath: string;
  fileSize: number;
  bitrate?: number;
}

export interface TranscodeStatusResponse {
  videoId: number;
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
  videoId: number;
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
  id: number;
  username: string;
  isAdmin: boolean;
  avatarUrl?: string;
  createdAt: string;
}

export interface UserProfile extends UserInfo {
  totalVideosWatched: number;
  totalWatchTimeMs: number;
  recentHistory: PlaybackHistory[];
}

export interface AuthResponse {
  ok: boolean;
  token?: string;
  error?: string;
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
  id: number;
  title: string;
  category: string;
  description: string;
  thumb: string | null;
  stream: string | null;
  cover: string | null;
  sourceType: string;
  duration: number;
  views: number;
  date: string;
  progress: number;
  hasVariants?: boolean;
  uploaderId?: number;
}

export interface MappedImage {
  id: number;
  title: string;
  category: string;
  thumb: string | null;
  sourceType: string;
}

export interface MappedHistory {
  id: number;
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
  id: number;
  name: string;
  description: string | null;
  isPublic: boolean;
  coverUrl: string | null;
  itemCount: number;
  createdAt: string;
  updatedAt: string;
}
