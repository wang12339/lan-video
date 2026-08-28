import { useState, useEffect, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router-dom'
import { useAuth } from '../../context/AuthContext'
import { forgotPassword, resetPassword } from '../../api'
import { verifyEmail } from '../../api/auth'
import { useFocusTrap } from '../../hooks/useFocusTrap'
import './AuthDialog.css'

interface AuthDialogProps {
  onClose?: () => void;
  closable?: boolean;
}

type Mode = 'login' | 'register' | 'forgot' | 'reset' | 'verify'
type FieldName = 'username' | 'email' | 'password' | 'token'

type FieldErrors = Partial<Record<FieldName, string>>

const USERNAME_MIN = 2
const USERNAME_MAX = 64
const PASSWORD_MIN = 8
const PASSWORD_MAX = 128
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
// 与后端 char::is_control 对齐（换行等控制字符）
const CONTROL_RE = /[\p{Cc}]/u
const MAX_TOKEN_LENGTH = 256

function countPasswordCategories(pw: string): number {
  let upper = 0
  let lower = 0
  let digit = 0
  let special = 0
  for (const ch of pw) {
    if (/[0-9]/.test(ch)) {
      digit = 1
    } else if (/[\p{Lu}]/u.test(ch)) {
      upper = 1
    } else if (/[\p{Ll}]/u.test(ch)) {
      lower = 1
    } else if (!/[\p{L}\p{N}]/u.test(ch)) {
      special = 1
    }
  }
  return upper + lower + digit + special
}

// 与后端 is_password_strong_enough 对齐：<12 位需至少 3 类，>=12 位需至少 2 类
function isPasswordStrongEnough(pw: string): boolean {
  return pw.length < 12 ? countPasswordCategories(pw) >= 3 : countPasswordCategories(pw) >= 2
}

export default function AuthDialog({ onClose, closable = true }: AuthDialogProps) {
  const { t } = useTranslation()
  const { login, register, kickedMsg, clearKickedMsg } = useAuth()
  const [searchParams] = useSearchParams()
  const resetTokenFromUrl = searchParams.get('reset_token')
  const verifyTokenFromUrl = searchParams.get('verify_token')

  const [mode, setMode] = useState<Mode>(
    resetTokenFromUrl ? 'reset' : verifyTokenFromUrl ? 'verify' : 'login'
  )
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [resetToken, setResetToken] = useState(resetTokenFromUrl || '')
  const [showPassword, setShowPassword] = useState(false)
  const [touched, setTouched] = useState<Partial<Record<FieldName, boolean>>>({})
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>({})
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')
  const [loading, setLoading] = useState(false)
  const [verifyState, setVerifyState] = useState<'verifying' | 'ok' | 'failed'>(() =>
    mode === 'verify' ? 'verifying' : 'ok'
  )
  const submitLockRef = useRef(false)
  const switchTimerRef = useRef<ReturnType<typeof setTimeout>>()
  const dialogRef = useRef<HTMLDivElement>(null)

  const validateForm = useCallback((m: Mode, values: {
    username: string; email: string; password: string; token: string;
  }): FieldErrors => {
    const errs: FieldErrors = {}
    const u = values.username.trim()
    const e = values.email.trim()
    const pw = values.password
    const tok = values.token.trim()

    if (m === 'login') {
      if (!u) errs.username = t('auth.validation.usernameRequired')
      if (!pw) errs.password = t('auth.validation.passwordRequired')
    } else if (m === 'register') {
      if (!u) errs.username = t('auth.validation.usernameRequired')
      else if (u.length < USERNAME_MIN || u.length > USERNAME_MAX) errs.username = t('auth.validation.usernameLength')
      else if (CONTROL_RE.test(u)) errs.username = t('auth.validation.usernameIllegal')
      if (!pw) errs.password = t('auth.validation.passwordRequired')
      else if (pw.length < PASSWORD_MIN || pw.length > PASSWORD_MAX) errs.password = t('auth.validation.passwordLength')
      else if (!isPasswordStrongEnough(pw)) errs.password = t('auth.validation.passwordStrength')
    } else if (m === 'forgot') {
      if (!e) errs.email = t('auth.validation.emailRequired')
      else if (!EMAIL_RE.test(e)) errs.email = t('auth.validation.emailInvalid')
    } else if (m === 'reset') {
      if (!tok) errs.token = t('auth.validation.tokenRequired')
      if (!pw) errs.password = t('auth.validation.passwordRequired')
      else if (pw.length < PASSWORD_MIN || pw.length > PASSWORD_MAX) errs.password = t('auth.validation.passwordLength')
      else if (!isPasswordStrongEnough(pw)) errs.password = t('auth.validation.passwordStrength')
    }
    return errs
  }, [t])

  const values = { username, email, password, token: resetToken }

  const sanitizeUsername = (v: string) => v.replace(CONTROL_RE, '').slice(0, USERNAME_MAX)
  const sanitizeToken = (v: string) => v.replace(/[^\w\-]/g, '').slice(0, MAX_TOKEN_LENGTH)

  const handleFieldChange = (field: FieldName, value: string, setter: (v: string) => void) => {
    setter(value)
    if (!touched[field]) return
    const errs = validateForm(mode, { ...values, [field]: value })
    setFieldErrors(prev => ({ ...prev, [field]: errs[field] }))
  }

  const handleFieldBlur = (field: FieldName) => {
    setTouched(prev => ({ ...prev, [field]: true }))
    const errs = validateForm(mode, values)
    setFieldErrors(prev => ({ ...prev, [field]: errs[field] }))
  }

  const switchMode = (next: Mode, clearPassword = false) => {
    setMode(next)
    setError('')
    setSuccess('')
    setTouched({})
    setFieldErrors({})
    if (clearPassword) setPassword('')
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (submitLockRef.current || loading) return
    const errs = validateForm(mode, values)
    setFieldErrors(errs)
    if (Object.keys(errs).length > 0) {
      const first = (['username', 'email', 'password', 'token'] as FieldName[]).find(f => errs[f])
      if (first) {
        document.getElementById(`auth-field-${first}`)?.focus()
      }
      return
    }
    submitLockRef.current = true
    setLoading(true)
    setError('')
    setSuccess('')
    const safeUsername = sanitizeUsername(username)
    const safeToken = sanitizeToken(resetToken)
    try {
      if (mode === 'login') {
        await login(safeUsername, password)
        onClose?.()
      } else if (mode === 'register') {
        const msg = await register(safeUsername, password)
        if (msg) {
          setSuccess(msg)
        } else {
          onClose?.()
        }
      } else if (mode === 'forgot') {
        const res = await forgotPassword(email.trim())
        setSuccess(res.message || t('auth.forgotSent'))
      } else if (mode === 'reset') {
        const res = await resetPassword(safeToken, password)
        setSuccess(res.message || t('auth.resetDone'))
        switchTimerRef.current = setTimeout(() => switchMode('login'), 2200)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('auth.error'))
    } finally {
      submitLockRef.current = false
      setLoading(false)
    }
  }

  useEffect(() => {
    // 锁定背景滚动，关闭时恢复原值
    const prevOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'

    return () => {
      document.body.style.overflow = prevOverflow
      if (switchTimerRef.current) clearTimeout(switchTimerRef.current)
    }
  }, [])

  // 焦点陷阱
  useFocusTrap(dialogRef, true)

  useEffect(() => {
    if (!closable) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose?.()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [closable, onClose])

  useEffect(() => {
    if (mode !== 'verify' || !verifyTokenFromUrl) return
    let cancelled = false
    setVerifyState('verifying')
    verifyEmail(verifyTokenFromUrl)
      .then(() => { if (!cancelled) setVerifyState('ok') })
      .catch(() => { if (!cancelled) setVerifyState('failed') })
    return () => { cancelled = true }
  }, [mode, verifyTokenFromUrl])

  const renderError = (field: FieldName) =>
    fieldErrors[field] ? (
      <p className="auth-field-error" role="alert">{fieldErrors[field]}</p>
    ) : (
      <p className="auth-field-error" aria-hidden="true">&nbsp;</p>
    )

  const title =
    mode === 'login' ? t('auth.login') :
    mode === 'register' ? t('auth.register') :
    mode === 'forgot' ? t('auth.forgotTitle') :
    mode === 'reset' ? t('auth.resetTitle') :
    t('auth.verifyTitle')

  const submitLabel =
    mode === 'forgot' ? t('auth.forgotSubmit') :
    mode === 'reset' ? t('auth.resetSubmit') :
    t('auth.submit')

  return (
    <div
      className="auth-overlay"
      onClick={(e) => {
        if (closable && e.target === e.currentTarget) onClose?.()
      }}
    >
      <div
        ref={dialogRef}
        className="auth-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="auth-dialog-title"
        onClick={(e) => e.stopPropagation()}
      >
        {closable && (
          <button
            type="button"
            className="auth-close"
            aria-label={t('auth.closeDialog')}
            onClick={onClose}
          >
            ✕
          </button>
        )}
        {kickedMsg && (
          <div className="auth-kicked-banner" role="alert">
            {kickedMsg}
            <button className="auth-kicked-close" aria-label={t('auth.closeDialog')} onClick={clearKickedMsg}>&times;</button>
          </div>
        )}
        <h2 id="auth-dialog-title" className="auth-title">{title}</h2>

        {(mode === 'login' || mode === 'register') && (
          <div className="auth-tabs" role="tablist" aria-label={title}>
            <button
              type="button"
              className={`auth-tab ${mode === 'login' ? 'active' : ''}`}
              role="tab"
              id="auth-tab-login"
              aria-selected={mode === 'login'}
              aria-controls="auth-panel"
              tabIndex={mode === 'login' ? 0 : -1}
              onClick={() => switchMode('login')}
            >
              {t('auth.login')}
            </button>
            <button
              type="button"
              className={`auth-tab ${mode === 'register' ? 'active' : ''}`}
              role="tab"
              id="auth-tab-register"
              aria-selected={mode === 'register'}
              aria-controls="auth-panel"
              tabIndex={mode === 'register' ? 0 : -1}
              onClick={() => switchMode('register')}
            >
              {t('auth.register')}
            </button>
          </div>
        )}

        {mode === 'verify' ? (
          <div className="auth-verify" role="status">
            {verifyState === 'verifying' && <div className="auth-verify-spinner" aria-hidden="true" />}
            {verifyState === 'verifying' && <p>{t('auth.verifying')}</p>}
            {verifyState === 'ok' && (
              <>
                <div className="auth-verify-icon ok" aria-hidden="true">✓</div>
                <p>{t('auth.verifySuccess')}</p>
              </>
            )}
            {verifyState === 'failed' && (
              <>
                <div className="auth-verify-icon fail" aria-hidden="true">✕</div>
                <p>{t('auth.verifyInvalid')}</p>
              </>
            )}
            {verifyState !== 'verifying' && (
              <button
                type="button"
                className="auth-btn"
                onClick={() => switchMode('login')}
              >
                {t('auth.verifySubmit')}
              </button>
            )}
          </div>
        ) : (
          <form className="auth-form" id="auth-panel" role="tabpanel" onSubmit={handleSubmit} noValidate>
            {mode === 'reset' ? (
              <>
                <label className="auth-label" htmlFor="auth-field-token">
                  <span className="auth-label-text">{t('auth.resetToken')}</span>
                  <input
                    id="auth-field-token"
                    className="auth-input"
                    type="text"
                    placeholder={t('auth.resetTokenPlaceholder')}
                    value={resetToken}
                    onChange={(e) => handleFieldChange('token', e.target.value, setResetToken)}
                    onBlur={() => handleFieldBlur('token')}
                    aria-invalid={!!fieldErrors.token}
                    aria-describedby="auth-token-error"
                    required
                    autoFocus
                    autoComplete="off"
                    maxLength={MAX_TOKEN_LENGTH}
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                  />
                </label>
                {renderError('token')}
                <label className="auth-label" htmlFor="auth-field-password">
                  <span className="auth-label-text">{t('auth.resetNewPassword')}</span>
                  <div className="auth-password-wrap">
                    <input
                      id="auth-field-password"
                      className="auth-input auth-password-input"
                      type={showPassword ? 'text' : 'password'}
                      placeholder={t('auth.resetNewPasswordPlaceholder')}
                      value={password}
                      onChange={(e) => handleFieldChange('password', e.target.value, setPassword)}
                      onBlur={() => handleFieldBlur('password')}
                      aria-invalid={!!fieldErrors.password}
                      aria-describedby="auth-password-error"
                      required
                      minLength={PASSWORD_MIN}
                      maxLength={PASSWORD_MAX}
                      autoComplete="new-password"
                    />
                    <button
                      type="button"
                      className="auth-password-toggle"
                      aria-label={showPassword ? t('auth.hidePassword') : t('auth.showPassword')}
                      onClick={() => setShowPassword(v => !v)}
                    >
                      {showPassword ? '🙈' : '👁'}
                    </button>
                  </div>
                </label>
                {renderError('password')}
              </>
            ) : mode === 'forgot' ? (
              <>
                <p className="auth-desc">{t('auth.forgotDesc')}</p>
                <label className="auth-label" htmlFor="auth-field-email">
                  <span className="auth-label-text">{t('auth.email')}</span>
                  <input
                    id="auth-field-email"
                    className="auth-input"
                    type="email"
                    placeholder={t('auth.forgotPlaceholder')}
                    value={email}
                    onChange={(e) => handleFieldChange('email', e.target.value, setEmail)}
                    onBlur={() => handleFieldBlur('email')}
                    aria-invalid={!!fieldErrors.email}
                    aria-describedby="auth-email-error"
                    required
                    autoFocus
                    autoComplete="email"
                    maxLength={254}
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                  />
                </label>
                {renderError('email')}
              </>
            ) : (
              <>
                <label className="auth-label" htmlFor="auth-field-username">
                  <span className="auth-label-text">{t('auth.username')}</span>
                  <input
                    id="auth-field-username"
                    className="auth-input"
                    type="text"
                    placeholder={t('auth.username')}
                    value={username}
                    onChange={(e) => handleFieldChange('username', e.target.value, setUsername)}
                    onBlur={() => handleFieldBlur('username')}
                    aria-invalid={!!fieldErrors.username}
                    aria-describedby="auth-username-error"
                    required
                    autoFocus
                    autoComplete="username"
                    maxLength={USERNAME_MAX}
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                  />
                </label>
                {renderError('username')}
                <label className="auth-label" htmlFor="auth-field-login-password">
                  <span className="auth-label-text">{t('auth.password')}</span>
                  <div className="auth-password-wrap">
                    <input
                      id="auth-field-login-password"
                      className="auth-input auth-password-input"
                      type={showPassword ? 'text' : 'password'}
                      placeholder={t('auth.password')}
                      value={password}
                      onChange={(e) => handleFieldChange('password', e.target.value, setPassword)}
                      onBlur={() => handleFieldBlur('password')}
                      aria-invalid={!!fieldErrors.password}
                      aria-describedby="auth-password-error"
                      required
                      autoComplete={mode === 'register' ? 'new-password' : 'current-password'}
                      minLength={mode === 'register' ? PASSWORD_MIN : undefined}
                      maxLength={PASSWORD_MAX}
                    />
                    <button
                      type="button"
                      className="auth-password-toggle"
                      aria-label={showPassword ? t('auth.hidePassword') : t('auth.showPassword')}
                      onClick={() => setShowPassword(v => !v)}
                    >
                      {showPassword ? '🙈' : '👁'}
                    </button>
                  </div>
                </label>
                {renderError('password')}
              </>
            )}
            {error && <div className="auth-error" role="alert">{error}</div>}
            {success && <div className="auth-success" role="status">{success}</div>}
            <button className="auth-btn" type="submit" disabled={loading}>
              {loading ? t('auth.submitting') : submitLabel}
            </button>
          </form>
        )}

        {mode === 'login' && (
          <div className="auth-links">
            <button type="button" className="auth-link" onClick={() => switchMode('forgot')}>
              {t('auth.forgotLink')}
            </button>
          </div>
        )}
        {(mode === 'forgot' || mode === 'reset') && (
          <div className="auth-links">
            <button
              type="button"
              className="auth-link"
              onClick={() => switchMode('login', true)}
            >
              {t('auth.backToLogin')}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
