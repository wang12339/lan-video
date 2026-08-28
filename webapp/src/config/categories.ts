export interface Category {
  key: string
  i18nKey: string
  icon: string
  color: string
}

export const CATEGORIES: Category[] = [
  { key: 'all', i18nKey: 'home.categories.all', icon: '📋', color: '#fff' },
  { key: 'tech', i18nKey: 'home.categories.tech', icon: '💻', color: '#3b82f6' },
  { key: 'design', i18nKey: 'home.categories.design', icon: '🎨', color: '#ec4899' },
  { key: 'music', i18nKey: 'home.categories.music', icon: '🎵', color: '#8b5cf6' },
  { key: 'tutorial', i18nKey: 'home.categories.tutorial', icon: '📚', color: '#10b981' },
  { key: 'entertainment', i18nKey: 'home.categories.entertainment', icon: '🎮', color: '#f59e0b' },
  { key: 'sports', i18nKey: 'home.categories.sports', icon: '⚽', color: '#ef4444' },
  { key: 'record', i18nKey: 'home.categories.record', icon: '📷', color: '#06b6d4' },
  { key: 'external', i18nKey: 'home.categories.external', icon: '🌐', color: '#6b7280' },
]

export const UPLOAD_CATEGORIES = CATEGORIES.filter((c) => c.key !== 'all')

export const CATEGORY_API_MAP: Record<string, string> = {
  all: '全部',
  tech: '科技',
  design: '设计',
  music: '音乐',
  tutorial: '教程',
  entertainment: '娱乐',
  sports: '运动',
  record: '记录',
  external: '外部',
}
