import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useState, useCallback, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { searchSuggest, setOnError } from '../../api'
import { trackClick } from '../../utils/track'
import { ToastProvider, useToast } from '../Toast/Toast'
import './Layout.css'

function ErrorBoundaryInit() {
  const { toast } = useToast()
  useEffect(() => {
    setOnError((err) => {
      toast(err.message || '操作失败', 'error')
    })
    return () => setOnError(() => {})
  }, [toast])
  return null
}

export default function Layout() {
  const { t, i18n } = useTranslation()
  const { user } = useAuth()
  const location = useLocation()
  const navigate = useNavigate()
  const [searchQuery, setSearchQuery] = useState('')
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [selectedIdx, setSelectedIdx] = useState(-1)
  const [menuOpen, setMenuOpen] = useState(false)
  const suggestTimer = useRef<ReturnType<typeof setTimeout>>()
  const searchRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
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
      return
    }
    suggestTimer.current = setTimeout(async () => {
      try {
        const res = await searchSuggest(value.trim())
        setSuggestions(res)
        setShowSuggestions(res.length > 0)
      } catch { /* ignore */ }
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
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  useEffect(() => {
    document.documentElement.lang = i18n.language === 'en-US' ? 'en' : 'zh-CN'
  }, [i18n.language])

  const closeMenu = useCallback(() => setMenuOpen(false), [])

  const isActive = (path: string) => location.pathname === path

  return (
    <ToastProvider>
      <ErrorBoundaryInit />
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
          {showSuggestions && suggestions.length > 0 && (
            <div className="search-suggestions" role="listbox">
              {suggestions.map((s, i) => (
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
                  <span aria-hidden="true">🔍</span> {s}
                </div>
              ))}
            </div>
          )}
        </div>

        <button
          ref={hamburgerRef}
          className="nav-menu-toggle"
          aria-label={menuOpen ? 'Close menu' : 'Open menu'}
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen(o => !o)}
        >
          {menuOpen ? '✕' : '☰'}
        </button>

        <div ref={menuRef} className={`nav-links ${menuOpen ? 'open' : ''}`}>
          <Link to="/" className={`nav-link ${isActive('/') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.home')); closeMenu() }}>{t('nav.home')}</Link>
          <Link to="/gallery" className={`nav-link ${isActive('/gallery') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.gallery')); closeMenu() }}>{t('nav.gallery')}</Link>
          {user?.isAdmin && (
            <Link to="/upload" className={`nav-link ${isActive('/upload') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.upload')); closeMenu() }}>{t('nav.upload')}</Link>
          )}
          {user?.isAdmin && (
            <Link to="/admin" className={`nav-link ${isActive('/admin') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.admin')); closeMenu() }}>{t('nav.admin')}</Link>
          )}
          <Link to="/profile" className={`nav-link ${isActive('/profile') ? 'active' : ''}`} onClick={() => { trackClick('导航', t('nav.profile')); closeMenu() }}>{t('nav.profile')}</Link>
        </div>

        <button
          className="nav-lang-toggle"
          onClick={() => {
            const next = i18n.language === 'zh-CN' ? 'en-US' : 'zh-CN'
            i18n.changeLanguage(next)
            localStorage.setItem('atmos.lang', next)
          }}
          aria-label="Toggle language"
        >
          {i18n.language === 'zh-CN' ? 'EN' : '中'}
        </button>

        {user ? (
          <Link to="/profile" className="nav-avatar">
            {user.username?.[0]?.toUpperCase() || '?'}
          </Link>
        ) : (
          <Link to="/profile" className="nav-avatar">?</Link>
        )}
      </nav>

      <main className="page-content">
        <Outlet />
      </main>
    </ToastProvider>
  )
}
