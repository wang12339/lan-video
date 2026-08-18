import { useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import './KeyboardShortcuts.css'

interface KeyboardShortcutsProps {
  visible: boolean
  onClose: () => void
}

interface ShortcutItem {
  action: string
  keys: string[]
}

interface ShortcutSection {
  title: string
  shortcuts: ShortcutItem[]
}

export default function KeyboardShortcuts({ visible, onClose }: KeyboardShortcutsProps) {
  const { t } = useTranslation()

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

  const sections: ShortcutSection[] = [
    {
      title: t('shortcuts.playback'),
      shortcuts: [
        { action: t('shortcuts.playPause'), keys: ['Space'] },
        { action: t('shortcuts.mute'), keys: ['M'] },
        { action: t('shortcuts.fullscreen'), keys: ['F'] },
        { action: t('shortcuts.skipForward'), keys: ['→'] },
        { action: t('shortcuts.skipBackward'), keys: ['←'] },
        { action: t('shortcuts.volumeUp'), keys: ['↑'] },
        { action: t('shortcuts.volumeDown'), keys: ['↓'] },
        { action: t('shortcuts.speedUp'), keys: ['>'] },
        { action: t('shortcuts.speedDown'), keys: ['<'] },
      ],
    },
    {
      title: t('shortcuts.navigation'),
      shortcuts: [
        { action: t('shortcuts.search'), keys: ['/'] },
        { action: t('shortcuts.home'), keys: ['H'] },
        { action: t('shortcuts.gallery'), keys: ['G'] },
        { action: t('shortcuts.upload'), keys: ['U'] },
        { action: t('shortcuts.profile'), keys: ['P'] },
        { action: t('shortcuts.showShortcuts'), keys: ['?'] },
      ],
    },
    {
      title: t('shortcuts.general'),
      shortcuts: [
        { action: t('shortcuts.closeDialog'), keys: ['Esc'] },
        { action: t('shortcuts.scrollToTop'), keys: ['Home'] },
        { action: t('shortcuts.scrollToBottom'), keys: ['End'] },
      ],
    },
  ]

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

        <div className="shortcuts-content">
          {sections.map((section) => (
            <div key={section.title} className="shortcuts-section">
              <h3 className="shortcuts-section-title">{section.title}</h3>
              <div className="shortcuts-list">
                {section.shortcuts.map((shortcut) => (
                  <div key={shortcut.action} className="shortcuts-item">
                    <span className="shortcuts-action">{shortcut.action}</span>
                    <span className="shortcuts-keys">
                      {shortcut.keys.map((key, index) => (
                        <span key={key}>
                          <kbd className="shortcuts-key">{key}</kbd>
                          {index < shortcut.keys.length - 1 && (
                            <span className="shortcuts-plus">+</span>
                          )}
                        </span>
                      ))}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        <div className="shortcuts-footer">
          {t('shortcuts.pressKey')} <kbd>?</kbd> {t('shortcuts.toToggle')}
        </div>
      </div>
    </div>
  )
}

/**
 * Hook to manage keyboard shortcuts help panel
 */
export function useShortcutsHelp() {
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    // Don't trigger shortcuts when typing in inputs
    const target = e.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      return
    }

    // ? to toggle shortcuts help
    if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault()
      return { type: 'toggle' as const }
    }

    return null
  }, [])

  return { handleKeyDown }
}
