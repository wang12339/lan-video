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

function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"|?*\x00-\x1f]/g, '_').replace(/\.+/g, '.').replace(/^\.+/, '').slice(0, 255)
}

export function useUploadManager() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [files, setFiles] = useState<UploadItem[]>([])
  const [category, setCategory] = useState('all')
  const [dragOver, setDragOver] = useState(false)
  const [uploading, setUploading] = useState(false)
  const abortRef = useRef(false)
  const dragDepthRef = useRef(0)
  const filesRef = useRef<UploadItem[]>([])
  filesRef.current = files

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
        file: f,
        name: sanitizeFilename(f.name),
        size: f.size,
        status: 'pending' as const,
        progress: 0,
        category,
      })
    }
    if (added.length > 0) {
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

  const startUpload = useCallback(async () => {
    if (!(await checkSession())) {
      toast(t('upload.loginRequired'), 'error')
      return
    }
    const targets = filesRef.current.filter((f) => f.status === 'pending' || f.status === 'error')
    if (targets.length === 0) return
    abortRef.current = false
    setUploading(true)

    let okCount = 0
    try {
      await runPool(targets, CONCURRENT_UPLOADS, async (item) => {
        if (abortRef.current) return
        if (await uploadSingleFile(item, setFiles, abortRef)) okCount++
      })
    } finally {
      setUploading(false)
    }

    if (abortRef.current) {
      toast(i18n.t('upload.cancelledToast'), 'info')
    } else if (okCount === targets.length) {
      toast(i18n.t('upload.successCount', { count: okCount }), 'success')
    } else if (okCount > 0) {
      toast(i18n.t('upload.partialSuccess', { ok: okCount, total: targets.length }), 'error')
    } else {
      toast(i18n.t('upload.uploadFailedRetry'), 'error')
    }
  }, [toast, t])

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
  }
}
