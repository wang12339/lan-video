import { Outlet, Link, useLocation, useNavigate, useSearchParams } from 'react-router-dom'
import { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { useChatRoom } from '../../context/ChatContext'
import { searchSuggest, getPendingUserCount, PENDING_USERS_CHANGED_EVENT } from '../../api'
import { addToSearchHistory } from '../../utils/searchHistory'
import { trackClick } from '../../utils/track'
import { useToast } from '../Toast/Toast'
import PageTransition from '../ui/PageTransition'
import AuthDialog from '../AuthDialog/AuthDialog'
import ThemeToggle from '../ui/ThemeToggle'
import { useScrollLock } from '../../hooks/useScrollLock'
import './Layout.css'

export default function Layout() {
  const location = useLocation()
  const { t } = useTranslation()
  return (
    <>
      <a href="#main-content" className="skip-link">{t('common.skipToContent') || '跳至主内容'}</a>
      <NavBar />
      <main id="main-content" className="page-content">
        <PageTransition transitionKey={location.pathname}>
          <Outlet />
        </PageTransition>
      </main>
    </>
  )
}

function NavBar() {
  const { t, i18n } = useTranslation()
  const { user, logout } = useAuth()
  const { unread } = useChatRoom()
  const { toast } = useToast()
  const location = useLocation()
  const navigate = useNavigate()
  const [searchQuery, setSearchQuery] = useState('')
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [selectedIdx, setSelectedIdx] = useState(-1)
  // 管理员：待审批注册用户数（导航徽标提醒）
  const [pendingCount, setPendingCount] = useState(0)
  const prevPendingRef = useRef<number | null>(null)
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchTried, setSearchTried] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  // 汉堡抽屉打开时锁背景滚动（移动端防穿透；抽屉自身 overscroll-behavior:contain）
  useScrollLock(menuOpen)

  // 实测导航实际高度写入 CSS 变量：≤640 时顶栏两行化，第二行搜索框的
  // 高度随断点/字体变化，此前用实测魔数 51px 让位，行高一变就错位。
  // ResizeObserver 让 .page-content/抽屉/吸顶工具条永远贴住导航底边。
  const navRef = useRef<HTMLElement>(null)
  useEffect(() => {
    const el = navRef.current
    if (!el) return
    const apply = () =>
      document.documentElement.style.setProperty('--nav-h-actual', `${el.offsetHeight}px`)
    apply()
    const ro = new ResizeObserver(apply)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  const [userMenuOpen, setUserMenuOpen] = useState(false)
  const [confirmLogout, setConfirmLogout] = useState(false)
  const [showAuth, setShowAuth] = useState(false)
  const [searchParams] = useSearchParams()

  // 网关 SSO 回跳落地（/webapp/?gw_code=... 或 ?gw_error=...）：
  // AuthDialog 只在 showAuth=true 时渲染，而回跳发生在新页面加载里，
  // 必须自动打开弹窗，否则 gw_code 无人消费、30 秒后过期 = SSO 登录失败。
  useEffect(() => {
    if (searchParams.has('gw_code') || searchParams.has('gw_error')) {
      setShowAuth(true)
    }
  }, [searchParams])
  const suggestTimer = useRef<ReturnType<typeof setTimeout>>()
  const suggestSeq = useRef(0)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const searchRef = useRef<HTMLDivElement>(null)
  const linksRef = useRef<HTMLDivElement>(null)
  const userMenuRef = useRef<HTMLDivElement>(null)
  const hamburgerRef = useRef<HTMLButtonElement>(null)

  const handleSearch = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    const q = selectedIdx >= 0 ? (suggestions[selectedIdx] ?? '') : searchQuery.trim()
    if (q) {
      try { addToSearchHistory(q) } catch (e) { void e; }
      navigate(`/?q=${encodeURIComponent(q)}`)
      setShowSuggestions(false)
      setSearchQuery(q)
    }
  }, [searchQuery, suggestions, selectedIdx, navigate])

  const handleInputChange = useCallback((value: string) => {
    setSearchQuery(value)
    setSelectedIdx(-1)
    if (suggestTimer.current) clearTimeout(suggestTimer.current)
    if (value.trim().length < 1) {
      setSuggestions([])
      setShowSuggestions(false)
      setSearchLoading(false)
      setSearchTried(false)
      return
    }
    setSearchLoading(true)
    setSearchTried(false)
    // 搜索建议接口需要登录（未登录必 401），游客直接跳过请求
    if (!user) {
      setSearchLoading(false)
      return
    }
    const seq = ++suggestSeq.current
    suggestTimer.current = setTimeout(async () => {
      try {
        const res = await searchSuggest(value.trim())
        if (seq !== suggestSeq.current) return
        setSuggestions(res)
        setShowSuggestions(res.length > 0)
        setSearchTried(true)
      } catch { /* ignore */ }
      finally {
        if (seq === suggestSeq.current) setSearchLoading(false)
      }
    }, 300)
  }, [user])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!showSuggestions) return
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIdx(i => Math.min(i + 1, suggestions.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIdx(i => Math.max(i - 1, -1))
    } else if (e.key === 'Escape') {
      setShowSuggestions(false)
    }
  }, [showSuggestions, suggestions.length])

  useEffect(() => {
    function handleClickOutside(e: MouseEvent | PointerEvent) {
      if (hamburgerRef.current && hamburgerRef.current.contains(e.target as Node)) {
        return
      }
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setShowSuggestions(false)
      }
      if (linksRef.current && !linksRef.current.contains(e.target as Node)) {
        setMenuOpen(false)
      }
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) {
        setUserMenuOpen(false)
        setConfirmLogout(false)
      }
    }
    document.addEventListener('pointerdown', handleClickOutside as EventListener)
    return () => {
      document.removeEventListener('pointerdown', handleClickOutside as EventListener)
    }
  }, [])

  useEffect(() => {
    document.documentElement.lang = i18n.language === 'en-US' ? 'en' : 'zh-CN'
  }, [i18n.language])

  const closeMenu = useCallback(() => setMenuOpen(false), [])

  // 路由变化时收起所有菜单
  useEffect(() => {
    setMenuOpen(false)
    setUserMenuOpen(false)
    setConfirmLogout(false)
  }, [location.pathname])

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setMenuOpen(false)
        setUserMenuOpen(false)
        setConfirmLogout(false)
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        searchInputRef.current?.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  const handleLogout = useCallback(async () => {
    setUserMenuOpen(false)
    setConfirmLogout(false)
    closeMenu()
    try {
      const serverOk = await logout()
      // 老会话缺 csrf cookie 时后端 403，会话没真吊销：提示刷新重试
      toast(t(serverOk ? 'nav.logoutSuccess' : 'nav.logoutRetryHint'), serverOk ? 'success' : 'error')
    } catch {
      toast(t('auth.error'), 'error')
    }
  }, [logout, toast, t, closeMenu])

  // 管理员：待审批注册用户提醒（徽标 + 新注册 toast，60s 轮询 + 回前台/审批后刷新）
  // toast/t 放进 ref：轮询只应随登录态（isAdmin）启停，避免依赖引用变化导致重复注册定时器
  const pendingToastRef = useRef(toast)
  pendingToastRef.current = toast
  const pendingTRef = useRef(t)
  pendingTRef.current = t
  useEffect(() => {
    if (!user?.isAdmin) {
      setPendingCount(0)
      prevPendingRef.current = null
      return
    }
    let cancelled = false
    const refresh = async () => {
      try {
        const n = await getPendingUserCount()
        if (cancelled) return
        setPendingCount(n)
        const prev = prevPendingRef.current
        if (prev !== null && n > prev) {
          pendingToastRef.current(
            pendingTRef.current('admin.users.newPendingToast', { count: n }),
            'info',
          )
        }
        prevPendingRef.current = n
      } catch {
        // 静默失败：徽标保持上次值，下一轮再试
      }
    }
    void refresh()
    const timer = setInterval(refresh, 60_000)
    const onVisible = () => {
      if (document.visibilityState === 'visible') void refresh()
    }
    const onChanged = () => void refresh()
    document.addEventListener('visibilitychange', onVisible)
    window.addEventListener(PENDING_USERS_CHANGED_EVENT, onChanged)
    return () => {
      cancelled = true
      clearInterval(timer)
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener(PENDING_USERS_CHANGED_EVENT, onChanged)
    }
  }, [user?.isAdmin])

  const handleGuestLogin = useCallback(() => {
    closeMenu()
    // 访客身份点击登录：直接弹登录框（不需要跳个人中心）
    if (user?.isGuest) {
      setShowAuth(true)
      return
    }
    // 首页对游客强制弹出登录框，避免重复叠加
    if (location.pathname === '/') {
      navigate('/profile')
    } else {
      setShowAuth(true)
    }
  }, [closeMenu, location.pathname, navigate, user])

  const isActive = useCallback((path: string) => location.pathname === path, [location.pathname])

  return (
    <>
      <nav ref={navRef} className="nav" aria-label={t('nav.mainNav')}>
        <Link to="/" className="nav-logo">{t('nav.logo')}</Link>

        <div className="nav-search" ref={searchRef}>
          <form onSubmit={handleSearch} className="nav-search-form">
            <span className="nav-search-icon" aria-hidden="true">🔍</span>
            <input
              ref={searchInputRef}
              type="text"
              placeholder={t('nav.search')}
              value={searchQuery}
              aria-label={t('common.search')}
              onChange={(e) => handleInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => suggestions.length > 0 && setShowSuggestions(true)}
            />
            <kbd className="search-shortcut" aria-hidden="true">
              {/Mac|iPhone|iPad|iPod/.test(navigator.platform) ? '⌘' : 'Ctrl+'}K
            </kbd>
          </form>
          {showSuggestions && (
            <div className="search-suggestions" role="listbox">
              {searchLoading ? (
                <div className="search-suggestions-loading">
                  <span className="search-spinner" aria-hidden="true" />
                  <span>{t('common.searching') || '搜索中…'}</span>
                </div>
              ) : suggestions.length > 0 ? (
                suggestions.map((s, i) => (
                  <div
                    key={s}
                    className={`search-suggestion ${i === selectedIdx ? 'selected' : ''}`}
                    role="option"
                    aria-selected={i === selectedIdx}
                    onMouseDown={() => {
                      try { addToSearchHistory(s) } catch (e) { void e; }
                      navigate(`/?q=${encodeURIComponent(s)}`)
                      setShowSuggestions(false)
                      setSearchQuery(s)
                    }}
                  >
                    <span className="search-suggestion-icon" aria-hidden="true">🔍</span>
                    <span className="search-suggestion-text">{s}</span>
                    <span className="search-suggestion-arrow" aria-hidden="true">↗</span>
                  </div>
                ))
              ) : searchTried ? (
                <div className="search-suggestions-empty">
                  <span className="search-empty-icon" aria-hidden="true">📭</span>
                  <span>{t('common.noResults') || '无搜索结果'}</span>
                </div>
              ) : null}
            </div>
          )}
        </div>

        <button
          ref={hamburgerRef}
          className="nav-menu-toggle"
          aria-label={menuOpen ? t('nav.closeMenu') : t('nav.openMenu')}
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen(o => !o)}
        >
          <span className={`hamburger-line ${menuOpen ? 'open' : ''}`} />
          <span className={`hamburger-line ${menuOpen ? 'open' : ''}`} />
          <span className={`hamburger-line ${menuOpen ? 'open' : ''}`} />
        </button>

        <div ref={linksRef} className={`nav-links ${menuOpen ? 'open' : ''}`}>
          <Link to="/" className={`nav-link ${isActive('/') ? 'active' : ''}`} aria-current={isActive('/') ? 'page' : undefined} onClick={() => { trackClick('导航', t('nav.home')); closeMenu() }}>{t('nav.home')}</Link>
          <Link to="/gallery" className={`nav-link ${isActive('/gallery') ? 'active' : ''}`} aria-current={isActive('/gallery') ? 'page' : undefined} onClick={() => { trackClick('导航', t('nav.gallery')); closeMenu() }}>{t('nav.gallery')}</Link>
          {user && (
            <Link to="/chat" className={`nav-link ${isActive('/chat') ? 'active' : ''}`} aria-current={isActive('/chat') ? 'page' : undefined} onClick={() => { trackClick('导航', t('nav.chat')); closeMenu() }}>
              {t('nav.chat')}
              {unread > 0 && <span className="nav-chat-badge" aria-label={t('chat.unread', { count: unread })}>{unread > 99 ? '99+' : unread}</span>}
            </Link>
          )}
          {user && (
            <Link to="/upload" className={`nav-link ${isActive('/upload') ? 'active' : ''}`} aria-current={isActive('/upload') ? 'page' : undefined} onClick={() => { trackClick('导航', t('nav.upload')); closeMenu() }}>{t('nav.upload')}</Link>
          )}
          {user?.isAdmin && (
            <Link
              to={pendingCount > 0 ? '/admin?tab=users' : '/admin'}
              className={`nav-link ${isActive('/admin') ? 'active' : ''}`}
              aria-current={isActive('/admin') ? 'page' : undefined}
              onClick={() => { trackClick('导航', t('nav.admin')); closeMenu() }}
            >
              {t('nav.admin')}
              {pendingCount > 0 && (
                <span className="nav-admin-badge" aria-label={t('admin.users.pendingAria', { count: pendingCount })}>
                  {pendingCount > 99 ? '99+' : pendingCount}
                </span>
              )}
            </Link>
          )}
          <div className="nav-mobile-user">
            {user ? (
              <>
                <Link to="/profile" className={`nav-link ${isActive('/profile') ? 'active' : ''}`} aria-current={isActive('/profile') ? 'page' : undefined} onClick={() => { trackClick('导航', t('nav.myProfile')); closeMenu() }}>{t('nav.myProfile')}</Link>
            <button
              type="button"
              className="nav-link nav-link-btn"
              aria-expanded={confirmLogout}
              onClick={() => {
                if (confirmLogout) {
                  void handleLogout()
                } else {
                  setConfirmLogout(true)
                }
              }}
            >
              {confirmLogout ? t('nav.logoutConfirm') : t('nav.logout')}
            </button>
              </>
            ) : (
              <button type="button" className="nav-link nav-link-btn" onClick={handleGuestLogin}>
                {t('nav.loginRegister')}
              </button>
            )}
          </div>
        </div>

        <button
          className="nav-lang-toggle"
          onClick={() => {
            const next = i18n.language === 'zh-CN' ? 'en-US' : 'zh-CN'
            i18n.changeLanguage(next)
            try { localStorage.setItem('atmos.lang', next) } catch (e) { void e; }
          }}
          aria-label={t('nav.toggleLanguage')}
          title={t('nav.toggleLanguage')}
        >
          {i18n.language === 'zh-CN' ? 'EN' : '中'}
        </button>
        <ThemeToggle />

        {user ? (
          <div className="nav-user" ref={userMenuRef}>
            <button
              className="nav-avatar"
              onClick={() => {
                setConfirmLogout(false)
                setUserMenuOpen(o => !o)
              }}
              aria-haspopup="menu"
              aria-expanded={userMenuOpen}
              aria-label={t('nav.myProfile')}
            >
              {user.username?.[0]?.toUpperCase() || '?'}
            </button>
            {userMenuOpen && (
              <div className="nav-user-menu" role="menu" aria-label={t('nav.myProfile')}>
                <div className="nav-user-menu-name">
                  {user.username}
                  {user.isGuest && <span className="nav-guest-badge">{t('nav.guestMode')}</span>}
                </div>
                {user.isGuest && (
                  <div className="nav-user-menu-hint">{t('nav.guestHint')}</div>
                )}
                <Link
                  to="/profile"
                  role="menuitem"
                  className="nav-user-menu-item"
                  onClick={() => { trackClick('导航', t('nav.myProfile')); setUserMenuOpen(false) }}
                >
                  {t('nav.myProfile')}
                </Link>
                {user.isGuest && (
                  <button
                    type="button"
                    role="menuitem"
                    className="nav-user-menu-item"
                    onClick={() => { trackClick('导航', t('nav.guestLoginCta')); setUserMenuOpen(false); handleGuestLogin() }}
                  >
                    {t('nav.guestLoginCta')}
                  </button>
                )}
                <button
                  type="button"
                  role="menuitem"
                  className="nav-user-menu-item nav-user-menu-logout"
                  onClick={() => {
                    if (confirmLogout) {
                      void handleLogout()
                    } else {
                      setConfirmLogout(true)
                    }
                  }}
                >
                  {confirmLogout ? t('nav.logoutConfirm') : t('nav.logout')}
                </button>
              </div>
            )}
          </div>
        ) : (
          <button type="button" className="nav-login-btn" onClick={handleGuestLogin}>
            {t('nav.login')}
          </button>
        )}
      </nav>

      {showAuth && (!user || user.isGuest) && (
        <AuthDialog onClose={() => setShowAuth(false)} />
      )}
    </>
  )
}
