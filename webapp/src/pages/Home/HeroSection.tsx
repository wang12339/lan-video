import React, { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { MappedVideo } from '../../api/types'
import VideoCard from '../../components/VideoCard/VideoCard'

const VideoCardMemo = React.memo(VideoCard)

const HOT_SEARCH_TAGS = ['tech', 'tutorial', 'music', 'design'] as const

interface HeroSectionProps {
  trending: MappedVideo[]
}

function HeroSection({ trending }: HeroSectionProps) {
  const { t } = useTranslation()

  const trendingSliced = useMemo(() => trending.slice(0, 3), [trending])

  return (
    <>
      <div className="hero">
        <div className="hero-particles" aria-hidden="true">
          <span /><span /><span /><span /><span />
        </div>
        <h1 className="hero-title">{t('home.heroTitle')}</h1>
        <p className="hero-sub">{t('home.heroSub')}</p>
        <p className="hero-desc">{t('home.heroDesc')}</p>
        <div className="hero-features">
          <div className="hero-feature" title={t('home.featureHlsDesc')}>
            <span className="hero-feature-icon">⚡</span>
            <div className="hero-feature-text">
              <span className="hero-feature-name">{t('home.featureHls')}</span>
              <span className="hero-feature-desc">{t('home.featureHlsDesc')}</span>
            </div>
          </div>
          <div className="hero-feature" title={t('home.featureUploadDesc')}>
            <span className="hero-feature-icon">📦</span>
            <div className="hero-feature-text">
              <span className="hero-feature-name">{t('home.featureUpload')}</span>
              <span className="hero-feature-desc">{t('home.featureUploadDesc')}</span>
            </div>
          </div>
          <div className="hero-feature" title={t('home.featureShareDesc')}>
            <span className="hero-feature-icon">🔒</span>
            <div className="hero-feature-text">
              <span className="hero-feature-name">{t('home.featureShare')}</span>
              <span className="hero-feature-desc">{t('home.featureShareDesc')}</span>
            </div>
          </div>
          <div className="hero-feature" title={t('home.featurePrivateDesc')}>
            <span className="hero-feature-icon">🏠</span>
            <div className="hero-feature-text">
              <span className="hero-feature-name">{t('home.featurePrivate')}</span>
              <span className="hero-feature-desc">{t('home.featurePrivateDesc')}</span>
            </div>
          </div>
        </div>
        <div className="hero-stats">
          <div className="hero-stat">
            <span className="hero-stat-value">HLS</span>
            <span className="hero-stat-label">{t('home.featureHls')}</span>
          </div>
          <div className="hero-stat">
            <span className="hero-stat-value">4K</span>
            <span className="hero-stat-label">Max</span>
          </div>
          <div className="hero-stat">
            <span className="hero-stat-value">∞</span>
            <span className="hero-stat-label">{t('home.featureUpload')}</span>
          </div>
        </div>
        <div className="hero-steps">
          <div className="hero-step">
            <span className="hero-step-num">1</span>
            <span className="hero-step-text">{t('home.step1')}</span>
          </div>
          <div className="hero-step-arrow" aria-hidden="true">→</div>
          <div className="hero-step">
            <span className="hero-step-num">2</span>
            <span className="hero-step-text">{t('home.step2')}</span>
          </div>
          <div className="hero-step-arrow" aria-hidden="true">→</div>
          <div className="hero-step">
            <span className="hero-step-num">3</span>
            <span className="hero-step-text">{t('home.step3')}</span>
          </div>
        </div>
        <div className="hero-scroll-hint" aria-hidden="true">
          <span className="hero-scroll-arrow">↓</span>
        </div>
      </div>

      <div className="trending-searches">
        <span className="trending-searches-label">{t('home.hotSearch')}</span>
        <div className="trending-search-tags">
          {HOT_SEARCH_TAGS.map(tag => (
            <Link key={tag} to={`/?q=${encodeURIComponent(t('home.hotSearchTags.' + tag))}`} className="trending-search-tag">
              {t('home.hotSearchTags.' + tag)}
            </Link>
          ))}
        </div>
      </div>

      {trendingSliced.length > 0 && (
        <section className="trending-section guest-preview" aria-label={t('home.trending')}>
          <h2 className="trending-title">{t('home.trending')} · {t('home.guestPreview')}</h2>
          <div className="video-grid">
            {trendingSliced.map((video, i) => (
              <div key={`guest-${video.id}`} style={{ '--card-index': i } as React.CSSProperties}>
                <VideoCardMemo video={video} eager={i < 2} />
              </div>
            ))}
          </div>
          <div className="guest-cta-wrap">
            <Link to="/profile" className="empty-cta">{t('home.guestCta')} →</Link>
          </div>
        </section>
      )}
    </>
  )
}

export default React.memo(HeroSection)
