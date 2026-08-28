import { useTheme } from '../../hooks/useTheme'
import './ThemeToggle.css'

import { memo } from 'react'

function ThemeToggleImpl() {
  const { resolvedTheme, toggleTheme } = useTheme()

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={toggleTheme}
      aria-label={resolvedTheme === 'dark' ? '切换到浅色模式' : '切换到深色模式'}
      title={resolvedTheme === 'dark' ? '切换到浅色模式' : '切换到深色模式'}
    >
      <span className="theme-toggle-icon" aria-hidden="true">
        {resolvedTheme === 'dark' ? '☀️' : '🌙'}
      </span>
    </button>
  )
}

export default memo(ThemeToggleImpl)
