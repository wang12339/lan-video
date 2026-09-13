import type { MutableRefObject } from 'react'
import { APIError } from '../../../api'
import { getUploadStatus, uploadResumeChunk } from '../../../api/videos'
import i18n from '../../../i18n'
import {
  UploadItem, CancelledError, computeContentHash,
  CHUNK_SIZE, MIN_CHUNK_SIZE, MAX_CHUNK_SIZE,
  MAX_CHUNK_RETRIES, RETRY_BASE_DELAY_MS,
  SLOW_CHUNK_MS, FAST_CHUNK_MS,
} from './useFileHash'
import { delay, isRetryable } from './uploadHelpers'

interface UploadErrorInfo {
  code?: string
  status?: number
  /** offset_mismatch 时服务端已接收的字节数 */
  received?: number
}

function getUploadErrorInfo(e: unknown): UploadErrorInfo {
  if (!(e instanceof APIError)) return {}
  const data = e.data as { received?: unknown } | undefined
  const received =
    data && typeof data.received === 'number' && Number.isFinite(data.received)
      ? data.received
      : undefined
  return { code: e.code, status: e.status, received }
}

/** 旧服务端（无 code）兼容：按文案识别重复/配额。 */
function isDuplicateMessage(msg: string): boolean {
  return (
    !msg.includes('配额') &&
    (msg.includes('重复') || msg.includes('已存在') ||
     msg.includes('duplicate') || msg.includes('already exists'))
  )
}

/** 命中去重后补查一次状态，尽量拿回已有视频 ID 供“查看”跳转。 */
async function resolveExistingId(
  hash: string,
  size: number,
): Promise<number | string | undefined> {
  try {
    const status = await getUploadStatus(hash, size)
    return status.existing_id
  } catch {
    return undefined
  }
}

const clamp = (n: number, max: number) => Math.max(0, Math.min(n, max))

