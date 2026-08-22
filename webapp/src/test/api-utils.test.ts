import { describe, it, expect } from 'vitest'
import {
  mapVideo, mapImage, mapHistory, mapPlaylist, formatCount, formatDuration,
} from '../api/utils'
import type { Video, PlaybackHistory } from '../api/types'
import type { Playlist } from '../api/playlists'

function makeVideo(overrides: Partial<Video> = {}): Video {
  return {
    id: '7',
    title: '测试视频',
    description: '描述',
    sourceType: 'local_video',
    coverUrl: null,
    streamUrl: '/media/v1.mp4',
    thumbUrl: '/media/v1.jpg',
    category: '科技',
    views: 100,
    duration: 65,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function makePlaylist(overrides: Partial<Playlist> = {}): Playlist {
  return {
    id: '1',
    name: '收藏夹',
    description: null,
    is_public: true,
    cover_url: null,
    item_count: 3,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
    ...overrides,
  }
}

describe('mapVideo', () => {
  it('returns null for null input', () => {
    expect(mapVideo(null)).toBeNull()
  })

  it('maps all fields', () => {
    const m = mapVideo(makeVideo())
    expect(m).toEqual({
      id: '7',
      title: '测试视频',
      category: '科技',
      description: '描述',
      thumb: '/media/v1.jpg',
      thumbnail_url: '/media/v1.jpg',
      stream: '/media/v1.mp4',
      cover: null,
      sourceType: 'local_video',
      duration: 65,
      views: 100,
      date: '2026-01-01T00:00:00Z',
      progress: 0,
      hasVariants: undefined,
      uploaderId: undefined,
    })
  })

  it('falls back to coverUrl for thumb when thumbUrl is missing', () => {
    const m = mapVideo(makeVideo({ thumbUrl: null, coverUrl: '/media/cover.jpg' }))
    expect(m?.thumb).toBe('/media/cover.jpg')
    expect(m?.cover).toBe('/media/cover.jpg')
  })

  it('falls back to placeholder data URL when no image available', () => {
    const m = mapVideo(makeVideo({ thumbUrl: null, coverUrl: null }))
    expect(m?.thumb).toMatch(/^data:image\/svg\+xml/)
  })

  it('applies defaults for empty fields', () => {
    const m = mapVideo(makeVideo({
      title: '', category: '', description: '', sourceType: '',
      duration: 0, views: 0, watchPosition: undefined,
    }))
    expect(m?.title).toBe('未命名')
    expect(m?.category).toBe('general')
    expect(m?.description).toBe('')
    expect(m?.sourceType).toBe('local_video')
    expect(m?.duration).toBe(0)
    expect(m?.views).toBe(0)
    expect(m?.progress).toBe(0)
  })

  it('passes through watchPosition as progress', () => {
    const m = mapVideo(makeVideo({ watchPosition: 45 }))
    expect(m?.progress).toBe(45)
  })
})

describe('mapImage', () => {
  it('returns null for null input', () => {
    expect(mapImage(null)).toBeNull()
  })

  it('maps fields with thumb from thumbUrl', () => {
    const m = mapImage(makeVideo())
    expect(m).toEqual({
      id: '7',
      title: '测试视频',
      category: '科技',
      thumb: '/media/v1.jpg',
      sourceType: 'local_video',
    })
  })

  it('falls back to streamUrl for image items', () => {
    const m = mapImage(makeVideo({ thumbUrl: null }))
    expect(m?.thumb).toBe('/media/v1.mp4')
  })

  it('uses placeholder data URL when nothing available', () => {
    const m = mapImage(makeVideo({ thumbUrl: null, streamUrl: '' }))
    expect(m?.thumb).toMatch(/^data:image\/svg\+xml/)
  })
})

describe('mapHistory', () => {
  function makeHistory(overrides: Partial<PlaybackHistory> = {}): PlaybackHistory {
    return {
      videoId: '3',
      title: '历史视频',
      coverUrl: '/media/h.jpg',
      streamUrl: '/media/h.mp4',
      sourceType: 'local_video',
      category: '教程',
      positionMs: 50000,
      durationMs: 100000,
      updatedAt: '2026-01-03T00:00:00Z',
      ...overrides,
    }
  }

  it('returns null for null input', () => {
    expect(mapHistory(null)).toBeNull()
  })

  it('maps fields and computes progress percentage', () => {
    const m = mapHistory(makeHistory())
    expect(m).toEqual({
      id: '3',
      title: '历史视频',
      category: '教程',
      thumb: '/media/h.jpg',
      stream: '/media/h.mp4',
      sourceType: 'local_video',
      positionMs: 50000,
      durationMs: 100000,
      updatedAt: '2026-01-03T00:00:00Z',
      progress: 50,
    })
  })

  it('rounds progress with Math.round', () => {
    const m = mapHistory(makeHistory({ positionMs: 33333, durationMs: 100000 }))
    expect(m?.progress).toBe(33)
  })

  it('progress is 0 when durationMs is 0', () => {
    const m = mapHistory(makeHistory({ positionMs: 50000, durationMs: 0 }))
    expect(m?.progress).toBe(0)
  })

  it('applies defaults for empty title/category/sourceType', () => {
    const m = mapHistory(makeHistory({ title: '', category: '', sourceType: '' }))
    expect(m?.title).toBe('未命名')
    expect(m?.category).toBe('general')
    expect(m?.sourceType).toBe('local_video')
  })

  it('uses streamUrl as thumb for local_image history', () => {
    const m = mapHistory(makeHistory({
      coverUrl: null, sourceType: 'local_image', streamUrl: '/media/img.png',
    }))
    expect(m?.thumb).toBe('/media/img.png')
  })

  it('falls back to placeholder when no image source', () => {
    const m = mapHistory(makeHistory({ coverUrl: null, sourceType: 'local_video', streamUrl: '' }))
    expect(m?.thumb).toMatch(/^data:image\/svg\+xml/)
  })
})

describe('mapPlaylist', () => {
  it('returns null for null input', () => {
    expect(mapPlaylist(null)).toBeNull()
  })

  it('maps all fields', () => {
    const p = mapPlaylist(makePlaylist({ cover_url: '/media/c.jpg' }))
    expect(p).toEqual({
      id: '1',
      name: '收藏夹',
      description: null,
      isPublic: true,
      coverUrl: '/media/c.jpg',
      itemCount: 3,
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-02T00:00:00Z',
    })
  })

  it('coverUrl is null when cover_url is missing', () => {
    expect(mapPlaylist(makePlaylist())?.coverUrl).toBeNull()
  })
})

describe('formatDuration extras', () => {
  it('uses zeroFallback for zero and invalid inputs', () => {
    expect(formatDuration(0, '—')).toBe('—')
    expect(formatDuration(NaN, '暂无')).toBe('暂无')
    expect(formatDuration(-1, '—')).toBe('—')
  })

  it('formats hour values without zero-padding hours', () => {
    expect(formatDuration(3600)).toBe('1:00:00')
    expect(formatDuration(7200)).toBe('2:00:00')
  })
})

describe('formatCount alias', () => {
  it('behaves identically to formatViews', () => {
    expect(formatCount(10000)).toBe('1.0万')
    expect(formatCount(null)).toBe('')
  })
})
