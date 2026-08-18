import { useState, useRef, useCallback, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { checkSession, APIError } from '../../api'
import { getUploadStatus, uploadResumeChunk } from '../../api/videos'
import { useToast } from '../../components/Toast/Toast'
import i18n from '../../i18n'
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

const CHUNK_SIZE = 5 * 1024 * 1024 // 5MB chunks for resume
// 与后端 admin_video.rs 的 MAX_UPLOAD_SIZE (50GB) 对齐
const VIDEO_MAX_SIZE = 50 * 1024 * 1024 * 1024
const IMAGE_MAX_SIZE = 50 * 1024 * 1024
const MAX_CHUNK_RETRIES = 3
const RETRY_BASE_DELAY_MS = 800
const CONCURRENT_UPLOADS = 2 // 并发文件数；单个文件的分片必须串行（后端按追加顺序写盘）
const SMALL_FILE_BYTES = 4 * 1024 * 1024
const HASH_SLICE_BYTES = 8 * 1024 * 1024

// 与后端 media_service.rs 的扩展名白名单对齐（部分格式 MIME 为空，需按扩展名兜底）
const VIDEO_EXTS = ['mp4', 'm4v', 'm3u8', 'mov', 'avi', 'mkv', 'webm', 'flv', 'wmv']
const IMAGE_EXTS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp']

interface UploadItem {
  file: File
  name: string
  size: number
  status: 'pending' | 'hashing' | 'uploading' | 'done' | 'error'
  progress: number
  errorMsg?: string
  category: string
  contentHash?: string
  videoId?: string
}

class CancelledError extends Error {
  constructor() {
    super('已取消')
    this.name = 'CancelledError'
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let i = 0
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024
    i++
  }
  return value.toFixed(1) + ' ' + (units[i] ?? 'KB')
}

function isSupportedFile(f: File): { ok: boolean; kind: 'video' | 'image' | 'other' } {
  const type = f.type.toLowerCase()
  if (type.startsWith('video/')) return { ok: true, kind: 'video' }
  if (type.startsWith('image/')) return { ok: true, kind: 'image' }
  const ext = (f.name.split('.').pop() ?? '').toLowerCase()
  if (VIDEO_EXTS.includes(ext)) return { ok: true, kind: 'video' }
  if (IMAGE_EXTS.includes(ext)) return { ok: true, kind: 'image' }
  return { ok: false, kind: 'other' }
}

// ---- 增量 SHA-256：大文件（>4MB）流式分片计算，避免整文件读入内存 ----

const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
])

const SHA256_INIT = new Uint32Array([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
])

function rotr(x: number, n: number): number {
  return (x >>> n) | (x << (32 - n))
}

class Sha256 {
  private h: Uint32Array
  private buffer = new Uint8Array(64)
  private bufferLen = 0
  private totalBytes = 0

  constructor() {
    this.h = new Uint32Array(SHA256_INIT)
  }

  update(data: Uint8Array): void {
    this.totalBytes += data.length
    let pos = 0
    if (this.bufferLen > 0) {
      const need = 64 - this.bufferLen
      const take = Math.min(need, data.length)
      this.buffer.set(data.subarray(0, take), this.bufferLen)
      this.bufferLen += take
      pos = take
      if (this.bufferLen === 64) {
        this.compress(this.buffer)
        this.bufferLen = 0
      }
    }
    while (pos + 64 <= data.length) {
      this.compress(data.subarray(pos, pos + 64))
      pos += 64
    }
    if (pos < data.length) {
      this.buffer.set(data.subarray(pos))
      this.bufferLen = data.length - pos
    }
  }

