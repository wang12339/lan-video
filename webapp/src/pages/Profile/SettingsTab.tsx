import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { sendVerificationEmail, updateEmail } from '../../api'
import { useQueryClient } from '@tanstack/react-query'

interface Props {
  autoPlay: boolean
  speedMem: boolean
  onAutoPlayChange: (checked: boolean) => void
  onSpeedMemChange: (checked: boolean) => void
  onLogout: () => void
  onAlert: (msg: string) => void
}

export default function SettingsTab({
  autoPlay, speedMem, onAutoPlayChange, onSpeedMemChange, onLogout, onAlert,
}: Props) {
  const { user, setUser } = useAuth()
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [sendingVerification, setSendingVerification] = useState(false)
  const [editingEmail, setEditingEmail] = useState(false)
  const [emailValue, setEmailValue] = useState('')
  const [savingEmail, setSavingEmail] = useState(false)

  const handleSendVerification = useCallback(async () => {
    setSendingVerification(true)
    try {
      const res = await sendVerificationEmail()
      onAlert(res.message || t('profile.verifySent'))
    } catch (err) {
      onAlert(err instanceof Error ? err.message : t('profile.sendFailed'))
    } finally {
      setSendingVerification(false)
    }
  }, [onAlert, t])

  const handleSaveEmail = useCallback(async () => {
    const email = emailValue.trim().toLowerCase()
    if (!email || !email.includes('@')) { onAlert(t('auth.validation.emailInvalid')); return }
    setSavingEmail(true)
    try {
      await updateEmail(email)
      if (setUser && user) {
        setUser({ ...user, email, emailVerified: false })
        queryClient.invalidateQueries({ queryKey: ['user-profile'] })
      }
      setEditingEmail(false)
      onAlert(t('profile.emailUpdated'))
    } catch (err) {
      onAlert(err instanceof Error ? err.message : t('common.saveFailed'))
    } finally {
      setSavingEmail(false)
    }
  }, [emailValue, onAlert, queryClient, setUser, user, t])

  if (!user) return null

  return (
    <div className="profile-content active" role="tabpanel">
      <div className="settings-section">
        <h3 className="settings-title">{t('profile.settingsPlayback')}</h3>
        <div className="settings-group">
          <div className="settings-row">
            <div>
              <span className="settings-label">{t('profile.autoPlay')}</span>
              <span className="settings-desc">{t('profile.autoPlayDesc')}</span>
            </div>
            <label className="toggle">
              <input type="checkbox" checked={autoPlay} onChange={(e) => onAutoPlayChange(e.target.checked)} />
              <span className="toggle-track" aria-hidden="true" />
            </label>
          </div>
          <div className="settings-row">
            <div>
              <span className="settings-label">{t('profile.speedMem')}</span>
              <span className="settings-desc">{t('profile.speedMemDesc')}</span>
            </div>
            <label className="toggle">
              <input type="checkbox" checked={speedMem} onChange={(e) => onSpeedMemChange(e.target.checked)} />
              <span className="toggle-track" aria-hidden="true" />
            </label>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-title">{t('profile.account')}</h3>
        <div className="settings-group">
          <div className="settings-row">
            <div>
              <span className="settings-label">{t('auth.username')}</span>
              <span className="settings-desc">{user.username}</span>
            </div>
            <span className="settings-value">{user.isAdmin ? t('profile.admin') : t('profile.normalUser')}</span>
          </div>
          <div className="settings-row">
            <div>
              <span className="settings-label">{t('auth.email')}</span>
              {editingEmail ? (
                <div className="email-edit">
                  <input
                    type="email"
                    className="email-input"
                    placeholder={t('auth.email')}
                    value={emailValue}
                    onChange={(e) => setEmailValue(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') handleSaveEmail() }}
                    autoFocus
                  />
                  <div className="email-actions">
                    <button className="profile-btn" onClick={handleSaveEmail} disabled={savingEmail}>{savingEmail ? t('common.saving') : t('common.save')}</button>
                    <button className="profile-btn-secondary" onClick={() => setEditingEmail(false)}>{t('common.cancel')}</button>
                  </div>
                </div>
              ) : (
                <span className="settings-desc">{user.email || t('profile.notSet')}</span>
              )}
            </div>
            {!editingEmail && (
              <div className="email-actions">
                {user.email ? (
                  <>
                    {user.emailVerified ? (
                      <span className="settings-value" style={{ color: 'var(--green)' }}>{t('profile.emailVerified')}</span>
                    ) : (
                      <button className="profile-btn" onClick={handleSendVerification} disabled={sendingVerification}>
                        {sendingVerification ? t('common.sending') : t('profile.verify')}
                      </button>
                    )}
                    <button className="profile-btn-secondary" onClick={() => { setEmailValue(user.email || ''); setEditingEmail(true) }}>
                      {t('profile.modify')}
                    </button>
                  </>
                ) : (
                  <button className="profile-btn" onClick={() => { setEmailValue(''); setEditingEmail(true) }}>
                    {t('profile.bindEmail')}
                  </button>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      <button className="settings-logout" onClick={onLogout}>{t('profile.logout')}</button>
    </div>
  )
}
