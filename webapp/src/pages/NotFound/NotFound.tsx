import { memo } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router-dom'
import './NotFound.css'

function NotFound() {
  const { t } = useTranslation()
  return (
    <div className="not-found">
      {/* Background decorations */}
      <div className="not-found__bg">
        <div className="not-found__orb not-found__orb--1" />
        <div className="not-found__orb not-found__orb--2" />
        <div className="not-found__orb not-found__orb--3" />
        <div className="not-found__grid" />
      </div>

      {/* Floating particles */}
      <div className="not-found__particles">
        {Array.from({ length: 12 }, (_, i) => (
          <span key={i} className={`not-found__particle not-found__particle--${i + 1}`} />
        ))}
      </div>

      {/* Main content */}
      <div className="not-found__content">
        {/* 404 visual */}
        <div className="not-found__visual">
          <span className="not-found__digit not-found__digit--4a">4</span>
          <span className="not-found__digit not-found__digit--0">
            <span className="not-found__zero-ring" />
            <span className="not-found__zero-ring not-found__zero-ring--inner" />
          </span>
          <span className="not-found__digit not-found__digit--4b">4</span>
        </div>

        {/* Glitch overlay for 404 */}
        <div className="not-found__glitch" aria-hidden="true">
          <span>4</span><span>0</span><span>4</span>
        </div>

        <p className="not-found__message">{t('notFound.message')}</p>

        <Link to="/" className="not-found__btn">
          <span className="not-found__btn-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
            </svg>
          </span>
          <span>{t('notFound.backHome')}</span>
          <span className="not-found__btn-arrow">→</span>
        </Link>
      </div>
    </div>
  )
}

export default memo(NotFound)
