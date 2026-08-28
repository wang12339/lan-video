import { describe, it, expect, vi, beforeAll } from 'vitest'
import { cleanupVideoElement, safeGetDuration } from '../utils/videoUtils'

class MockMediaStream {
  private tracks: Array<{ stop: vi.Mock }> = []
  constructor(tracks: Array<{ stop: vi.Mock }> = []) {
    this.tracks = tracks
  }
  getTracks() { return this.tracks }
}

// @ts-expect-error - jsdom doesn't have MediaStream
globalThis.MediaStream = MockMediaStream

function createMockVideo(overrides: Partial<HTMLVideoElement> = {}): HTMLVideoElement {
  const video = {
    pause: vi.fn(),
    removeAttribute: vi.fn(),
    load: vi.fn(),
    duration: 100,
    ...overrides,
  } as unknown as HTMLVideoElement
  return video
}

describe('cleanupVideoElement', () => {
  it('does nothing when video is null', () => {
    expect(() => cleanupVideoElement(null)).not.toThrow()
  })

  it('pauses the video', () => {
    const video = createMockVideo()
    cleanupVideoElement(video)
    expect(video.pause).toHaveBeenCalled()
  })

  it('removes src attribute', () => {
    const video = createMockVideo()
    cleanupVideoElement(video)
    expect(video.removeAttribute).toHaveBeenCalledWith('src')
  })

  it('calls load to release media resources', () => {
    const video = createMockVideo()
    cleanupVideoElement(video)
    expect(video.load).toHaveBeenCalled()
  })

  it('clears srcObject', () => {
    const video = createMockVideo({ srcObject: new MockMediaStream() as unknown as MediaStream })
    cleanupVideoElement(video)
    expect(video.srcObject).toBeNull()
  })

  it('stops all tracks when srcObject is a MediaStream-like object', () => {
    const track1 = { stop: vi.fn() }
    const track2 = { stop: vi.fn() }
    const stream = new MockMediaStream([track1, track2])
    const video = createMockVideo({ srcObject: stream as unknown as MediaStream })
    cleanupVideoElement(video)
    expect(track1.stop).toHaveBeenCalled()
    expect(track2.stop).toHaveBeenCalled()
  })

  it('does not try to get tracks from non-MediaStream srcObject', () => {
    const plainObj = { type: 'blob' }
    const video = createMockVideo({ srcObject: plainObj as unknown as MediaStream })
    cleanupVideoElement(video)
    expect(video.srcObject).toBeNull()
  })
})

describe('safeGetDuration', () => {
  it('returns 0 for null video', () => {
    expect(safeGetDuration(null)).toBe(0)
  })

  it('returns duration when valid', () => {
    const video = createMockVideo({ duration: 120.5 })
    expect(safeGetDuration(video)).toBe(120.5)
  })

  it('returns 0 for NaN duration', () => {
    const video = createMockVideo({ duration: NaN })
    expect(safeGetDuration(video)).toBe(0)
  })

  it('returns 0 for Infinity duration', () => {
    const video = createMockVideo({ duration: Infinity })
    expect(safeGetDuration(video)).toBe(0)
  })

  it('returns 0 for negative duration', () => {
    const video = createMockVideo({ duration: -5 })
    expect(safeGetDuration(video)).toBe(0)
  })

  it('returns 0 for zero duration', () => {
    const video = createMockVideo({ duration: 0 })
    expect(safeGetDuration(video)).toBe(0)
  })
})