export async function uploadSingleFile(
  item: UploadItem,
  setFiles: (fn: (prev: UploadItem[]) => UploadItem[]) => void,
  abortRef: MutableRefObject<boolean>
): Promise<boolean> {
  const updateItem = (patch: Partial<UploadItem>) => {
    setFiles((prev) => prev.map((f) => (f.uid === item.uid ? { ...f, ...patch } : f)))
  }
  updateItem({ status: 'hashing', progress: 0, errorMsg: undefined })

  let hash: string
  try {
    hash = await computeContentHash(item.file, () => abortRef.current)
    updateItem({ contentHash: hash })
  } catch (e) {
    if (e instanceof CancelledError) {
      updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
    } else {
      updateItem({ status: 'error', errorMsg: i18n.t('upload.hashFailed') })
    }
    return false
  }
  if (abortRef.current) {
    updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
    return false
  }

  // 预检：重复（可零传输跳过）/ 配额（传输前失败）/ 断点位置
  let offset = 0
  try {
    const status = await getUploadStatus(hash, item.file.size)
    if (status.exists) {
      updateItem({
        status: 'done',
        progress: 100,
        videoId: status.existing_id ?? item.videoId,
      })
      return true
    }
    if (typeof status.received === 'number' && status.received > 0) {
      offset = clamp(status.received, item.file.size)
    }
  } catch (e) {
    const info = getUploadErrorInfo(e)
    const msg = e instanceof Error ? e.message : ''
    if (info.code === 'quota_exceeded' || (!info.code && msg.includes('配额'))) {
      updateItem({ status: 'error', errorMsg: i18n.t('upload.quotaExceeded') })
      return false
    }
    // 其他预检失败不阻断：从已确认的偏移（0）开始，finalize 仍有权威去重兜底
  }

  updateItem({
    status: 'uploading',
    progress: Math.round((offset / item.file.size) * 100),
  })

  // 按 offset 驱动的分片循环（不再按分片序号推进）：服务端返回的 received
  // 是唯一权威偏移，超时重试/偏移漂移都能安全收敛。分片大小自适应。
  let chunkSize = Math.max(MIN_CHUNK_SIZE, Math.min(CHUNK_SIZE, item.file.size))
  let consecutiveMismatches = 0

  while (offset < item.file.size) {
    if (abortRef.current) {
      updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
      return false
    }
    const end = Math.min(offset + chunkSize, item.file.size)
    const chunk = item.file.slice(offset, end)
    const startedAt = Date.now()
    let advanced = false
    let lastErr: unknown = null

    for (let attempt = 0; attempt <= MAX_CHUNK_RETRIES; attempt++) {
      if (abortRef.current) {
        updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
        return false
      }
      try {
        const result = await uploadResumeChunk(
          hash, item.file.name, item.file.size, item.category, chunk, offset
        )
        if (result.id !== undefined) {
          updateItem({ status: 'done', progress: 100, videoId: result.id })
          return true
        }
        const received = typeof result.received === 'number'
          ? clamp(result.received, item.file.size)
          : end
        if (received <= offset && received < item.file.size) {
          // 服务端未前进：当作可重试失败处理，避免死循环
          throw new Error(i18n.t('upload.uploadFailed'))
        }
        offset = received
        consecutiveMismatches = 0
        updateItem({ progress: Math.round((offset / item.file.size) * 100) })
        advanced = true
        break
      } catch (e) {
        const info = getUploadErrorInfo(e)
        const msg = e instanceof Error ? e.message : ''

        if (info.code === 'duplicate' || (!info.code && isDuplicateMessage(msg))) {
          // 服务端去重命中（同账号已传过同一文件）→ 视为成功
          updateItem({
            status: 'done',
            progress: 100,
            errorMsg: undefined,
            videoId: item.videoId ?? (await resolveExistingId(hash, item.file.size)),
          })
          return true
        }
        if (info.code === 'hash_mismatch') {
          updateItem({ status: 'error', errorMsg: i18n.t('upload.hashMismatch') })
          return false
        }
        if (info.code === 'quota_exceeded') {
          updateItem({ status: 'error', errorMsg: i18n.t('upload.quotaExceeded') })
          return false
        }
        if (info.code === 'offset_mismatch') {
          // 服务端偏移权威：按返回的 received 重新切片。不消耗重试次数，
          // 但连续多次仍无法前进则中止，避免死循环。
          consecutiveMismatches++
          if (consecutiveMismatches > MAX_CHUNK_RETRIES + 2) {
            updateItem({ status: 'error', errorMsg: i18n.t('upload.uploadFailed') })
            return false
          }
          offset = clamp(info.received ?? 0, item.file.size)
          updateItem({ progress: Math.round((offset / item.file.size) * 100) })
          advanced = true
          break
        }
        lastErr = e
        if (!isRetryable(e)) break
        await delay(RETRY_BASE_DELAY_MS * 2 ** attempt)
      }
    }

    if (!advanced) {
      updateItem({
        status: 'error',
        errorMsg: lastErr instanceof Error ? lastErr.message : i18n.t('upload.uploadFailed'),
      })
      return false
    }

    // 自适应分片：慢则减半、快则加倍（降低慢网络下超时/重试概率）
    const elapsed = Date.now() - startedAt
    if (elapsed > SLOW_CHUNK_MS && chunkSize > MIN_CHUNK_SIZE) {
      chunkSize = Math.max(MIN_CHUNK_SIZE, Math.floor(chunkSize / 2))
    } else if (elapsed < FAST_CHUNK_MS && chunkSize < MAX_CHUNK_SIZE) {
      chunkSize = Math.min(MAX_CHUNK_SIZE, chunkSize * 2)
    }
  }

  // 字节已全部接收但服务端未返回 id（临时文件完整但 finalize 未执行，
  // 例如上次进程在 finalize 前崩溃）→ 空 body + offset=size 触发恢复。
  try {
    const fin = await uploadResumeChunk(
      hash, item.file.name, item.file.size, item.category, new Blob([]), item.file.size
    )
    if (fin.id !== undefined) {
      updateItem({ status: 'done', progress: 100, videoId: fin.id })
      return true
    }
  } catch (e) {
    const info = getUploadErrorInfo(e)
    if (info.code === 'duplicate') {
      updateItem({
        status: 'done',
        progress: 100,
        videoId: item.videoId ?? (await resolveExistingId(hash, item.file.size)),
      })
      return true
    }
    const msg = e instanceof Error ? e.message : ''
    if (!info.code && isDuplicateMessage(msg)) {
      updateItem({
        status: 'done',
        progress: 100,
        videoId: item.videoId ?? (await resolveExistingId(hash, item.file.size)),
      })
      return true
    }
    updateItem({ status: 'error', errorMsg: i18n.t('upload.uploadFailed') })
    return false
  }

  // 未返回 id：服务端 offsets 与本地不一致（异常状态），提示重试而不是假成功
  updateItem({ status: 'error', errorMsg: i18n.t('upload.uploadFailed') })
  return false
}
