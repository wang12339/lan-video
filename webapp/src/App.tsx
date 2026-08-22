import { lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { AuthProvider } from './context/AuthContext'
import Layout from './components/Layout/Layout'
import ErrorBoundary from './components/ui/ErrorBoundary'
import { RequireAuth, RequireAdmin } from './components/ProtectedRoute'

const Home = lazy(() => import('./pages/Home/Home'))
const Player = lazy(() => import('./pages/Player/Player'))
const Gallery = lazy(() => import('./pages/Gallery/Gallery'))
const Upload = lazy(() => import('./pages/Upload/Upload'))
const Profile = lazy(() => import('./pages/Profile/Profile'))
const Admin = lazy(() => import('./pages/Admin/Admin'))
const NotFound = lazy(() => import('./pages/NotFound/NotFound'))

import './components/ui/PageTransition.css'

function Loading() {
  const { t } = useTranslation()
  return (
    <div className="page-loading">
      <div className="page-loading-spinner" />
      <span>{t('common.loading')}</span>
    </div>
  )
}

function App() {
  return (
    <BrowserRouter basename="/webapp">
      <AuthProvider>
        <Suspense fallback={<Loading />}>
          <ErrorBoundary>
            <Routes>
              <Route element={<Layout />}>
                <Route path="/" element={<Home />} />
                <Route path="/gallery" element={<Gallery />} />
                <Route path="/profile" element={<Profile />} />
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
      </AuthProvider>
    </BrowserRouter>
  )
}

export default App
