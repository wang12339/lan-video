import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import './AuthDialog.css'

interface AuthDialogProps {
  onClose?: () => void;
  closable?: boolean;
}

export default function AuthDialog({ onClose, closable = true }: AuthDialogProps) {
  const { t } = useTranslation()
  const { login, register, kickedMsg, clearKickedMsg } = useAuth()
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setSuccess('')
    setLoading(true)

    try {
      if (mode === 'login') {
        await login(username, password)
        onClose?.()
      } else {
        const msg = await register(username, password)
        if (msg) {
          setSuccess(msg)
        } else {
          onClose?.()
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('auth.error'))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="auth-overlay" onClick={closable ? onClose : undefined}>
      <div className="auth-dialog" onClick={(e) => e.stopPropagation()}>
        {kickedMsg && (
          <div className="auth-kicked-banner">
            {kickedMsg}
            <button className="auth-kicked-close" onClick={clearKickedMsg}>&times;</button>
          </div>
        )}
        <h2 className="auth-title">
          {mode === 'login' ? t('auth.login') : t('auth.register')}
        </h2>

        <div className="auth-tabs">
          <button
            className={`auth-tab ${mode === 'login' ? 'active' : ''}`}
            onClick={() => { setMode('login'); setError(''); setSuccess('') }}
          >
            {t('auth.login')}
          </button>
          <button
            className={`auth-tab ${mode === 'register' ? 'active' : ''}`}
            onClick={() => { setMode('register'); setError(''); setSuccess('') }}
          >
            {t('auth.register')}
          </button>
        </div>

        <form className="auth-form" onSubmit={handleSubmit}>
          <input
            className="auth-input"
            type="text"
            placeholder={t('auth.username')}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
            autoFocus
            autoComplete="username"
          />
          <input
            className="auth-input"
            type="password"
            placeholder={t('auth.password')}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoComplete="current-password"
          />
          {error && <div className="auth-error">{error}</div>}
          {success && <div className="auth-success">{success}</div>}
          <button className="auth-btn" type="submit" disabled={loading}>
            {loading ? t('auth.submitting') : t('auth.submit')}
          </button>
        </form>
      </div>
    </div>
  )
}
