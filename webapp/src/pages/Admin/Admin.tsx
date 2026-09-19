import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useAuth } from '../../context/AuthContext'
import { ErrorBoundary } from './components'
import './Admin.css'

const DashboardTab = lazy(() => import('./DashboardTab'))
const VideosTab = lazy(() => import('./VideosTab'))
const UsersTab = lazy(() => import('./UsersTab'))
const TagsTab = lazy(() => import('./TagsTab'))
const SystemTab = lazy(() => import('./SystemTab'))
const LogsTab = lazy(() => import('./LogsTab'))
const ChatTab = lazy(() => import('./ChatTab'))

type Tab = 'dashboard' | 'videos' | 'users' | 'tags' | 'system' | 'logs' | 'chat'
type MediaSubTab = 'video' | 'image'

const TABS: { key: Tab; icon: string }[] = [
  { key: 'dashboard', icon: '📊' },
  { key: 'videos', icon: '🎬' },
  { key: 'users', icon: '👥' },
  { key: 'tags', icon: '🏷️' },
  { key: 'system', icon: '⚙️' },
  { key: 'logs', icon: '📋' },
  { key: 'chat', icon: '💬' },
]

const MEDIA_SUB_TABS: { key: MediaSubTab; sourceType: string }[] = [
  { key: 'video', sourceType: 'local_video' },
  { key: 'image', sourceType: 'local_image' },
]

