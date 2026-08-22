import { useEffect, useCallback, useMemo, useState, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import './KeyboardShortcuts.css'

// ─── Types ───────────────────────────────────────────────────────────────────

export interface ShortcutDef {
  /** 稳定的唯一标识，如 'player.playPause' */
  id: string
  /** i18n 翻译 key（显示名） */
  labelKey: string
  /** 默认按键组合 */
  defaultKeys: string[]
  /** 当前生效的按键组合（可被用户覆盖） */
  keys: string[]
  /** 所属分组 */
  group: string
  /** 作用域：全局 或 仅播放器 */
  scope: 'global' | 'player'
  /** 触发条件 */
  modifiers?: ('ctrl' | 'meta' | 'alt' | 'shift')[]
}

interface ShortcutSection {
  title: string
  shortcuts: ShortcutDef[]
}

// ─── Registry ────────────────────────────────────────────────────────────────

const STORAGE_KEY = 'atmos_shortcut_overrides'

/** 默认快捷键注册表 — 所有快捷键在此声明一次 */
const DEFAULT_SHORTCUTS: Omit<ShortcutDef, 'keys'>[] = [
  // 播放控制
  { id: 'player.playPause',    labelKey: 'shortcuts.playPause',    defaultKeys: ['Space'],  group: 'playback', scope: 'player' },
  { id: 'player.mute',         labelKey: 'shortcuts.mute',         defaultKeys: ['M'],      group: 'playback', scope: 'player' },
  { id: 'player.fullscreen',   labelKey: 'shortcuts.fullscreen',   defaultKeys: ['F'],      group: 'playback', scope: 'player' },
  { id: 'player.skipForward',  labelKey: 'shortcuts.skipForward',  defaultKeys: ['→'],      group: 'playback', scope: 'player' },
  { id: 'player.skipBackward', labelKey: 'shortcuts.skipBackward', defaultKeys: ['←'],      group: 'playback', scope: 'player' },
  { id: 'player.volumeUp',     labelKey: 'shortcuts.volumeUp',     defaultKeys: ['↑'],      group: 'playback', scope: 'player' },
  { id: 'player.volumeDown',   labelKey: 'shortcuts.volumeDown',   defaultKeys: ['↓'],      group: 'playback', scope: 'player' },
  { id: 'player.speedUp',      labelKey: 'shortcuts.speedUp',      defaultKeys: ['>'],      group: 'playback', scope: 'player' },
  { id: 'player.speedDown',    labelKey: 'shortcuts.speedDown',    defaultKeys: ['<'],      group: 'playback', scope: 'player' },
  // 页面导航
  { id: 'nav.search',          labelKey: 'shortcuts.search',       defaultKeys: ['/'],      group: 'navigation', scope: 'global' },
  { id: 'nav.home',            labelKey: 'shortcuts.home',         defaultKeys: ['H'],      group: 'navigation', scope: 'global' },
  { id: 'nav.gallery',         labelKey: 'shortcuts.gallery',      defaultKeys: ['G'],      group: 'navigation', scope: 'global' },
  { id: 'nav.upload',          labelKey: 'shortcuts.upload',       defaultKeys: ['U'],      group: 'navigation', scope: 'global' },
  { id: 'nav.profile',         labelKey: 'shortcuts.profile',      defaultKeys: ['P'],      group: 'navigation', scope: 'global' },
  { id: 'nav.showShortcuts',   labelKey: 'shortcuts.showShortcuts',defaultKeys: ['?'],      group: 'navigation', scope: 'global' },
  // 通用
  { id: 'general.closeDialog', labelKey: 'shortcuts.closeDialog',  defaultKeys: ['Esc'],    group: 'general', scope: 'global' },
  { id: 'general.scrollToTop', labelKey: 'shortcuts.scrollToTop',  defaultKeys: ['Home'],   group: 'general', scope: 'global' },
  { id: 'general.scrollToEnd', labelKey: 'shortcuts.scrollToBottom',defaultKeys: ['End'],   group: 'general', scope: 'global' },
]

// ─── Utilities ───────────────────────────────────────────────────────────────

/** 将事件标准化为可比较的快捷键字符串，如 "ctrl+shift+k" */
function normalizeEvent(e: KeyboardEvent): string {
  const parts: string[] = []
  if (e.ctrlKey) parts.push('ctrl')
  if (e.metaKey) parts.push('meta')
  if (e.altKey) parts.push('alt')
  if (e.shiftKey) parts.push('shift')
  parts.push(e.key.toLowerCase())
  return parts.join('+')
}

/** 将 ShortcutDef.keys 数组标准化为同样的字符串用于比较 */
function normalizeKeys(keys: string[]): string {
  return keys.map(k => k.toLowerCase().trim()).sort().join('+')
}

/** 检查元素是否为可输入控件 */
function isInputElement(el: EventTarget | null): boolean {
  if (!el || !(el instanceof HTMLElement)) return false
  const tag = el.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || el.isContentEditable
}

/** 从 localStorage 读取用户自定义覆盖 */
function loadOverrides(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    return JSON.parse(raw) as Record<string, string[]>
  } catch {
    return {}
  }
}

