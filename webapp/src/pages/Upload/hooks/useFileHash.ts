import { Sha256 } from '../utils/chunkUpload'

export const CHUNK_SIZE = 5 * 1024 * 1024
export const VIDEO_MAX_SIZE = 50 * 1024 * 1024 * 1024
export const IMAGE_MAX_SIZE = 50 * 1024 * 1024
export const MAX_CHUNK_RETRIES = 3
export const RETRY_BASE_DELAY_MS = 800
export const CONCURRENT_UPLOADS = 2
export const SMALL_FILE_BYTES = 4 * 1024 * 1024
export const HASH_SLICE_BYTES = 8 * 1024 * 1024

export const VIDEO_EXTS = ['mp4', 'm4v', 'm3u8', 'mov', 'avi', 'mkv', 'webm', 'flv', 'wmv']
export const IMAGE_EXTS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp']

export interface UploadItem {
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
  return Array.from(new Uint8Array(buf)).map((b) => b.toString(16).padStart(2, '0')).join('')
}

export async function computeContentHash(file: File, isCancelled: () => boolean): Promise<string> {
  const subtle = crypto.subtle
  if (subtle && (file.size <= SMALL_FILE_BYTES || !verifyStreamingHash())) {
    return hexFromBuffer(await subtle.digest('SHA-256', await file.arrayBuffer()))
  }
  if (!verifyStreamingHash()) {
    throw new Error('File hashing is not supported in this environment')
  }
  const hasher = new Sha256()
  for (let offset = 0; offset < file.size; offset += HASH_SLICE_BYTES) {
    if (isCancelled()) throw new CancelledError()
    const buf = await file.slice(offset, Math.min(offset + HASH_SLICE_BYTES, file.size)).arrayBuffer()
    hasher.update(new Uint8Array(buf))
  }
  return hasher.digest()
}
