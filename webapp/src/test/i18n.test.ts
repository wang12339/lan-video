import { describe, it, expect } from 'vitest'
import {
  formatDate,
  formatNumber,
  formatFileSize,
  formatPercent,
  pluralize
} from '../utils/i18n'

describe('国际化工具函数', () => {
  describe('formatDate', () => {
    it('格式化日期（中文）', () => {
      const date = new Date('2026-01-15')
      const result = formatDate(date, 'zh-CN')
      expect(result).toContain('2026')
      expect(result).toContain('1')
      expect(result).toContain('15')
    })

    it('格式化日期（英文）', () => {
      const date = new Date('2026-01-15')
      const result = formatDate(date, 'en-US')
      expect(result).toContain('2026')
      expect(result).toContain('Jan')
    })
  })

  describe('formatNumber', () => {
    it('格式化数字（中文）', () => {
      expect(formatNumber(1234567, 'zh-CN')).toBe('1,234,567')
    })

    it('格式化小数', () => {
      expect(formatNumber(1234.56, 'zh-CN', { maximumFractionDigits: 1 })).toBe('1,234.6')
    })
  })

  describe('formatFileSize', () => {
    it('格式化字节', () => {
      expect(formatFileSize(1024, 'zh-CN')).toContain('1')
      expect(formatFileSize(1024, 'zh-CN')).toContain('KB')
    })

    it('格式化MB', () => {
      expect(formatFileSize(1048576, 'zh-CN')).toContain('1')
      expect(formatFileSize(1048576, 'zh-CN')).toContain('MB')
    })
  })

  describe('formatPercent', () => {
    it('格式化百分比', () => {
      expect(formatPercent(75, 'zh-CN')).toContain('75')
    })
  })

  describe('pluralize', () => {
    it('中文复数', () => {
      expect(pluralize(1, '个', undefined, 'zh-CN')).toBe('1 个')
      expect(pluralize(5, '个', undefined, 'zh-CN')).toBe('5 个')
    })

    it('英文复数', () => {
      expect(pluralize(1, 'video', undefined, 'en-US')).toBe('1 video')
      expect(pluralize(5, 'video', undefined, 'en-US')).toBe('5 videos')
    })
  })
})