/** 保存用户自定义覆盖到 localStorage */
function saveOverrides(overrides: Record<string, string[]>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides))
  } catch { /* quota exceeded, silently ignore */ }
}

// ─── Hook: useShortcutRegistry ───────────────────────────────────────────────

/**
 * 核心 Hook：管理快捷键注册表
 * - 从 localStorage 加载用户自定义覆盖
 * - 提供冲突检测
 * - 暴露查询/修改接口
 */
export function useShortcutRegistry() {
  const [overrides, setOverrides] = useState<Record<string, string[]>>(loadOverrides)

  // 构建完整的快捷键列表（默认 + 用户覆盖）
  const shortcuts: ShortcutDef[] = useMemo(() =>
    DEFAULT_SHORTCUTS.map(s => ({
      ...s,
      keys: overrides[s.id] ?? s.defaultKeys,
    })),
  [overrides])

  // 冲突检测：找出所有被多个快捷键占用的按键组合
  const conflicts = useMemo(() => {
    const map = new Map<string, string[]>() // normalizedKey → shortcut ids
    for (const s of shortcuts) {
      const nk = normalizeKeys(s.keys)
      if (!nk) continue
      const existing = map.get(nk)
      if (existing) {
        existing.push(s.id)
      } else {
        map.set(nk, [s.id])
      }
    }
    const result = new Map<string, string[]>()
    for (const [key, ids] of map) {
      if (ids.length > 1) result.set(key, ids)
    }
    return result
  }, [shortcuts])

  /** 更新某个快捷键的绑定 */
  const updateShortcut = useCallback((id: string, newKeys: string[]) => {
    setOverrides(prev => {
      const next = { ...prev, [id]: newKeys }
      saveOverrides(next)
      return next
    })
  }, [])

  /** 将某个快捷键恢复为默认 */
  const resetShortcut = useCallback((id: string) => {
    setOverrides(prev => {
      const next = { ...prev }
      delete next[id]
      saveOverrides(next)
      return next
    })
  }, [])

  /** 恢复所有快捷键为默认 */
  const resetAll = useCallback(() => {
    setOverrides({})
    saveOverrides({})
  }, [])

  /** 按 group 分组 */
  const sections: ShortcutSection[] = useMemo(() => {
    const groups = ['playback', 'navigation', 'general'] as const
    const groupTitles: Record<string, string> = {
      playback: 'shortcuts.playback',
      navigation: 'shortcuts.navigation',
      general: 'shortcuts.general',
    }
    return groups.map(g => ({
      title: groupTitles[g]!,
      shortcuts: shortcuts.filter(s => s.group === g),
    }))
  }, [shortcuts])

  return {
    shortcuts,
    sections,
    conflicts,
    updateShortcut,
    resetShortcut,
    resetAll,
    hasOverrides: Object.keys(overrides).length > 0,
  }
}

// ─── Hook: useShortcutsHelp ──────────────────────────────────────────────────

/**
 * Hook：管理快捷键帮助面板的显示/隐藏
 * 在 Layout 级别使用，返回 toggle 函数和 visible 状态
 */
export function useShortcutsHelp() {
  const [visible, setVisible] = useState(false)

  const toggle = useCallback(() => setVisible(v => !v), [])
  const show = useCallback(() => setVisible(true), [])
  const hide = useCallback(() => setVisible(false), [])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isInputElement(e.target)) return
      // ? to toggle (no modifiers)
      if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault()
        setVisible(v => !v)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  return { visible, toggle, show, hide }
}

