import { useTheme } from '../../hooks/useTheme'
import { useTranslation } from 'react-i18next'
import './ThemeToggle.css'

import { memo } from 'react'

function ThemeToggleImpl() {
  const { resolvedTheme, toggleTheme } = useTheme()
  const { t } = useTranslation()
  const label = resolvedTheme === 'dark' ? t('nav.switchToLight') : t('nav.switchToDark')

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={toggleTheme}
      aria-label={label}
      title={label}
    >
      <span className="theme-toggle-icon" aria-hidden="true">
        {resolvedTheme === 'dark' ? '☀️' : '🌙'}
      </span>
    </button>
  )
}

export default memo(ThemeToggleImpl)
