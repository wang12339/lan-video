import { useState, useRef, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { checkSession } from '../../../api'
import { useToast } from '../../../components/Toast/Toast'
import i18n from '../../../i18n'
import {
  UploadItem, isSupportedFile, formatSize,
  CONCURRENT_UPLOADS, VIDEO_MAX_SIZE, IMAGE_MAX_SIZE,
} from './useFileHash'
import { CATEGORIES, runPool } from './uploadHelpers'
import { uploadSingleFile } from './uploadSingleWorker'

export { formatSize }
export type { UploadItem }
export { CATEGORIES }

const MAX_TOTAL_FILES = 50
const MAX_TOTAL_SIZE = 200 * 1024 * 1024 * 1024
const PENDING_STORAGE_KEY = 'atmos_upload_pending_v1'

/** 未完成上传的元数据（File 对象无法持久化，刷新后需用户重新选择文件）。 */
interface PendingUploadRecord {
  name: string
  size: number
  lastModified: number
  category: string
}

/* eslint-disable no-control-regex */
function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"|?*\x00-\x1f]/g, '_').replace(/\.+/g, '.').replace(/^\.+/, '').slice(0, 255)
}
/* eslint-enable no-control-regex */

export function useUploadManager() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [files, setFiles] = useState<UploadItem[]>([])
  const [category, setCategory] = useState('all')
  const [dragOver, setDragOver] = useState(false)
  const [uploading, setUploading] = useState(false)
  const [resumeNotice, setResumeNotice] = useState(0)
  const abortRef = useRef(false)
  const mountedRef = useRef(true)
  const dragDepthRef = useRef(0)
  const filesRef = useRef<UploadItem[]>([])
  filesRef.current = files

  // 刷新/重开页面后提示上次未完成的上传；服务端仍保有断点进度，
  // 用户重新选择相同文件即可自动续传。
  useEffect(() => {
    try {
      const raw = sessionStorage.getItem(PENDING_STORAGE_KEY)
      if (!raw) return
      const parsed = JSON.parse(raw) as unknown
      if (Array.isArray(parsed) && parsed.length > 0) setResumeNotice(parsed.length)
    } catch { /* ignore */ }
  }, [])

  useEffect(() => {
    try {
      const pending: PendingUploadRecord[] = files
        .filter((f) => f.status !== 'done')
        .map((f) => ({
          name: f.name,
          size: f.size,
          lastModified: f.file.lastModified,
          category: f.category,
        }))
      if (pending.length === 0) sessionStorage.removeItem(PENDING_STORAGE_KEY)
      else sessionStorage.setItem(PENDING_STORAGE_KEY, JSON.stringify(pending))
    } catch { /* ignore */ }
  }, [files])

  const dismissResumeNotice = useCallback(() => {
    setResumeNotice(0)
    try {
      sessionStorage.removeItem(PENDING_STORAGE_KEY)
    } catch { /* ignore */ }
  }, [])

  useEffect(() => {
    if (!uploading) return
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [uploading])

  useEffect(() => {
    const prevent = (e: DragEvent) => e.preventDefault()
    window.addEventListener('dragover', prevent)
    window.addEventListener('drop', prevent)
    return () => {
      window.removeEventListener('dragover', prevent)
      window.removeEventListener('drop', prevent)
    }
  }, [])

  // 卸载时中止仍在进行的分片/并发循环（下一个检查点生效），并标记组件已卸载，
  // 避免后续 setState/toast 泄漏到其它页面。startUpload 开始时仍会重新置 false。
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      abortRef.current = true
    }
  }, [])

  const addFiles = useCallback((newFiles: File[]) => {
    const added: UploadItem[] = []
    const currentTotalSize = filesRef.current.reduce((sum, f) => sum + f.size, 0)
    for (const f of newFiles) {
      if (filesRef.current.length + added.length >= MAX_TOTAL_FILES) {
        toast(t('upload.tooManyFiles', { max: MAX_TOTAL_FILES }), 'error')
        break
      }
      const { ok, kind } = isSupportedFile(f)
      if (!ok) {
        toast(t('upload.invalidFormat', { name: f.name }), 'error')
        continue
      }
      if (f.size === 0) {
        toast(i18n.t('upload.emptyFile', { name: f.name }), 'error')
        continue
      }
      if (kind === 'video' && f.size > VIDEO_MAX_SIZE) {
        toast(t('upload.tooLarge5GB', { name: f.name }), 'error')
        continue
      }
      if (kind === 'image' && f.size > IMAGE_MAX_SIZE) {
        toast(t('upload.tooLarge50MB', { name: f.name }), 'error')
        continue
      }
      if (currentTotalSize + added.reduce((s, a) => s + a.size, 0) + f.size > MAX_TOTAL_SIZE) {
        toast(t('upload.totalSizeExceeded'), 'error')
        break
      }
      added.push({
        uid: `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 9)}`,
        file: f,
        name: sanitizeFilename(f.name),
        size: f.size,
        status: 'pending' as const,
        progress: 0,
        category,
      })
    }
    if (added.length > 0) {
      setResumeNotice(0)
      setFiles((prev) => {
        const existing = new Set(prev.map((f) => `${f.name}|${f.size}|${f.file.lastModified}`))
        const deduped = added.filter((item) => {
          const key = `${item.name}|${item.size}|${item.file.lastModified}`
          if (existing.has(key)) {
            toast(i18n.t('upload.alreadyInList', { name: item.name }), 'info')
            return false
          }
          existing.add(key)
          return true
        })
        return deduped.length > 0 ? [...prev, ...deduped] : prev
      })
    }
  }, [category, toast, t])

  // 分片循环可能在卸载后才走到检查点，此时丢弃状态更新，避免对已卸载组件 setState。
  const setFilesSafe = useCallback((updater: (prev: UploadItem[]) => UploadItem[]) => {
    if (mountedRef.current) setFiles(updater)
  }, [])

  const startUpload = useCallback(async () => {
    if (!(await checkSession())) {
      if (mountedRef.current) toast(t('upload.loginRequired'), 'error')
      return
    }
    if (!mountedRef.current) return
    const targets = filesRef.current.filter((f) => f.status === 'pending' || f.status === 'error')
    if (targets.length === 0) return
    abortRef.current = false
    setUploading(true)

    let okCount = 0
    try {
      await runPool(targets, CONCURRENT_UPLOADS, async (item) => {
        if (abortRef.current) return
        if (await uploadSingleFile(item, setFilesSafe, abortRef)) okCount++
      })
    } finally {
      if (mountedRef.current) setUploading(false)
    }

    // 组件已卸载：中止属于生命周期清理，不应把“用户取消”toast 带到其它页面。
    if (!mountedRef.current) return
    if (abortRef.current) {
      toast(i18n.t('upload.cancelledToast'), 'info')
    } else if (okCount === targets.length) {
      toast(i18n.t('upload.successCount', { count: okCount }), 'success')
    } else if (okCount > 0) {
      toast(i18n.t('upload.partialSuccess', { ok: okCount, total: targets.length }), 'error')
    } else {
      toast(i18n.t('upload.uploadFailedRetry'), 'error')
    }
  }, [setFilesSafe, toast, t])

  const cancelUpload = useCallback(() => {
    abortRef.current = true
  }, [])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    dragDepthRef.current = 0
    setDragOver(false)
    if (uploading) {
      toast(i18n.t('upload.uploadBusy'), 'info')
      return
    }
    addFiles(Array.from(e.dataTransfer.files))
  }, [addFiles, uploading, toast])

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    dragDepthRef.current++
    setDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1)
    if (dragDepthRef.current === 0) setDragOver(false)
  }, [])

  return {
    files, setFiles, category, setCategory,
    dragOver, uploading, addFiles,
    startUpload, cancelUpload,
    handleDrop, handleDragEnter, handleDragLeave,
    resumeNotice, dismissResumeNotice,
  }
}