  private compress(block: Uint8Array): void {
    const w = new Uint32Array(64)
    for (let i = 0; i < 16; i++) {
      const o = i * 4
      w[i] = (block[o]! << 24) | (block[o + 1]! << 16) | (block[o + 2]! << 8) | block[o + 3]!
    }
    for (let i = 16; i < 64; i++) {
      const w15 = w[i - 15]!
      const w2 = w[i - 2]!
      const s0 = rotr(w15, 7) ^ rotr(w15, 18) ^ (w15 >>> 3)
      const s1 = rotr(w2, 17) ^ rotr(w2, 19) ^ (w2 >>> 10)
      w[i] = (w[i - 16]! + s0 + w[i - 7]! + s1) | 0
    }
    const h = this.h
    let a = h[0]!
    let b = h[1]!
    let c = h[2]!
    let d = h[3]!
    let e = h[4]!
    let f = h[5]!
    let g = h[6]!
    let hh = h[7]!
    for (let i = 0; i < 64; i++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
      const ch = (e & f) ^ (~e & g)
      const temp1 = (hh + S1 + ch + SHA256_K[i]! + w[i]!) | 0
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
      const maj = (a & b) ^ (a & c) ^ (b & c)
      const temp2 = (S0 + maj) | 0
      hh = g
      g = f
      f = e
      e = (d + temp1) | 0
      d = c
      c = b
      b = a
      a = (temp1 + temp2) | 0
    }
    h[0] = (h[0]! + a) | 0
    h[1] = (h[1]! + b) | 0
    h[2] = (h[2]! + c) | 0
    h[3] = (h[3]! + d) | 0
    h[4] = (h[4]! + e) | 0
    h[5] = (h[5]! + f) | 0
    h[6] = (h[6]! + g) | 0
    h[7] = (h[7]! + hh) | 0
  }

  digest(): string {
    const bitLen = this.totalBytes * 8 // 50GB 内为精确整数（< 2^53）
    const hi = Math.floor(bitLen / 0x100000000)
    const lo = bitLen % 0x100000000
    const padLen = this.bufferLen < 56 ? 64 - this.bufferLen : 128 - this.bufferLen
    const padded = new Uint8Array(padLen)
    padded[0] = 0x80
    padded[padLen - 8] = (hi >>> 24) & 0xff
    padded[padLen - 7] = (hi >>> 16) & 0xff
    padded[padLen - 6] = (hi >>> 8) & 0xff
    padded[padLen - 5] = hi & 0xff
    padded[padLen - 4] = (lo >>> 24) & 0xff
    padded[padLen - 3] = (lo >>> 16) & 0xff
    padded[padLen - 2] = (lo >>> 8) & 0xff
    padded[padLen - 1] = lo & 0xff
    this.update(padded)
    let out = ''
    for (let i = 0; i < 8; i++) {
      out += this.h[i]!.toString(16).padStart(8, '0')
    }
    return out
  }
}

// 流式实现自检（已知向量 SHA-256("abc")），防止实现错误静默产生错误哈希
const SHA256_ABC = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
let streamingHashOk: boolean | null = null

function verifyStreamingHash(): boolean {
  if (streamingHashOk === null) {
    const hasher = new Sha256()
    hasher.update(new TextEncoder().encode('abc'))
    streamingHashOk = hasher.digest() === SHA256_ABC
  }
  return streamingHashOk
}

function hexFromBuffer(buf: ArrayBuffer): string {
  return Array.from(new Uint8Array(buf)).map((b) => b.toString(16).padStart(2, '0')).join('')
}

async function computeContentHash(file: File, isCancelled: () => boolean): Promise<string> {
  const subtle = crypto.subtle
  if (subtle && (file.size <= SMALL_FILE_BYTES || !verifyStreamingHash())) {
    // 小文件直接一次性读入；流式实现不可用时兜底（大文件会占内存，但仅异常路径）
    return hexFromBuffer(await subtle.digest('SHA-256', await file.arrayBuffer()))
  }
  if (!verifyStreamingHash()) {
    throw new Error('当前环境不支持文件哈希计算')
  }
  const hasher = new Sha256()
  for (let offset = 0; offset < file.size; offset += HASH_SLICE_BYTES) {
    if (isCancelled()) throw new CancelledError()
    const buf = await file.slice(offset, Math.min(offset + HASH_SLICE_BYTES, file.size)).arrayBuffer()
    hasher.update(new Uint8Array(buf))
  }
  return hasher.digest()
}

// ---- 通用并发池（文件级并发；分片仍按文件串行） ----

async function runPool<T>(items: T[], limit: number, worker: (item: T) => Promise<void>): Promise<void> {
  const queue = [...items]
  const runners = Array.from({ length: Math.min(limit, queue.length) }, async () => {
    while (queue.length > 0) {
      const item = queue.shift()
      if (item === undefined) return
      await worker(item)
    }
  })
  await Promise.all(runners)
}

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

