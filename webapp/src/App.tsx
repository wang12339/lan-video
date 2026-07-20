import { lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import { AuthProvider } from './context/AuthContext'
import Layout from './components/Layout/Layout'

const Home = lazy(() => import('./pages/Home/Home'))
const Player = lazy(() => import('./pages/Player/Player'))
const Gallery = lazy(() => import('./pages/Gallery/Gallery'))
const Upload = lazy(() => import('./pages/Upload/Upload'))
const Profile = lazy(() => import('./pages/Profile/Profile'))
const Admin = lazy(() => import('./pages/Admin/Admin'))

function Loading() {
  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '50vh', color: 'var(--text3)' }}>
      加载中...
    </div>
  )
}

function App() {
  return (
    <BrowserRouter>
      <AuthProvider>
        <Suspense fallback={<Loading />}>
          <Routes>
            <Route element={<Layout />}>
              <Route path="/" element={<Home />} />
              <Route path="/gallery" element={<Gallery />} />
              <Route path="/upload" element={<Upload />} />
              <Route path="/profile" element={<Profile />} />
              <Route path="/admin" element={<Admin />} />
            </Route>
            <Route path="/player" element={<Player />} />
          </Routes>
        </Suspense>
      </AuthProvider>
    </BrowserRouter>
  )
}

export default App
