import { lazy, Suspense, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { ErrorBoundary } from './components'
import './Admin.css'

const DashboardTab = lazy(() => import('./DashboardTab'))
const VideosTab = lazy(() => import('./VideosTab'))
const UsersTab = lazy(() => import('./UsersTab'))
const TagsTab = lazy(() => import('./TagsTab'))
const SystemTab = lazy(() => import('./SystemTab'))
const LogsTab = lazy(() => import('./LogsTab'))

type Tab = 'dashboard' | 'videos' | 'users' | 'tags' | 'system' | 'logs'
type MediaSubTab = 'video' | 'image'

const TABS: Tab[] = ['dashboard', 'videos', 'users', 'tags', 'system', 'logs']

const MEDIA_SUB_TABS: { key: MediaSubTab; sourceType: string }[] = [
  { key: 'video', sourceType: 'local_video' },
  { key: 'image', sourceType: 'local_image' },
]

export default function Admin() {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [tab, setTab] = useState<Tab>('dashboard')
  const [mediaTab, setMediaTab] = useState<MediaSubTab>('video')

  if (!user?.isAdmin) {
    return (
      <div className="admin-page">
        <div className="admin-denied">
          <div className="admin-denied-icon"><span aria-hidden="true">🔒</span></div>
          <p>{t('admin.denied')}</p>
        </div>
      </div>
    )
  }

  const currentSourceType = MEDIA_SUB_TABS.find(t => t.key === mediaTab)?.sourceType ?? 'local_video'

  return (
    <div className="admin-page">
      <h1 className="admin-title">{t('admin.title')}</h1>
      <div className="admin-tabs">
        {TABS.map(key => (
          <button key={key} className={`admin-tab ${tab === key ? 'active' : ''}`} onClick={() => setTab(key)}>{t(`admin.tabs.${key}`)}</button>
        ))}
      </div>
      <ErrorBoundary key={tab} errorTitle={t('errors.componentError')} errorMessage={t('errors.unknownError')} retryText={t('common.retry')}>
        <Suspense fallback={<div className="admin-loading">{t('common.loading')}</div>}>
          {tab === 'dashboard' && <DashboardTab />}
          {tab === 'videos' && (
            <>
              <div className="admin-sub-tabs">
                {MEDIA_SUB_TABS.map(st => (
                  <button key={st.key} className={`admin-sub-tab ${mediaTab === st.key ? 'active' : ''}`} onClick={() => setMediaTab(st.key)}>{t(`admin.media.sub${st.key === 'video' ? 'Video' : 'Image'}`)}</button>
                ))}
              </div>
              <VideosTab key={mediaTab} sourceType={currentSourceType} />
            </>
          )}
          {tab === 'users' && <UsersTab />}
          {tab === 'tags' && <TagsTab />}
          {tab === 'system' && <SystemTab />}
          {tab === 'logs' && <LogsTab />}
        </Suspense>
      </ErrorBoundary>
    </div>
  )
}
