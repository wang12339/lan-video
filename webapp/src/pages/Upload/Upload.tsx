import { useState, useRef, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { checkSession } from '../../api'
import { getUploadStatus, uploadResumeChunk } from '../../api/videos'
import { useToast } from '../../components/Toast/Toast'
import './Upload.css'

const CATEGORIES = [
  { key: 'general', label: '其他', color: '#fff' },
  { key: '科技', label: '科技', color: '#3b82f6' },
  { key: '设计', label: '设计', color: '#ec4899' },
  { key: '音乐', label: '音乐', color: '#8b5cf6' },
  { key: '教程', label: '教程', color: '#10b981' },
  { key: '娱乐', label: '娱乐', color: '#f59e0b' },
  { key: '运动', label: '运动', color: '#ef4444' },
  { key: '记录', label: '记录', color: '#06b6d4' },
]

const BATCH_SIZE = 100
const CHUNK_SIZE = 5 * 1024 * 1024 // 5MB chunks for resume

interface UploadItem {
  file: File
  name: string
  size: number
  status: 'pending' | 'hashing' | 'uploading' | 'done' | 'error'
  progress: number
  errorMsg?: string
  category: string
  contentHash?: string
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / 1048576).toFixed(1) + ' MB'
}

async function computeContentHash(file: File): Promise<string> {
  const buf = await file.arrayBuffer()
  const hashBuf = await crypto.subtle.digest('SHA-256', buf)
  return Array.from(new Uint8Array(hashBuf)).map((b) => b.toString(16).padStart(2, '0')).join('')
}

