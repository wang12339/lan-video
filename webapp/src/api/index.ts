// API 模块导出

export { BASE, getToken, clearToken, mediaUrl, cacheClear, APIError, AuthError, ValidationError, RateLimitError, NotFoundError, NetworkError, TimeoutError, setOnAuthRequired, setOnError } from './client';
export { health } from './client';
export type { VideoListResponse, Video, PlaybackHistory, UserInfo, UserProfile, AuthResponse, ServerInfo, HealthResponse, MappedVideo, MappedImage, MappedHistory, MappedPlaylist, VideoVariant, TranscodeStatusResponse } from './types';
export { login, register, logout, getUserInfo, getUserProfile, checkSession, uploadAvatar, sendVerificationEmail, updateEmail, forgotPassword, resetPassword, verifyEmail } from './auth';
export { listVideos, getVideo, incrementViews, deleteVideo, deleteVideos, listFavorites, toggleFavorite, getFavoriteStatus, getUploadStatus, uploadResumeChunk, transcodeVideo, getTranscodeStatus, searchVideos, searchSuggest } from './videos';
export type { SearchResult, SearchResponse } from './videos';
export { savePlayback, listPlaybackHistory, startPlaybackSession, heartbeatPlaybackSession, stopPlaybackSession } from './playback';
export { mapVideo, mapImage, mapHistory, mapPlaylist, formatDuration, formatViews, formatCount, formatBytes, getCatColor } from './utils';
export { loadPrefs, getPref, setPref } from './prefs';
export { listTags, getPopularTags, createTag, updateTag, deleteTag, getVideoTags, addVideoTags, removeVideoTag } from './tags';
export type { Tag, TagListResponse, TagCreateRequest, TagUpdateRequest } from './tags';
export { getTrendingVideos, getSimilarVideos } from './recommendations';
export type { RecommendationItem, RecommendationResponse } from './recommendations';
export { listMyPlaylists, createPlaylist, updatePlaylist, deletePlaylist, addVideoToPlaylist, removeVideoFromPlaylist } from './playlists';
export type { Playlist, PlaylistListResponse } from './playlists';
export { listComments, listReplies, createComment, deleteComment } from './comments';
export type { Comment, CommentListResponse } from './comments';
export { createShareLink, getShareVideo, deleteShareLink, listMyShares, revokeMyShare } from './shares';
export type { ShareLink, ShareListItem, ShareVideoInfo } from './shares';
export { listUsers, deleteUser as adminDeleteUser, listAdminVideos, updateVideo as adminUpdateVideo, addExternalVideo, uploadCover as adminUploadCover, scanMedia, backfillThumbnails, getStats, batchUpdateCategory, resetUserPassword, toggleUserAdmin, approveUser, kickUser, getRegistrationEnabled, setRegistrationEnabled, getSystemInfo, isForbidden } from './admin';
export type { AdminUser, AdminVideo, AdminStats, SystemInfo } from './admin';
export { getLogs, clearLogs } from './logs';
export type { LogEntry, LogsResponse } from './logs';