export default function Admin() {
  const { t } = useTranslation()
  const { user, loading } = useAuth()
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  // 支持 /admin?tab=users 深链（导航待审批徽标跳转用）
  const [tab, setTab] = useState<Tab>(() => {
    const q = searchParams.get('tab')
    return TABS.some((x) => x.key === q) ? (q as Tab) : 'dashboard'
  })
  const [mediaTab, setMediaTab] = useState<MediaSubTab>('video')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const tabRefs = useRef<Partial<Record<Tab, HTMLButtonElement | null>>>({})

  // URL -> state：覆盖浏览器前进/后退以及同路由深链跳转，非法 tab 回退 dashboard
  useEffect(() => {
    const q = searchParams.get('tab')
    const next: Tab = TABS.some((x) => x.key === q) ? (q as Tab) : 'dashboard'
    setTab((prev) => (prev === next ? prev : next))
  }, [searchParams])

  // state -> URL：replace 写回并保留其他查询参数（不新增历史记录）
  const selectTab = useCallback((key: Tab) => {
    setTab((prev) => (prev === key ? prev : key))
    setSearchParams((prev) => {
      if (prev.get('tab') === key) return prev
      const next = new URLSearchParams(prev)
      next.set('tab', key)
      return next
    }, { replace: true })
  }, [setSearchParams])

  // 侧边栏为纵向排列，方向键优先 Up/Down（同时兼容 Left/Right）；Home/End 跳首尾，选择跟随焦点
  const handleTabsKeyDown = useCallback((event: React.KeyboardEvent<HTMLElement>) => {
    const { key } = event
    const current = TABS.findIndex((x) => x.key === tab)
    let nextIndex: number
    if (key === 'ArrowDown' || key === 'ArrowRight') nextIndex = (current + 1) % TABS.length
    else if (key === 'ArrowUp' || key === 'ArrowLeft') nextIndex = (current - 1 + TABS.length) % TABS.length
    else if (key === 'Home') nextIndex = 0
    else if (key === 'End') nextIndex = TABS.length - 1
    else return
    event.preventDefault()
    const nextKey = TABS[nextIndex]!.key
    selectTab(nextKey)
    tabRefs.current[nextKey]?.focus()
  }, [tab, selectTab])

  // 会话尚未恢复完成时先显示加载态，避免管理员页面出现"无权限"闪屏
  if (loading) {
    return (
      <div className="admin-page">
        <div className="admin-loading">
          <div className="admin-loading-spinner" />
          {t('common.loading')}
        </div>
      </div>
    )
  }

  if (!user?.isAdmin) {
    return (
      <div className="admin-page">
        <div className="admin-denied">
          <div className="admin-denied-icon"><span aria-hidden="true">🔒</span></div>
          <p>{t('admin.denied')}</p>
          <button type="button" className="admin-btn admin-btn-primary" onClick={() => navigate('/')}>
            {t('player.backToHome')}
          </button>
        </div>
      </div>
    )
  }

  const currentSourceType = MEDIA_SUB_TABS.find(x => x.key === mediaTab)?.sourceType ?? 'local_video'

  return (
    <div className={`admin-page ${sidebarCollapsed ? 'admin-sidebar-collapsed' : ''}`}>
      <aside className="admin-sidebar" role="navigation" aria-label={t('admin.title')}>
        <div className="admin-sidebar-header">
          <div className="admin-sidebar-logo">
            <span className="admin-sidebar-logo-icon">⚡</span>
            {!sidebarCollapsed && <span className="admin-sidebar-logo-text">{t('admin.title')}</span>}
          </div>
          <button
            type="button"
            className="admin-sidebar-toggle"
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            aria-label={sidebarCollapsed ? t('admin.sidebar.expand') : t('admin.sidebar.collapse')}
          >
            {sidebarCollapsed ? '»' : '«'}
          </button>
        </div>
        <nav className="admin-sidebar-nav" role="tablist" aria-orientation="vertical" onKeyDown={handleTabsKeyDown}>
          {TABS.map(({ key, icon }) => (
            <button
              key={key}
              ref={(el) => { tabRefs.current[key] = el }}
              type="button"
              role="tab"
              id={`admin-tab-${key}`}
              aria-selected={tab === key}
              aria-controls={`admin-panel-${key}`}
              tabIndex={tab === key ? 0 : -1}
              className={`admin-sidebar-item ${tab === key ? 'active' : ''}`}
              onClick={() => selectTab(key)}
              title={t(`admin.tabs.${key}`)}
            >
              <span className="admin-sidebar-icon">{icon}</span>
              {!sidebarCollapsed && <span className="admin-sidebar-label">{t(`admin.tabs.${key}`)}</span>}
              {tab === key && <span className="admin-sidebar-indicator" />}
            </button>
          ))}
        </nav>
        <div className="admin-sidebar-footer">
          <button
            type="button"
            className="admin-sidebar-item admin-sidebar-back"
            onClick={() => navigate('/')}
            title={t('player.backToHome')}
          >
            <span className="admin-sidebar-icon">🏠</span>
            {!sidebarCollapsed && <span className="admin-sidebar-label">{t('player.backToHome')}</span>}
          </button>
        </div>
      </aside>
      <main className="admin-content">
        <header className="admin-content-header">
          <h1 className="admin-title">
            <span className="admin-title-icon">{TABS.find(t => t.key === tab)?.icon}</span>
            {t(`admin.tabs.${tab}`)}
          </h1>
        </header>
        <ErrorBoundary key={tab} errorTitle={t('errors.componentError')} errorMessage={t('errors.unknownError')} retryText={t('common.retry')}>
          <Suspense fallback={<div className="admin-loading"><div className="admin-loading-spinner" />{t('common.loading')}</div>}>
            <div className="admin-tab-content" role="tabpanel" id={`admin-panel-${tab}`} aria-labelledby={`admin-tab-${tab}`}>
              {tab === 'dashboard' && <DashboardTab />}
              {tab === 'videos' && (
                <>
                  <nav className="admin-sub-tabs" role="tablist" aria-label={t('admin.media.subVideo')}>
                    {MEDIA_SUB_TABS.map(st => (
                      <button
                        key={st.key}
                        type="button"
                        role="tab"
                        aria-selected={mediaTab === st.key}
                        className={`admin-sub-tab ${mediaTab === st.key ? 'active' : ''}`}
                        onClick={() => setMediaTab(st.key)}
                      >
                        {t(`admin.media.sub${st.key === 'video' ? 'Video' : 'Image'}`)}
                      </button>
                    ))}
                  </nav>
                  <VideosTab key={mediaTab} sourceType={currentSourceType} />
                </>
              )}
              {tab === 'users' && <UsersTab />}
              {tab === 'tags' && <TagsTab />}
              {tab === 'system' && <SystemTab />}
              {tab === 'logs' && <LogsTab />}
              {tab === 'chat' && <ChatTab />}
            </div>
          </Suspense>
        </ErrorBoundary>
      </main>
    </div>
  )
}