// ─── Hook: useGlobalShortcuts ────────────────────────────────────────────────

/**
 * Hook：注册全局导航快捷键（在 Layout 级别使用）
 * 仅处理 scope === 'global' 的快捷键
 */
export function useGlobalShortcuts(
  navigate: (path: string) => void,
  toggleShortcuts: () => void,
) {
  const { shortcuts } = useShortcutRegistry()

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isInputElement(e.target)) return

      const nk = normalizeEvent(e)
      const globalShortcuts = shortcuts.filter(s => s.scope === 'global')

      for (const s of globalShortcuts) {
        if (normalizeKeys(s.keys) !== nk) continue

        switch (s.id) {
          case 'nav.search':
            e.preventDefault()
            document.querySelector<HTMLInputElement>('.nav-search input')?.focus()
            return
          case 'nav.home':
            e.preventDefault()
            navigate('/')
            return
          case 'nav.gallery':
            e.preventDefault()
            navigate('/gallery')
            return
          case 'nav.upload':
            e.preventDefault()
            navigate('/upload')
            return
          case 'nav.profile':
            e.preventDefault()
            navigate('/profile')
            return
          case 'nav.showShortcuts':
            e.preventDefault()
            toggleShortcuts()
            return
          case 'general.scrollToTop':
            e.preventDefault()
            window.scrollTo({ top: 0, behavior: 'smooth' })
            return
          case 'general.scrollToEnd':
            e.preventDefault()
            window.scrollTo({ top: document.body.scrollHeight, behavior: 'smooth' })
            return
          case 'general.closeDialog':
            // 由各 Dialog 自行处理 Esc
            return
        }
      }
    }

    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [shortcuts, navigate, toggleShortcuts])
}

// ─── Conflict Warning Component ──────────────────────────────────────────────

interface ConflictWarningProps {
  conflicts: Map<string, string[]>
  shortcuts: ShortcutDef[]
}

