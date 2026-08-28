import { describe, it, expect } from 'vitest'
import { formatDuration, formatViews, formatBytes, getCatColor } from '../api/utils'

describe('formatDuration', () => {
  it('formats seconds to MM:SS or HH:MM:SS', () => {
    expect(formatDuration(0)).toBe('00:00')
    expect(formatDuration(65)).toBe('01:05')
    expect(formatDuration(3661)).toBe('1:01:01')
    expect(formatDuration(86399)).toBe('23:59:59')
  })

  it('handles edge cases', () => {
    expect(formatDuration(-1)).toBe('')
    expect(formatDuration(undefined as unknown as number)).toBe('')
    expect(formatDuration(null as unknown as number)).toBe('')
  })
})

describe('formatViews', () => {
  it('formats view counts with Chinese units', () => {
    expect(formatViews(0)).toBe('0')
    expect(formatViews(999)).toBe('999')
    expect(formatViews(1000)).toBe('1.0k')
    expect(formatViews(1234)).toBe('1.2k')
    expect(formatViews(10000)).toBe('1.0万')
    expect(formatViews(123456789)).toBe('1.2亿')
  })

  it('handles edge cases', () => {
    expect(formatViews(null)).toBe('')
    expect(formatViews(undefined)).toBe('')
  })
})

describe('getCatColor', () => {
  it('returns consistent colors for categories', () => {
    const color = getCatColor('科技')
    expect(color).toMatch(/^#[0-9a-f]{6}$/)
    expect(getCatColor('科技')).toBe(color)
  })

  it('handles unknown categories', () => {
    const color = getCatColor('未知分类')
    expect(color).toMatch(/^#[0-9a-f]{6}$/)
  })
})

describe('formatBytes', () => {
  it('formats zero bytes', () => {
    expect(formatBytes(0)).toBe('0 B')
  })

  it('formats bytes', () => {
    expect(formatBytes(500)).toBe('500 B')
  })

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1 KB')
    expect(formatBytes(1536)).toBe('1.5 KB')
  })

  it('formats megabytes', () => {
    expect(formatBytes(1048576)).toBe('1 MB')
  })

  it('formats gigabytes', () => {
    expect(formatBytes(1073741824)).toBe('1 GB')
  })

  it('formats terabytes', () => {
    expect(formatBytes(1099511627776)).toBe('1 TB')
  })
})
