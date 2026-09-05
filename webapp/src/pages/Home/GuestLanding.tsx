import React from 'react'
import { useTranslation } from 'react-i18next'

interface GuestLandingProps {
  onLogin: () => void
}

function GuestLanding({ onLogin }: GuestLandingProps) {
  const { t } = useTranslation()

  return (
    <div className="hero guest-landing">
      <h1 className="hero-title">{t('home.heroTitle')}</h1>
      <p className="hero-sub">{t('home.heroSub')}</p>
      <p className="hero-desc">{t('home.heroDesc')}</p>
      <button type="button" className="empty-cta guest-login-btn" onClick={onLogin}>
        {t('nav.loginRegister')}
      </button>
    </div>
  )
}

export default React.memo(GuestLanding)
