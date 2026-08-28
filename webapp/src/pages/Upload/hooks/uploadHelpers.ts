import { APIError } from '../../../api'
import { CATEGORIES } from '../../../config/categories'
import type { UploadItem } from './useFileHash'

export { CATEGORIES }

export async function runPool<T>(items: T[], limit: number, worker: (item: T) => Promise<void>): Promise<void> {
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

export const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

export function isRetryable(e: unknown): boolean {
  if (e instanceof APIError) {
    return e.status === 0 || e.status === 429 || e.status >= 500
  }
  return e instanceof TypeError
}

export type UploadItemUpdater = (item: UploadItem, patch: Partial<UploadItem>) => void
