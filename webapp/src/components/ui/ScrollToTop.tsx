import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import './ScrollToTop.css'

interface ScrollToTopProps {
  /** Show after scrolling this many pixels */
  threshold?: number
  /** Show progress ring based on scroll position */
  showProgress?: boolean
}

export default function ScrollToTop({
  threshold = 300,
  showProgress = true,
}: ScrollToTopProps) {
  const { t } = useTranslation()
  const [visible, setVisible] = useState(false)
  const [scrollPercent, setScrollPercent] = useState(0)

  const handleScroll = useCallback(() => {
    const scrollTop = window.scrollY || document.documentElement.scrollTop
    const scrollHeight = document.documentElement.scrollHeight - document.documentElement.clientHeight

    setVisible(scrollTop > threshold)

    if (scrollHeight > 0) {
      setScrollPercent(Math.min(100, (scrollTop / scrollHeight) * 100))
    }
  }, [threshold])

  useEffect(() => {
    window.addEventListener('scroll', handleScroll, { passive: true })
    return () => window.removeEventListener('scroll', handleScroll)
  }, [handleScroll])

  const scrollToTop = () => {
    window.scrollTo({
      top: 0,
      behavior: 'smooth',
    })
  }

  // Calculate progress ring values
  const radius = 20
  const circumference = 2 * Math.PI * radius
  const dashOffset = circumference - (scrollPercent / 100) * circumference

  return (
    <button
      className={`scroll-to-top ${visible ? 'visible' : ''}`}
      onClick={scrollToTop}
      aria-label={t('common.scrollToTop')}
      title={t('common.scrollToTop')}
    >
      {showProgress && (
        <svg className="scroll-to-top-progress" viewBox="0 0 48 48">
          <circle cx="24" cy="24" r={radius} />
          <circle
            className="progress-value"
            cx="24"
            cy="24"
            r={radius}
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
          />
        </svg>
      )}
      <svg
        width="20"
        height="20"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <polyline points="18,15 12,9 6,15" />
      </svg>
    </button>
  )
}
