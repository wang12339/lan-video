import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'

interface GuestLandingProps {
  onLogin: () => void
}

function GuestLanding({ onLogin }: GuestLandingProps) {
  const { t } = useTranslation()
  const { enterGuest } = useAuth()
  const [entering, setEntering] = useState(false)
  const [failed, setFailed] = useState(false)

  const handleGuestEnter = async () => {
    if (entering) return
    setEntering(true)
    setFailed(false)
    const ok = await enterGuest()
    setEntering(false)
    if (!ok) setFailed(true)
  }

  return (
    <div className="hero guest-landing">
      <h1 className="hero-title">{t('home.heroTitle')}</h1>
      <p className="hero-sub">{t('home.heroSub')}</p>
      <p className="hero-desc">{t('home.heroDesc')}</p>
      <button
        type="button"
        className="empty-cta guest-login-btn"
        onClick={handleGuestEnter}
        disabled={entering}
      >
        {entering ? t('home.guestEntering') : t('home.guestEnter')}
      </button>
      <button type="button" className="guest-register-link" onClick={onLogin}>
        {t('nav.loginRegister')}
      </button>
      <p className="guest-enter-hint">{t('home.guestEnterHint')}</p>
      {failed && (
        <p className="guest-enter-error" role="alert">{t('home.guestEnterFailed')}</p>
      )}
    </div>
  )
}

export default React.memo(GuestLanding)
