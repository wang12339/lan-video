import { memo } from 'react'
import { Navigate, Outlet, useLocation } from 'react-router-dom'
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
  const location = useLocation()
  if (loading) return <Loading />
  if (!user) {
    // 记录来源（path+search），登录成功后由 Home 消费跳回
    return <Navigate to="/" replace state={{ from: location.pathname + location.search }} />
  }
  return <Outlet />
})

export const RequireAdmin = memo(function RequireAdmin() {
  const { user, loading } = useAuth()
  const location = useLocation()
  if (loading) return <Loading />
  if (!user) {
    return <Navigate to="/" replace state={{ from: location.pathname + location.search }} />
  }
  // 已登录但非管理员：不记录来源，避免登录后又被弹回无权限页
  if (!user.isAdmin) return <Navigate to="/" replace />
  return <Outlet />
})