function isRetryable(e: unknown): boolean {
  if (e instanceof APIError) {
    return e.status === 0 || e.status === 429 || e.status >= 500
  }
  return e instanceof TypeError // 网络层失败
}

export default function Upload() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const navigate = useNavigate()
  const [files, setFiles] = useState<UploadItem[]>([])
  const [category, setCategory] = useState('general')
  const [dragOver, setDragOver] = useState(false)
  const [uploading, setUploading] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const abortRef = useRef(false)
  const dragDepthRef = useRef(0)

  useEffect(() => {
    if (!uploading) return
    const handler = (e: BeforeUnloadEvent) => {
      e.preventDefault()
      e.returnValue = ''
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [uploading])

  // 防止把文件拖到页面其他位置时浏览器直接打开文件
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
    const existing = new Set(
      files.map((f) => `${f.name}|${f.size}|${f.file.lastModified}`)
    )
    const added: UploadItem[] = []
    for (const f of newFiles) {
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
      const key = `${f.name}|${f.size}|${f.lastModified}`
      if (existing.has(key)) {
        toast(i18n.t('upload.alreadyInList', { name: f.name }), 'info')
        continue
      }
      existing.add(key)
      added.push({
        file: f,
        name: f.name,
        size: f.size,
        status: 'pending' as const,
        progress: 0,
        category,
      })
    }
    if (added.length > 0) {
      setFiles((prev) => [...prev, ...added])
    }
  }, [category, files, toast, t])

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

  const handleFileInput = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      addFiles(Array.from(e.target.files))
    }
    e.target.value = ''
  }, [addFiles])

  const openFilePicker = useCallback(() => {
    if (uploading) {
      toast(i18n.t('upload.uploadBusy'), 'info')
      return
    }
    fileInputRef.current?.click()
  }, [uploading, toast])

  const removeFile = useCallback((idx: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== idx))
  }, [])

  const retryItem = useCallback((idx: number) => {
    setFiles((prev) =>
      prev.map((f, i) =>
        i === idx ? { ...f, status: 'pending' as const, progress: 0, errorMsg: undefined } : f
      )
    )
  }, [])

  const clearAll = useCallback(() => {
    setFiles([])
  }, [])

  const uploadSingle = useCallback(async (item: UploadItem): Promise<boolean> => {
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

    // 查询服务端已接收字节数，支持断点续传
    let startOffset = 0
    try {
      const status = await getUploadStatus(hash)
      if (status.received >= item.file.size) {
        // 之前已完整上传（可能响应丢失），直接视为成功
        updateItem({ status: 'done', progress: 100 })
        return true
      }
      if (status.received > 0) startOffset = status.received
    } catch { /* 服务端无记录，从头开始 */ }

    updateItem({ status: 'uploading' })

    const totalChunks = Math.ceil(item.file.size / CHUNK_SIZE)
    const startChunk = Math.floor(startOffset / CHUNK_SIZE)

    // 注意：后端 upload_resume 是顺序追加写盘，分片必须串行发送，
    // 不能并发分片（并发会导致文件错乱）。
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
          if (msg.includes('重复')) {
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
  }, [])

  const startUpload = useCallback(async () => {
    const targets = files.filter((f) => f.status === 'pending' || f.status === 'error')
    if (targets.length === 0) return
    if (!(await checkSession())) {
      toast(t('upload.loginRequired'), 'error')
      return
    }
    abortRef.current = false
    setUploading(true)

    let okCount = 0
    try {
      await runPool(targets, CONCURRENT_UPLOADS, async (item) => {
        if (abortRef.current) return
        if (await uploadSingle(item)) okCount++
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
  }, [files, uploadSingle, toast, t])

  const cancelUpload = useCallback(() => {
    abortRef.current = true
  }, [])

  const pendingCount = files.filter((f) => f.status === 'pending' || f.status === 'error').length

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

  const catEditable = (item: UploadItem) =>
    !uploading && (item.status === 'pending' || item.status === 'error')

  return (
    <div className="upload-page">
      <div className="upload-header">
        <h1>{t('upload.title')}</h1>
        <p>{i18n.t('upload.formatHint')}</p>
      </div>

      <div
        className={`upload-dropzone ${dragOver ? 'drag-over' : ''} ${files.length > 0 ? 'has-files' : ''} ${uploading ? 'disabled' : ''}`}
        role="button"
        tabIndex={0}
        aria-label={i18n.t('upload.selectFilesAria')}
        aria-disabled={uploading}
        onClick={openFilePicker}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            openFilePicker()
          }
        }}
        onDragEnter={(e) => {
          e.preventDefault()
          dragDepthRef.current++
          setDragOver(true)
        }}
        onDragOver={(e) => e.preventDefault()}
        onDragLeave={(e) => {
          e.preventDefault()
          dragDepthRef.current = Math.max(0, dragDepthRef.current - 1)
          if (dragDepthRef.current === 0) setDragOver(false)
        }}
        onDrop={handleDrop}
      >
        <div className="dropzone-icon">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
        </div>
        <p className="dropzone-text">{t('upload.orDrag')}</p>
        <p className="dropzone-sub">{i18n.t('upload.orClickBelow')}</p>
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
          onClick={openFilePicker}
          disabled={uploading}
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
                disabled={uploading}
              >
                <span className="dot" style={{ background: cat.color }} />
                {cat.label}
              </button>
            ))}
          </div>

          <div className="upload-queue">
            {files.map((item, idx) => (
              <div key={item.file.name + item.file.size + item.file.lastModified} className={`upload-item ${item.status === 'done' ? 'is-done' : ''} ${item.status === 'error' ? 'is-error' : ''} ${item.status === 'uploading' ? 'is-uploading' : ''} ${item.status === 'hashing' ? 'is-hashing' : ''}`}>
                <div className="upload-item-icon">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                    <polygon points="23 7 16 12 23 17 23 7" />
                    <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
                  </svg>
                </div>
                <div className="upload-item-info">
                  <div className="upload-item-top">
                    <div className="upload-item-name">{item.name}</div>
                    <select
                      className="upload-item-cat"
                      value={item.category}
                      aria-label={i18n.t('upload.categoryAria', { name: item.name })}
                      disabled={!catEditable(item)}
                      onChange={(e) =>
                        setFiles((prev) =>
                          prev.map((f) => (f === item ? { ...f, category: e.target.value } : f))
                        )
                      }
                    >
                      {CATEGORIES.map((cat) => (
                        <option key={cat.key} value={cat.key}>{cat.label}</option>
                      ))}
                    </select>
                  </div>
                  <div className="upload-item-size">{formatSize(item.size)}</div>
                  <div className="upload-item-status">
                    <div
                      className="progress-bar"
                      role="progressbar"
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={item.progress}
                      aria-label={i18n.t('upload.progressAria', { name: item.name })}
                    >
                      <div className="progress-fill" style={{ width: item.progress + '%' }} />
                    </div>
                    <span className={`status-text ${getStatusClass(item)}`}>
                      {getStatusText(item)}
                    </span>
                  </div>
                </div>
                <div className="upload-item-actions">
                  {item.status === 'done' && item.videoId !== undefined && (
                    <button
                      className="upload-item-view"
                      title={i18n.t('upload.goToPlayer')}
                      onClick={() => navigate(`/player?id=${item.videoId}`)}
                    >
                      {i18n.t('upload.view')}
                    </button>
                  )}
                  {item.status === 'error' && !uploading && (
                    <button
                      className="upload-item-resume"
                      title={i18n.t('upload.retryTitle')}
                      onClick={() => retryItem(idx)}
                    >
                      {i18n.t('common.retry')}
                    </button>
                  )}
                  {(item.status === 'pending' || item.status === 'error') && (
                    <button
                      className="upload-item-remove"
                      onClick={() => removeFile(idx)}
                      disabled={uploading}
                      title={i18n.t('upload.remove')}
                      aria-label={i18n.t('upload.removeAria', { name: item.name })}
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                        <line x1="18" y1="6" x2="6" y2="18" />
                        <line x1="6" y1="6" x2="18" y2="18" />
                      </svg>
                    </button>
                  )}
                </div>
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
                {t('upload.pendingInfo', { pendingCount })}
              </button>
            )}
            <button className="upload-clear-btn" onClick={clearAll} disabled={uploading}>
              {t('upload.clearAll')}
            </button>
          </div>
        </>
      )}
    </div>
  )
}
