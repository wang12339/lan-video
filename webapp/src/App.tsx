import { lazy, Suspense, memo, useEffect } from 'react'
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { AuthProvider } from './context/AuthContext'
import { ChatProvider } from './context/ChatContext'
import Layout from './components/Layout/Layout'
import ErrorBoundary from './components/ui/ErrorBoundary'
import { RequireAuth, RequireAdmin } from './components/ProtectedRoute'
import { ToastProvider, useToast } from './components/Toast/Toast'
import { setOnError } from './api'

const Home = lazy(() => import('./pages/Home/Home'))
const Player = lazy(() => import('./pages/Player/Player'))
const Gallery = lazy(() => import('./pages/Gallery/Gallery'))
const Upload = lazy(() => import('./pages/Upload/Upload'))
const Profile = lazy(() => import('./pages/Profile/Profile'))
const Chat = lazy(() => import('./pages/Chat/Chat'))
const Admin = lazy(() => import('./pages/Admin/Admin'))
const NotFound = lazy(() => import('./pages/NotFound/NotFound'))

import './components/ui/PageTransition.css'

const Loading = memo(function Loading() {
  const { t } = useTranslation()
  return (
    <div className="page-loading">
      <div className="page-loading-spinner" />
      <span>{t('common.loading')}</span>
    </div>
  )
})

// API 全局错误 → Toast 桥接：随 ToastProvider 上移到 App 层，
// 让 /player（不在 Layout 内）也能收到全局错误提示
function GlobalErrorBridge() {
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

function App() {
  return (
    <BrowserRouter basename="/webapp">
      <AuthProvider>
        <ChatProvider>
          <ToastProvider>
            <Suspense fallback={<Loading />}>
              <ErrorBoundary>
                <GlobalErrorBridge />
                <Routes>
                  <Route element={<Layout />}>
                    <Route path="/" element={<Home />} />
                    <Route path="/gallery" element={<Gallery />} />
                    <Route path="/profile" element={<Profile />} />
                    <Route path="/chat" element={<Chat />} />
                    <Route element={<RequireAuth />}>
                      <Route path="/upload" element={<Upload />} />
                    </Route>
                    <Route element={<RequireAdmin />}>
                      <Route path="/admin" element={<Admin />} />
                    </Route>
                    <Route path="*" element={<NotFound />} />
                  </Route>
                  <Route path="/player" element={<Player />} />
                </Routes>
              </ErrorBoundary>
            </Suspense>
          </ToastProvider>
        </ChatProvider>
      </AuthProvider>
    </BrowserRouter>
  )
}

export default App