export default function Upload() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [files, setFiles] = useState<UploadItem[]>([])
  const [category, setCategory] = useState('general')
  const [dragOver, setDragOver] = useState(false)
  const [uploading, setUploading] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const abortRef = useRef(false)

  useEffect(() => {
    if (!uploading) return
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [uploading])

  const addFiles = useCallback((newFiles: File[]) => {
    const valid = newFiles.filter((f) => {
      const isVideo = f.type.startsWith('video/')
      const isImage = f.type.startsWith('image/')
      if (!isVideo && !isImage) {
        toast(t('upload.invalidFormat', { name: f.name }), 'error')
        return false
      }
      if (isVideo && f.size > 5 * 10 ** 9) {
        toast(t('upload.tooLarge5GB', { name: f.name }), 'error')
        return false
      }
      if (isImage && f.size > 50 * 1048576) {
        toast(t('upload.tooLarge50MB', { name: f.name }), 'error')
        return false
      }
      return true
    })
    setFiles((prev) => [
      ...prev,
      ...valid.map((f) => ({
        file: f,
        name: f.name,
        size: f.size,
        status: 'pending' as const,
        progress: 0,
        category,
      })),
    ])
  }, [category, toast, t])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
    const dropped = Array.from(e.dataTransfer.files).filter((f) =>
      f.type.startsWith('video/') || f.type.startsWith('image/')
    )
    if (dropped.length > 0) addFiles(dropped)
  }, [addFiles])

  const handleFileInput = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      addFiles(Array.from(e.target.files))
    }
    e.target.value = ''
  }, [addFiles])

  const removeFile = useCallback((idx: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== idx))
  }, [])

  const clearAll = useCallback(() => {
    setFiles([])
  }, [])

  const uploadSingle = useCallback(async (item: UploadItem): Promise<boolean> => {
    const updateItem = (patch: Partial<UploadItem>) => {
      setFiles((prev) => prev.map((f) => f === item ? { ...f, ...patch } : f))
    }

    updateItem({ status: 'hashing', progress: 0 })

    let hash: string
    try {
      hash = await computeContentHash(item.file)
      updateItem({ contentHash: hash })
    } catch {
      updateItem({ status: 'error', errorMsg: '哈希计算失败' })
      return false
    }

    // Check if partial upload exists
    let startOffset = 0
    try {
      const status = await getUploadStatus(hash)
      if (status.received > 0 && status.received < item.file.size) {
        startOffset = status.received
      }
    } catch { /* no existing upload */ }

    updateItem({ status: 'uploading' })

    const totalChunks = Math.ceil(item.file.size / CHUNK_SIZE)
    const startChunk = Math.floor(startOffset / CHUNK_SIZE)

    for (let i = startChunk; i < totalChunks; i++) {
      if (abortRef.current) {
        updateItem({ status: 'error', errorMsg: '已取消' })
        return false
      }

      const start = i * CHUNK_SIZE
      const end = Math.min(start + CHUNK_SIZE, item.file.size)
      const chunk = item.file.slice(start, end)

      try {
        const result = await uploadResumeChunk(hash, item.file.name, item.file.size, item.category, chunk)
        updateItem({ progress: Math.round((result.received / item.file.size) * 100) })

        if (result.id) {
          updateItem({ status: 'done', progress: 100 })
          return true
        }
      } catch (e) {
        const msg = e instanceof Error ? e.message : '上传失败'
        if (msg.includes('重复')) {
          updateItem({ status: 'done', progress: 100, errorMsg: undefined })
          return true
        }
        updateItem({ status: 'error', errorMsg: msg })
        return false
      }
    }

    updateItem({ status: 'done', progress: 100 })
    return true
  }, [])

  const startUpload = useCallback(async () => {
    const pending = files.filter((f) => f.status === 'pending' || f.status === 'error')
    if (pending.length === 0) return
    if (!(await checkSession())) {
      toast(t('upload.loginRequired'), 'error')
      return
    }
    abortRef.current = false
    setUploading(true)

    for (let i = 0; i < pending.length; i += BATCH_SIZE) {
      const batch = pending.slice(i, i + BATCH_SIZE)
      const batchNum = Math.floor(i / BATCH_SIZE) + 1
      const totalBatches = Math.ceil(pending.length / BATCH_SIZE)
      if (totalBatches > 1) {
        console.log(`[上传] 第 ${batchNum}/${totalBatches} 批 (${batch.length} 个文件)`)
      }
      for (const item of batch) {
        if (abortRef.current) break
        await uploadSingle(item)
      }
    }

    setUploading(false)
  }, [files, uploadSingle])

  const cancelUpload = useCallback(() => {
    abortRef.current = true
  }, [])

  const pendingCount = files.filter((f) => f.status === 'pending' || f.status === 'error').length
  const batchCount = Math.ceil(pendingCount / BATCH_SIZE)

  const getStatusText = (item: UploadItem): string => {
    switch (item.status) {
      case 'pending': return t('upload.pending')
      case 'hashing': return t('upload.hashing')
      case 'uploading': return `${t('upload.uploading')} ${item.progress}%`
      case 'done': return `✅ ${t('upload.success')}`
      case 'error': return '❌ ' + (item.errorMsg || t('upload.failed'))
    }
  }

  const getStatusClass = (item: UploadItem): string => {
    if (item.status === 'done') return 'done'
    if (item.status === 'error') return 'error'
    return ''
  }

  return (
    <div className="upload-page">
      <div className="upload-header">
        <h1>{t('upload.title')}</h1>
        <p>支持视频（MP4、MOV 等）和图片（JPG、PNG 等），视频最大 50GB，图片最大 50MB</p>
      </div>

      <div
        className={`upload-dropzone ${dragOver ? 'drag-over' : ''} ${files.length > 0 ? 'has-files' : ''}`}
        onDragOver={(e) => { e.preventDefault(); setDragOver(true) }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
      >
        <div className="dropzone-icon">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
        </div>
        <p className="dropzone-text">{t('upload.orDrag')}</p>
        <p className="dropzone-sub">或点击下方按钮选择文件</p>
        <input
          ref={fileInputRef}
          type="file"
          accept="video/*,image/*"
          hidden
          multiple
          onChange={handleFileInput}
        />
      </div>

      <div className="upload-select-btns">
        <button
          className="upload-select-btn"
          onClick={() => fileInputRef.current?.click()}
        >
          {t('upload.select')}
        </button>
      </div>

      {files.length > 0 && (
        <>
          <div className="upload-cats">
            <span className="upload-cats-label">{t('upload.category')}</span>
            {CATEGORIES.map((cat) => (
              <button
                key={cat.key}
                className={`cat-dot ${category === cat.key ? 'active' : ''}`}
                onClick={() => setCategory(cat.key)}
              >
                <span className="dot" style={{ background: cat.color }} />
                {cat.label}
              </button>
            ))}
          </div>

          <div className="upload-queue">
            {files.map((item, idx) => (
              <div key={item.file.name + item.file.size + item.file.lastModified} className="upload-item">
                <div className="upload-item-icon">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                    <polygon points="23 7 16 12 23 17 23 7" />
                    <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                  </svg>
                </div>
                <div className="upload-item-info">
                  <div className="upload-item-name">{item.name}</div>
                  <div className="upload-item-size">{formatSize(item.size)}</div>
                  <div className="upload-item-status">
                    <div className="progress-bar">
                      <div className="progress-fill" style={{ width: item.progress + '%' }} />
                    </div>
                    <span className={`status-text ${getStatusClass(item)}`}>
                      {getStatusText(item)}
                    </span>
                  </div>
                </div>
                {(item.status === 'pending' || item.status === 'error') && (
                  <button className="upload-item-remove" onClick={() => removeFile(idx)} title="移除">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                      <line x1="18" y1="6" x2="6" y2="18" />
                      <line x1="6" y1="6" x2="18" y2="18" />
                    </svg>
                  </button>
                )}
              </div>
            ))}
          </div>

          <div className="upload-actions">
            {uploading ? (
              <button className="upload-cancel-btn" onClick={cancelUpload}>
                {t('common.cancel')}
              </button>
            ) : (
              <button
                className="upload-start-btn"
                onClick={startUpload}
                disabled={pendingCount === 0}
              >
                {batchCount > 1 ? t('upload.batchInfo', { batchCount, pendingCount }) : t('upload.pendingInfo', { pendingCount })}
              </button>
            )}
            <button className="upload-clear-btn" onClick={clearAll}>
              {t('upload.clearAll')}
            </button>
          </div>
        </>
      )}
    </div>
  )
}
