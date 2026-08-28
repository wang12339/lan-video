import { memo } from 'react'
import { Navigate, Outlet } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../context/AuthContext'

function Loading() {
  const { t } = useTranslation()
  return (
    <div className="page-loading" role="status" aria-busy="true">
      <div className="page-loading-spinner" aria-hidden="true" />
      <span>{t('common.loading')}</span>
    </div>
  )
}

export const RequireAuth = memo(function RequireAuth() {
  const { user, loading } = useAuth()
  if (loading) return <Loading />
  if (!user) return <Navigate to="/" replace />
  return <Outlet />
})

export const RequireAdmin = memo(function RequireAdmin() {
  const { user, loading } = useAuth()
  if (loading) return <Loading />
  if (!user) return <Navigate to="/" replace />
  if (!user.isAdmin) return <Navigate to="/" replace />
  return <Outlet />
})
