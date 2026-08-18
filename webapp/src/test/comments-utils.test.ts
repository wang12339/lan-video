import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { vi } from 'vitest'
import i18n from '../i18n'
import { formatDate, getInitial } from '../components/Comments/utils'

describe('getInitial', () => {
  it('returns first character of username', () => {
    expect(getInitial('Alice')).toBe('A')
    expect(getInitial('张三')).toBe('张')
  })

  it('trims whitespace before taking first char', () => {
    expect(getInitial('  Bob  ')).toBe('B')
  })

  it('falls back to ? for empty or blank usernames', () => {
    expect(getInitial('')).toBe('?')
    expect(getInitial('   ')).toBe('?')
  })
})

describe('formatDate', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('zh-CN')
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-08-13T12:00:00Z'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  const ago = (ms: number) => new Date(Date.now() - ms).toISOString()

  it('returns 刚刚 within the first minute', () => {
    expect(formatDate(ago(0))).toBe('刚刚')
    expect(formatDate(ago(59999))).toBe('刚刚')
  })

  it('returns minutesAgo within the first hour', () => {
    expect(formatDate(ago(60000))).toBe('1分钟前')
    expect(formatDate(ago(5 * 60000))).toBe('5分钟前')
  })

  it('returns hoursAgo within the first day', () => {
    expect(formatDate(ago(3 * 3600000))).toBe('3小时前')
    expect(formatDate(ago(23 * 3600000))).toBe('23小时前')
  })

  it('returns daysAgo within the first month', () => {
    expect(formatDate(ago(2 * 86400000))).toBe('2天前')
    expect(formatDate(ago(29 * 86400000))).toBe('29天前')
  })

  it('returns date part (YYYY-MM-DD) for older dates', () => {
    expect(formatDate(ago(30 * 86400000))).toBe('2026-07-14')
    expect(formatDate('2020-01-01T00:00:00Z')).toBe('2020-01-01')
  })

  it('falls back to sliced input for invalid dates', () => {
    expect(formatDate('not-a-real-date')).toBe('not-a-real')
    expect(formatDate('')).toBe('')
  })
})
