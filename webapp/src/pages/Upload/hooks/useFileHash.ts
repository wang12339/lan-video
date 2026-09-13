import { Sha256 } from '../utils/chunkUpload'

/** 默认分片大小（服务端 body 上限 32MB，留出余量）。 */
export const CHUNK_SIZE = 16 * 1024 * 1024
/** 自适应分片下限，也是首片大小：慢网下首片即可见进度并降低超时风险。 */
export const MIN_CHUNK_SIZE = 2 * 1024 * 1024
/** 自适应分片上限（= 默认值）。 */
export const MAX_CHUNK_SIZE = CHUNK_SIZE
/** 单片耗时超过该值则下一片减半（毫秒）。 */
export const SLOW_CHUNK_MS = 45_000
/** 单片耗时低于该值则下一片加倍（毫秒）。 */
export const FAST_CHUNK_MS = 5_000
export const VIDEO_MAX_SIZE = 50 * 1024 * 1024 * 1024
export const IMAGE_MAX_SIZE = 50 * 1024 * 1024
export const MAX_CHUNK_RETRIES = 3
export const RETRY_BASE_DELAY_MS = 800
export const CONCURRENT_UPLOADS = 4
export const SMALL_FILE_BYTES = 4 * 1024 * 1024
export const HASH_SLICE_BYTES = 8 * 1024 * 1024

export const VIDEO_EXTS = ['mp4', 'm4v', 'm3u8', 'mov', 'avi', 'mkv', 'webm', 'flv', 'wmv']
export const IMAGE_EXTS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp']

export interface UploadItem {
  /** 队列内唯一 id：worker 的 updateItem 靠它匹配状态更新
   *（不能用对象引用——第一次 update 就会 {...f,...patch} 替换成新对象，
   * 引用比对永远失配，后续进度/错误/完成全部静默丢失，状态卡在 hashing） */
  uid: string
  file: File
  name: string
  size: number
  status: 'pending' | 'hashing' | 'uploading' | 'done' | 'error'
  progress: number
  errorMsg?: string
  category: string
  contentHash?: string
  videoId?: number | string
}

export class CancelledError extends Error {
  constructor() {
    super('Cancelled')
    this.name = 'CancelledError'
  }
}

export { formatFileSize as formatSize } from '../../../utils/i18n'

export function isSupportedFile(f: File): { ok: boolean; kind: 'video' | 'image' | 'other' } {
  const type = f.type.toLowerCase()
  if (type.startsWith('video/')) return { ok: true, kind: 'video' }
  if (type.startsWith('image/')) return { ok: true, kind: 'image' }
  const ext = (f.name.split('.').pop() ?? '').toLowerCase()
  if (VIDEO_EXTS.includes(ext)) return { ok: true, kind: 'video' }
  if (IMAGE_EXTS.includes(ext)) return { ok: true, kind: 'image' }
  return { ok: false, kind: 'other' }
}

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
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * 在 Web Worker 中计算大文件哈希（WASM 优先，纯 JS 回退）。
 *
 * 返回 `null` 表示环境不支持 Worker 或 Worker 内部失败，调用方回退到
 * 主线程流式实现。取消时 reject `CancelledError` 并终止 Worker。
 */
function computeHashInWorker(file: File, isCancelled: () => boolean): Promise<string | null> {
  if (typeof Worker === 'undefined') return Promise.resolve(null)
  return new Promise((resolve, reject) => {
    let worker: Worker
    try {
      worker = new Worker(new URL('../utils/hashWorker.ts', import.meta.url), {
        type: 'module',
      })
    } catch {
      resolve(null)
      return
    }
    let settled = false
    const finish = (fn: () => void) => {
      if (settled) return
      settled = true
      clearInterval(timer)
      worker.terminate()
      fn()
    }
    const timer = setInterval(() => {
      if (isCancelled()) finish(() => reject(new CancelledError()))
    }, 100)
    worker.onmessage = (e: MessageEvent) => {
      const data = e.data as { type?: string; hash?: string }
      if (data?.type === 'done' && typeof data.hash === 'string') {
        finish(() => resolve(data.hash ?? null))
      } else {
        finish(() => resolve(null))
      }
    }
    worker.onerror = () => finish(() => resolve(null))
    worker.postMessage({ file })
  })
}

export async function computeContentHash(file: File, isCancelled: () => boolean): Promise<string> {
  const subtle = crypto.subtle
  // 小文件用 WebCrypto 一次性摘要;大文件绝不可 `file.arrayBuffer()`
  // 整读(50GB 视频会直接 OOM),必须走流式分块。
  if (subtle && file.size <= SMALL_FILE_BYTES) {
    // 必须包一层 Uint8Array:jsdom 等环境下 Blob.arrayBuffer() 返回的是
    // 另一个 realm 的 ArrayBuffer,WebCrypto 的 instanceof 检查会拒绝
    // (Node 20 实测),TypedArray 视图走 ArrayBuffer.isView 检查,跨 realm 安全。
    const buf = await file.arrayBuffer()
    return hexFromBuffer(await subtle.digest('SHA-256', new Uint8Array(buf)))
  }
  // 大文件优先丢进 Worker（WASM/纯 JS），不阻塞主线程。
  const viaWorker = await computeHashInWorker(file, isCancelled)
  if (viaWorker !== null) return viaWorker
  if (!verifyStreamingHash()) {
    throw new Error('File hashing is not supported in this environment')
  }
  const hasher = new Sha256()
  for (let offset = 0; offset < file.size; offset += HASH_SLICE_BYTES) {
    if (isCancelled()) throw new CancelledError()
    const buf = await file
      .slice(offset, Math.min(offset + HASH_SLICE_BYTES, file.size))
      .arrayBuffer()
    hasher.update(new Uint8Array(buf))
  }
  return hasher.digest()
}
