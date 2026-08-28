import type { MutableRefObject } from 'react'
import { getUploadStatus, uploadResumeChunk } from '../../../api/videos'
import i18n from '../../../i18n'
import {
  UploadItem, CancelledError, computeContentHash,
  CHUNK_SIZE, MAX_CHUNK_RETRIES, RETRY_BASE_DELAY_MS,
} from './useFileHash'
import { delay, isRetryable } from './uploadHelpers'

export async function uploadSingleFile(
  item: UploadItem,
  setFiles: (fn: (prev: UploadItem[]) => UploadItem[]) => void,
  abortRef: MutableRefObject<boolean>,
): Promise<boolean> {
  const updateItem = (patch: Partial<UploadItem>) => {
    setFiles((prev) => prev.map((f) => (f === item ? { ...f, ...patch } : f)))
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

  let startOffset = 0
  try {
    const status = await getUploadStatus(hash)
    if (status.received >= item.file.size) {
      updateItem({ status: 'done', progress: 100 })
      return true
    }
    if (status.received > 0) startOffset = status.received
  } catch { /* no record */ }

  updateItem({ status: 'uploading' })

  const totalChunks = Math.ceil(item.file.size / CHUNK_SIZE)
  const startChunk = Math.floor(startOffset / CHUNK_SIZE)

  for (let i = startChunk; i < totalChunks; i++) {
    if (abortRef.current) {
      updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
      return false
    }
    const chunk = item.file.slice(i * CHUNK_SIZE, Math.min((i + 1) * CHUNK_SIZE, item.file.size))

    let lastErr: unknown = null
    let uploaded = false
    for (let attempt = 0; attempt <= MAX_CHUNK_RETRIES; attempt++) {
      if (abortRef.current) {
        updateItem({ status: 'error', errorMsg: i18n.t('upload.cancelled') })
        return false
      }
      try {
        const result = await uploadResumeChunk(hash, item.file.name, item.file.size, item.category, chunk)
        if (result.id) {
          updateItem({ status: 'done', progress: 100, videoId: result.id })
          return true
        }
        updateItem({ progress: Math.round((result.received / item.file.size) * 100) })
        uploaded = true
        break
      } catch (e) {
        const msg = e instanceof Error ? e.message : i18n.t('upload.uploadFailed')
        if (msg.includes('duplicate') || msg.includes('already exists')) {
          updateItem({ status: 'done', progress: 100, errorMsg: undefined })
          return true
        }
        lastErr = e
        if (!isRetryable(e)) break
        await delay(RETRY_BASE_DELAY_MS * 2 ** attempt)
      }
    }
    if (!uploaded) {
      updateItem({
        status: 'error',
        errorMsg: lastErr instanceof Error ? lastErr.message : i18n.t('upload.uploadFailed'),
      })
      return false
    }
  }

  updateItem({ status: 'done', progress: 100 })
  return true
}