function ConflictWarning({ conflicts, shortcuts }: ConflictWarningProps) {
  const { t } = useTranslation()
  if (conflicts.size === 0) return null

  return (
    <div className="shortcuts-conflict" role="alert">
      <span className="shortcuts-conflict-icon" aria-hidden="true">⚠</span>
      <div className="shortcuts-conflict-list">
        {[...conflicts.entries()].map(([key, ids]) => {
          const names = ids.map(id => {
            const s = shortcuts.find(s => s.id === id)
            return s ? t(s.labelKey) : id
          })
          return (
            <div key={key} className="shortcuts-conflict-item">
              <kbd>{key}</kbd> {t('shortcuts.conflictAssigned', { names: names.join(', ') })}
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ─── Inline Key Editor ───────────────────────────────────────────────────────

interface KeyEditorProps {
  shortcut: ShortcutDef
  onUpdate: (id: string, keys: string[]) => void
  onReset: (id: string) => void
  isConflict: boolean
}

function KeyEditor({ shortcut, onUpdate, onReset, isConflict }: KeyEditorProps) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [pendingKeys, setPendingKeys] = useState<string[]>([])
  const inputRef = useRef<HTMLDivElement>(null)

  const isModified = normalizeKeys(shortcut.keys) !== normalizeKeys(shortcut.defaultKeys)

  useEffect(() => {
    if (!editing) return
    const handler = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()

      // Esc cancels editing
      if (e.key === 'Escape') {
        setEditing(false)
        return
      }

      // Enter confirms
      if (e.key === 'Enter') {
        if (pendingKeys.length > 0) {
          onUpdate(shortcut.id, pendingKeys)
        }
        setEditing(false)
        return
      }

      // Backspace/Delete clears
      if (e.key === 'Backspace' || e.key === 'Delete') {
        setPendingKeys([])
        return
      }

      // Build key combo
      const parts: string[] = []
      if (e.ctrlKey) parts.push('Ctrl')
      if (e.metaKey) parts.push('Meta')
      if (e.altKey) parts.push('Alt')
      if (e.shiftKey) parts.push('Shift')
      // Only add the main key if it's not a modifier itself
      const mainKey = e.key
      if (!['Control', 'Meta', 'Alt', 'Shift'].includes(mainKey)) {
        parts.push(mainKey === ' ' ? 'Space' : mainKey)
      }
      if (parts.length > 0) {
        setPendingKeys(parts)
      }
    }

    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  }, [editing, pendingKeys, shortcut.id, onUpdate])

  if (editing) {
    return (
      <div
        className={`shortcuts-key-editor ${isConflict ? 'conflict' : ''}`}
        ref={inputRef}
        tabIndex={-1}
      >
        {pendingKeys.length > 0 ? (
          pendingKeys.map((k, i) => (
            <span key={k}>
              <kbd className="shortcuts-key editing">{k}</kbd>
              {i < pendingKeys.length - 1 && <span className="shortcuts-plus">+</span>}
            </span>
          ))
        ) : (
          <span className="shortcuts-key-placeholder">
            {t('shortcuts.pressNewKey')}
          </span>
        )}
        <button
          className="shortcuts-key-action"
          onClick={() => setEditing(false)}
          title="Esc"
        >
          ✕
        </button>
      </div>
    )
  }

  return (
    <div className={`shortcuts-keys ${isConflict ? 'conflict' : ''}`}>
      {shortcut.keys.map((key, index) => (
        <span key={key}>
          <kbd className="shortcuts-key">{key}</kbd>
          {index < shortcut.keys.length - 1 && (
            <span className="shortcuts-plus">+</span>
          )}
        </span>
      ))}
      <button
        className="shortcuts-key-action edit"
        onClick={() => {
          setPendingKeys(shortcut.keys)
          setEditing(true)
        }}
        title={t('shortcuts.editKey')}
      >
        ✎
      </button>
      {isModified && (
        <button
          className="shortcuts-key-action reset"
          onClick={() => onReset(shortcut.id)}
          title={t('shortcuts.resetKey')}
        >
          ↺
        </button>
      )}
    </div>
  )
}

// ─── Main Component ──────────────────────────────────────────────────────────

interface KeyboardShortcutsProps {
  visible: boolean
  onClose: () => void
}

export default function KeyboardShortcuts({ visible, onClose }: KeyboardShortcutsProps) {
  const { t } = useTranslation()
  const { sections, shortcuts, conflicts, updateShortcut, resetShortcut, resetAll, hasOverrides } = useShortcutRegistry()

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape' && visible) {
      onClose()
    }
  }, [visible, onClose])

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  // Lock body scroll when visible
  useEffect(() => {
    if (visible) {
      document.body.style.overflow = 'hidden'
    } else {
      document.body.style.overflow = ''
    }
    return () => { document.body.style.overflow = '' }
  }, [visible])

  return (
    <div
      className={`shortcuts-overlay ${visible ? 'visible' : ''}`}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={t('shortcuts.title')}
    >
      <div className="shortcuts-panel" onClick={(e) => e.stopPropagation()}>
        <div className="shortcuts-header">
          <h2 className="shortcuts-title">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="2" y="4" width="20" height="16" rx="2" />
              <line x1="6" y1="8" x2="6" y2="8" />
              <line x1="10" y1="8" x2="10" y2="8" />
              <line x1="14" y1="8" x2="14" y2="8" />
              <line x1="18" y1="8" x2="18" y2="8" />
              <line x1="6" y1="12" x2="18" y2="12" />
              <line x1="6" y1="16" x2="18" y2="16" />
            </svg>
            {t('shortcuts.title')}
          </h2>
          <button className="shortcuts-close" onClick={onClose} aria-label={t('common.close')}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <ConflictWarning conflicts={conflicts} shortcuts={shortcuts} />

        <div className="shortcuts-content">
          {sections.map((section) => (
            <div key={section.title} className="shortcuts-section">
              <h3 className="shortcuts-section-title">{t(section.title)}</h3>
              <div className="shortcuts-list">
                {section.shortcuts.map((shortcut) => (
                  <div key={shortcut.id} className="shortcuts-item">
                    <span className="shortcuts-action">{t(shortcut.labelKey)}</span>
                    <KeyEditor
                      shortcut={shortcut}
                      onUpdate={updateShortcut}
                      onReset={resetShortcut}
                      isConflict={
                        [...conflicts.values()].some(ids => ids.includes(shortcut.id))
                      }
                    />
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        <div className="shortcuts-footer">
          <span className="shortcuts-footer-hint">
            {t('shortcuts.pressKey')} <kbd>?</kbd> {t('shortcuts.toToggle')}
          </span>
          {hasOverrides && (
            <button className="shortcuts-reset-all" onClick={resetAll}>
              {t('shortcuts.resetAll')}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
