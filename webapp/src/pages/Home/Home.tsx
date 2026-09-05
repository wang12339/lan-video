import { useState, useEffect, useRef, useMemo, useCallback, memo } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { useInfiniteScroll } from '../../hooks/useInfiniteScroll'
import AuthDialog from '../../components/AuthDialog/AuthDialog'
import { useHomeData } from './hooks/useHomeData'
import HeroSection from './HeroSection'
import CategoryFilter from './CategoryFilter'
import SearchBar from './SearchBar'
import RecentSection from './RecentSection'
import TrendingSection from './TrendingSection'
import VideoGrid from './VideoGrid'
import './Home.css'

let homeScrollY = 0

const ScrollTopButton = memo(function ScrollTopButton({ ariaLabel }: { ariaLabel: string }) {
  const scrollToTop = useCallback(() => {
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }, [])

  return (
    <button
      className="scroll-top-btn"
      onClick={scrollToTop}
      aria-label={ariaLabel}
    >
      ↑
    </button>
  )
})

export default function Home() {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [showAuth, setShowAuth] = useState(false)
  const [viewMode, setViewMode] = useState<'grid' | 'list'>(() =>
    (localStorage.getItem('home-view-mode') as 'grid' | 'list') || 'grid'
  )
  const [showScrollTop, setShowScrollTop] = useState(false)

  const {
    category,
    setCategory,
    query,
    emailVerified,
    searchParams,
    setSearchParams,
    data,
    isPending,
    isError,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
    refetch,
    trending,
    recentVideos,
    videos,
    total,
  } = useHomeData()

  const filteredVideos = videos

  // 返回首页时恢复滚动位置
  const restoredRef = useRef(false)
  useEffect(() => {
    if (restoredRef.current || !data) return
    restoredRef.current = true
    const y = homeScrollY
    if (y > 0) {
      requestAnimationFrame(() => {
        window.scrollTo({ top: y, behavior: 'instant' })
      })
    }
  }, [data])

  // 分类/关键词变化时回到顶部
  useEffect(() => {
    restoredRef.current = false
    homeScrollY = 0
    window.scrollTo({ top: 0, behavior: 'instant' })
  }, [category, query])

  // Unified scroll handler
  useEffect(() => {
    const onScroll = () => {
      homeScrollY = window.scrollY
      setShowScrollTop(window.scrollY > 400)
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  // Keyboard shortcut to focus search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        const searchInput = document.querySelector('.nav-search input') as HTMLInputElement
        searchInput?.focus()
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [])

  // Pause animations when tab is hidden to save CPU
  useEffect(() => {
    const handleVisibility = () => {
      document.documentElement.style.setProperty(
        '--animation-play-state',
        document.hidden ? 'paused' : 'running'
      )
    }
    document.addEventListener('visibilitychange', handleVisibility)
    return () => document.removeEventListener('visibilitychange', handleVisibility)
  }, [])

  // Arrow key navigation between video cards
  useEffect(() => {
    const handleArrowKeys = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return

      const cards = document.querySelectorAll('.video-card[tabindex="0"]') as NodeListOf<HTMLElement>
      const current = document.activeElement
      const currentIndex = Array.from(cards).indexOf(current as HTMLElement)

      if (currentIndex < 0) return

      let nextIndex = currentIndex
      if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
        nextIndex = Math.min(currentIndex + 1, cards.length - 1)
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
        nextIndex = Math.max(currentIndex - 1, 0)
      } else {
        return
      }

      if (nextIndex !== currentIndex) {
        e.preventDefault()
        cards[nextIndex]?.focus()
      }
    }

    document.addEventListener('keydown', handleArrowKeys)
    return () => document.removeEventListener('keydown', handleArrowKeys)
  }, [])

  // 底部哨兵
  const sentinelRef = useRef<HTMLDivElement>(null)
  useInfiniteScroll(sentinelRef, {
    hasMore: !!hasNextPage && filteredVideos.length > 0,
    loading: isFetchingNextPage,
    onLoadMore: fetchNextPage,
  })

  const structuredData = useMemo(() => ({
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    name: 'Atmos Video',
    description: t('home.heroDesc'),
    url: window.location.origin,
    potentialAction: {
      '@type': 'SearchAction',
      target: `${window.location.origin}/webapp/?q={search_term_string}`,
      'query-input': 'required name=search_term_string',
    },
  }), [t])

  const handleRetry = useCallback(() => refetch(), [refetch])
  const handleLoadMore = useCallback(() => fetchNextPage(), [fetchNextPage])

  return (
    <div className="home">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(structuredData) }}
      />

      <SearchBar isPending={!!user && isPending} emailVerified={emailVerified} />

      {!user && !query && <HeroSection trending={trending} />}

      {!user && query && (
        <div className="empty-state" role="status" aria-live="polite">
          <div className="empty-icon" aria-hidden="true">🔍</div>
          <div className="empty-text">{t('home.guestSearchTitle')}</div>
          <p className="empty-hint">{t('home.guestSearchDesc')}</p>
          <button className="empty-cta" onClick={() => setShowAuth(true)}>
            {t('nav.loginRegister')}
          </button>
        </div>
      )}

      {user && (
        <>
          <CategoryFilter
            category={category}
            total={total}
            viewMode={viewMode}
            onCategoryChange={setCategory}
            onViewModeChange={setViewMode}
            searchParams={searchParams}
            setSearchParams={setSearchParams}
          />

          {!query && <RecentSection recentVideos={recentVideos} viewMode={viewMode} />}

          {!query && <TrendingSection trending={trending} viewMode={viewMode} />}

          <VideoGrid
            videos={filteredVideos}
            viewMode={viewMode}
            isPending={isPending}
            isError={isError}
            hasNextPage={!!hasNextPage}
            isFetchingNextPage={isFetchingNextPage}
            onRetry={handleRetry}
            onLoadMore={handleLoadMore}
          />
        </>
      )}

      <div ref={sentinelRef} className="load-sentinel" aria-hidden="true" />

      {showScrollTop && <ScrollTopButton ariaLabel={t('common.scrollToTop')} />}

      {showAuth && !user && <AuthDialog onClose={() => setShowAuth(false)} />}
    </div>
  )
}
