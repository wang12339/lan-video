// 大文件 SHA-256 哈希 Worker：优先使用 hash-wasm（WASM，比纯 JS 快数倍），
// 加载失败时回退到内置纯 JS 实现。放在 Worker 中执行避免阻塞主线程 UI。
//
// 注意：hash-wasm 必须静态导入 —— 动态 import() 会让 worker 产物产生代码
// 分割，与 Vite 默认的 IIFE worker 格式不兼容（构建直接失败）。
import { createSHA256 } from 'hash-wasm'
import { Sha256 } from './chunkUpload'

const SLICE_BYTES = 8 * 1024 * 1024

const ctx = self as unknown as {
  onmessage: ((e: MessageEvent<{ file: File }>) => void) | null
  postMessage: (msg: { type: 'done'; hash: string } | { type: 'error'; message: string }) => void
}

async function hashWithWasm(file: File): Promise<string | null> {
  try {
    const hasher = await createSHA256()
    hasher.init()
    for (let offset = 0; offset < file.size; offset += SLICE_BYTES) {
      const buf = await file
        .slice(offset, Math.min(offset + SLICE_BYTES, file.size))
        .arrayBuffer()
      hasher.update(new Uint8Array(buf))
    }
    return hasher.digest('hex')
  } catch {
    return null
  }
}

async function hashWithJs(file: File): Promise<string> {
  const hasher = new Sha256()
  for (let offset = 0; offset < file.size; offset += SLICE_BYTES) {
    const buf = await file
      .slice(offset, Math.min(offset + SLICE_BYTES, file.size))
      .arrayBuffer()
    hasher.update(new Uint8Array(buf))
  }
  return hasher.digest()
}

ctx.onmessage = async (e) => {
  try {
    const hash = (await hashWithWasm(e.data.file)) ?? (await hashWithJs(e.data.file))
    ctx.postMessage({ type: 'done', hash })
  } catch (err) {
    ctx.postMessage({
      type: 'error',
      message: err instanceof Error ? err.message : String(err),
    })
  }
}
