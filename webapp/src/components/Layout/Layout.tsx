import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { searchSuggest, setOnError } from '../../api'
import { trackClick } from '../../utils/track'
import { ToastProvider, useToast } from '../Toast/Toast'
import PageTransition from '../ui/PageTransition'
import AuthDialog from '../AuthDialog/AuthDialog'
import './Layout.css'

function ErrorBoundaryInit() {
  const { toast } = useToast()
  const { t } = useTranslation()
  useEffect(() => {
    setOnError((err) => {
      toast(err.message || t('auth.error'), 'error')
    })
    return () => setOnError(() => {})
  }, [toast, t])
  return null
}

export default function Layout() {
  const location = useLocation()
  return (
    <ToastProvider>
      <ErrorBoundaryInit />
      <NavBar />
      <main className="page-content">
        <PageTransition transitionKey={location.pathname}>
          <Outlet />
        </PageTransition>
      </main>
    </ToastProvider>
  )
}

function NavBar() {
  const { t, i18n } = useTranslation()
  const { user, logout } = useAuth()
  const { toast } = useToast()
  const location = useLocation()
  const navigate = useNavigate()
  const [searchQuery, setSearchQuery] = useState('')
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [selectedIdx, setSelectedIdx] = useState(-1)
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchTried, setSearchTried] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const [userMenuOpen, setUserMenuOpen] = useState(false)
  const [confirmLogout, setConfirmLogout] = useState(false)
  const [showAuth, setShowAuth] = useState(false)
  const suggestTimer = useRef<ReturnType<typeof setTimeout>>()
  const searchRef = useRef<HTMLDivElement>(null)
  const linksRef = useRef<HTMLDivElement>(null)
  const userMenuRef = useRef<HTMLDivElement>(null)
  const hamburgerRef = useRef<HTMLButtonElement>(null)

  const handleSearch = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    const q = selectedIdx >= 0 ? suggestions[selectedIdx] : searchQuery.trim()
    if (q) {
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
    suggestTimer.current = setTimeout(async () => {
      try {
        const res = await searchSuggest(value.trim())
        setSuggestions(res)
        setShowSuggestions(res.length > 0)
        setSearchTried(true)
      } catch { /* ignore */ }
      finally { setSearchLoading(false) }
    }, 300)
  }, [])

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
    function handleClickOutside(e: MouseEvent) {
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
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
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

  // Esc 收起菜单
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setMenuOpen(false)
        setUserMenuOpen(false)
        setConfirmLogout(false)
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
      await logout()
      toast(t('nav.logoutSuccess'), 'success')
    } catch {
      toast(t('auth.error'), 'error')
    }
  }, [logout, toast, t, closeMenu])

  const handleGuestLogin = useCallback(() => {
    closeMenu()
    // 首页对游客强制弹出登录框，避免重复叠加
    if (location.pathname === '/') {
      navigate('/profile')
    } else {
      setShowAuth(true)
    }
  }, [closeMenu, location.pathname, navigate])

  const isActive = (path: string) => location.pathname === path

  return (
    <>
      <nav className="nav">
        <Link to="/" className="nav-logo">{t('nav.logo')}</Link>

        <div className="nav-search" ref={searchRef}>
          <form onSubmit={handleSearch} className="nav-search-form">
            <span className="nav-search-icon" aria-hidden="true">🔍</span>
            <input
              type="text"
              placeholder={t('nav.search')}
              value={searchQuery}
              aria-label={t('common.search')}
              onChange={(e) => handleInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => suggestions.length > 0 && setShowSuggestions(true)}
            />
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
          <Link to="/" className={`nav-link ${isActive('/') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.home')); closeMenu() }}>{t('nav.home')}</Link>
          <Link to="/gallery" className={`nav-link ${isActive('/gallery') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.gallery')); closeMenu() }}>{t('nav.gallery')}</Link>
          {user?.isAdmin && (
            <Link to="/upload" className={`nav-link ${isActive('/upload') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.upload')); closeMenu() }}>{t('nav.upload')}</Link>
          )}
          {user?.isAdmin && (
            <Link to="/admin" className={`nav-link ${isActive('/admin') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.admin')); closeMenu() }}>{t('nav.admin')}</Link>
          )}
          <div className="nav-mobile-user">
            {user ? (
              <>
                <Link to="/profile" className={`nav-link ${isActive('/profile') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.myProfile')); closeMenu() }}>{t('nav.myProfile')}</Link>
                <button
                  type="button"
                  className="nav-link nav-link-btn"
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
            localStorage.setItem('atmos.lang', next)
          }}
          aria-label={t('nav.toggleLanguage')}
          title={t('nav.toggleLanguage')}
        >
          {i18n.language === 'zh-CN' ? 'EN' : '中'}
        </button>

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
                <div className="nav-user-menu-name">{user.username}</div>
                <Link
                  to="/profile"
                  role="menuitem"
                  className="nav-user-menu-item"
                  onClick={() => { trackClick('导航', t('nav.myProfile')); setUserMenuOpen(false) }}
                >
                  {t('nav.myProfile')}
                </Link>
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

      {showAuth && !user && (
        <AuthDialog onClose={() => setShowAuth(false)} />
      )}
    </>
  )
}
