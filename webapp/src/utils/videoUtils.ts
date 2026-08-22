/**
 * 清理视频元素资源
 */
export function cleanupVideoElement(video: HTMLVideoElement | null): void {
  if (!video) return
  video.pause()
  video.removeAttribute('src')
  video.load()
  if (video.srcObject instanceof MediaStream) {
    video.srcObject.getTracks().forEach(track => track.stop())
    video.srcObject = null
  }
}

/**
 * 安全地获取视频时长
 */
export function safeGetDuration(video: HTMLVideoElement | null): number {
  if (!video || !isFinite(video.duration) || video.duration <= 0) return 0
  return video.duration
}
