/**
 * 清理视频元素资源
 */
export function cleanupVideoElement(video: HTMLVideoElement | null): void {
  if (!video) return
  video.pause()
  video.removeAttribute('src')
  video.load()
  const srcObject = video.srcObject
  if (srcObject instanceof MediaStream) {
    for (const track of srcObject.getTracks()) {
      track.stop()
    }
  }
  video.srcObject = null
}

/**
 * 安全地获取视频时长
 */
export function safeGetDuration(video: HTMLVideoElement | null): number {
  if (!video || !isFinite(video.duration) || video.duration <= 0) return 0
  return video.duration
}
