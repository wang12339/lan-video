import { useTheme } from '../../hooks/useTheme'
import './ThemeToggle.css'

export default function ThemeToggle() {
  const { resolvedTheme, toggleTheme } = useTheme()

  return (
    <button
      className="theme-toggle"
      onClick={toggleTheme}
      aria-label={resolvedTheme === 'dark' ? '切换到浅色模式' : '切换到深色模式'}
      title={resolvedTheme === 'dark' ? '切换到浅色模式' : '切换到深色模式'}
    >
      <span className="theme-toggle-icon">
        {resolvedTheme === 'dark' ? '☀️' : '🌙'}
      </span>
    </button>
  )
}
