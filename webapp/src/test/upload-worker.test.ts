import { describe, it, expect, vi, beforeEach } from 'vitest'
import { APIError } from '../api'
import type { UploadItem } from '../pages/Upload/hooks/useFileHash'

vi.mock('../api/videos', () => ({
  getUploadStatus: vi.fn(),
  uploadResumeChunk: vi.fn(),
}))

vi.mock('../pages/Upload/hooks/useFileHash', async (importOriginal) => {
  const mod = await importOriginal<typeof import('../pages/Upload/hooks/useFileHash')>()
  return { ...mod, computeContentHash: vi.fn() }
})

import { getUploadStatus, uploadResumeChunk } from '../api/videos'
import { computeContentHash, CHUNK_SIZE, MIN_CHUNK_SIZE } from '../pages/Upload/hooks/useFileHash'
import { uploadSingleFile } from '../pages/Upload/hooks/uploadSingleWorker'

const mockedGetUploadStatus = vi.mocked(getUploadStatus)
const mockedUploadResumeChunk = vi.mocked(uploadResumeChunk)
const mockedComputeContentHash = vi.mocked(computeContentHash)

let uidSeq = 0

function makeItem(size = 1024): UploadItem {
  const file = new File([new Uint8Array(size)], 'a.mp4', { type: 'video/mp4' })
  return {
    uid: `test-${++uidSeq}`,
    file,
    name: 'a.mp4',
    size,
    status: 'pending',
    progress: 0,
    category: 'local',
  }
}

async function run(item: UploadItem): Promise<{ ok: boolean; item: UploadItem }> {
  let files: UploadItem[] = [item]
  const setFiles = (fn: (prev: UploadItem[]) => UploadItem[]) => {
    files = fn(files)
  }
  const ok = await uploadSingleFile(item, setFiles, { current: false })
  return { ok, item: files[0]! }
}

describe('uploadSingleFile 协议行为', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockedComputeContentHash.mockResolvedValue('a'.repeat(64))
  })

  it('exists=true 时零传输完成并带回已有视频 ID', async () => {
    mockedGetUploadStatus.mockResolvedValue({ received: 0, exists: true, existing_id: 42 })

    const { ok, item } = await run(makeItem())

    expect(ok).toBe(true)
    expect(item.status).toBe('done')
    expect(item.videoId).toBe(42)
    expect(mockedUploadResumeChunk).not.toHaveBeenCalled()
  })

  it('received==size 但 exists=false：发空 body finalize，不直接判定成功', async () => {
    mockedGetUploadStatus.mockResolvedValue({ received: 1024, exists: false })
    mockedUploadResumeChunk.mockResolvedValue({ received: 1024, id: 7 })

    const { ok, item } = await run(makeItem(1024))

    expect(ok).toBe(true)
    expect(item.status).toBe('done')
    expect(item.videoId).toBe(7)
    expect(mockedUploadResumeChunk).toHaveBeenCalledTimes(1)
    const call = mockedUploadResumeChunk.mock.calls[0]!
    expect((call[4] as Blob).size).toBe(0)
    expect(call[5]).toBe(1024)
  })

  it('最后一片无 id 时不得误报成功', async () => {
    mockedGetUploadStatus.mockResolvedValue({ received: 0, exists: false })
    mockedUploadResumeChunk
      .mockResolvedValueOnce({ received: 1024 })
      .mockRejectedValue(new APIError('文件校验失败', 400))

    const { ok, item } = await run(makeItem(1024))

    expect(ok).toBe(false)
    expect(item.status).toBe('error')
  })

  it('409 offset_mismatch 按服务端 received 回退重切片', async () => {
    mockedGetUploadStatus.mockResolvedValue({ received: 0, exists: false })
    mockedUploadResumeChunk
      .mockRejectedValueOnce(new APIError('偏移不一致', 409, 'offset_mismatch', { received: 512 }))
      .mockResolvedValueOnce({ received: 1024 })
      .mockResolvedValueOnce({ received: 1024, id: 9 })

    const { ok, item } = await run(makeItem(1024))

    expect(ok).toBe(true)
    expect(item.videoId).toBe(9)
    expect(mockedUploadResumeChunk.mock.calls[0]![5]).toBe(0)
    expect(mockedUploadResumeChunk.mock.calls[1]![5]).toBe(512)
    const resyncedChunk = mockedUploadResumeChunk.mock.calls[1]![4] as Blob
    expect(resyncedChunk.size).toBe(512)
  })

  it('duplicate 时按已有视频处理并补查 ID', async () => {
    mockedGetUploadStatus
      .mockResolvedValueOnce({ received: 0, exists: false })
      .mockResolvedValueOnce({ received: 0, exists: true, existing_id: 5 })
    mockedUploadResumeChunk.mockRejectedValue(
      new APIError('文件已存在，请勿重复上传', 409, 'duplicate')
    )

    const { ok, item } = await run(makeItem())

    expect(ok).toBe(true)
    expect(item.status).toBe('done')
    expect(item.videoId).toBe(5)
  })

  it('多分片上传每片偏移严格衔接且首片从最小分片起步', async () => {
    const size = CHUNK_SIZE + 100
    mockedGetUploadStatus.mockResolvedValue({ received: 0, exists: false })
    // 按传入 offset/chunk 回显 received；最后一片返回 id
    mockedUploadResumeChunk.mockImplementation(
      async (_hash, _name, totalSize, _category, chunk, offset) => {
        const received = (offset ?? 0) + chunk.size
        return received >= totalSize ? { received, id: 11 } : { received }
      },
    )

    const { ok, item } = await run(makeItem(size))

    expect(ok).toBe(true)
    expect(item.videoId).toBe(11)
    const calls = mockedUploadResumeChunk.mock.calls
    // 首片 = 最小分片（慢网下快速可见进度）
    expect(calls[0]![5]).toBe(0)
    expect((calls[0]![4] as Blob).size).toBe(MIN_CHUNK_SIZE)
    // 每个分片的 offset 必须等于前序分片累计字节数
    let expectedOffset = 0
    for (const call of calls) {
      expect(call[5]).toBe(expectedOffset)
      expect(call[4]).toBeInstanceOf(Blob)
      expectedOffset += (call[4] as Blob).size
    }
    expect(expectedOffset).toBe(size)
  })
})
