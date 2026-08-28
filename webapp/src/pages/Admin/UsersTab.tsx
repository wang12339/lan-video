import { useState, useEffect, useMemo, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useAuth } from '../../context/AuthContext'
import { listUsers, deleteUser, resetUserPassword, toggleUserAdmin, approveUser, kickUser } from '../../api/admin'
import type { AdminUser } from '../../api/admin'
import { useDebouncedValue } from '../../utils/throttle'
import { useConfirmDialog } from '../../hooks/useConfirmDialog'
import { useAlertDialog } from '../../hooks/useAlertDialog'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from './components'
import AdminModal from './components/AdminModal'

const PAGE_SIZE = 8
const SEARCH_DEBOUNCE_MS = 300

type RoleFilter = 'all' | 'admin' | 'user'

export default function UsersTab() {
  const { t } = useTranslation()
  const { user: currentUser } = useAuth()
  const queryClient = useQueryClient()

  // 搜索 / 角色筛选 / 分页
  const [searchInput, setSearchInput] = useState('')
  const debouncedSearch = useDebouncedValue(searchInput, SEARCH_DEBOUNCE_MS)
  const [roleFilter, setRoleFilter] = useState<RoleFilter>('all')
  const [page, setPage] = useState(0)

  // Password reset
  const [pwUserId, setPwUserId] = useState<string | null>(null)
  const [pwValue, setPwValue] = useState('')
  const [pwSaving, setPwSaving] = useState(false)
  const [pwOk, setPwOk] = useState(false)
  const [pwMsg, setPwMsg] = useState('')

  // Dialogs
  const { confirmDialog, askConfirm, handleCancel } = useConfirmDialog()
  const { alertMsg, showAlert, closeAlert } = useAlertDialog()

  // ──────────────────────────────────────────────────────────────
  // React Query: 获取用户列表
  // NOTE: 后端 GET /admin/users 不支持 search/page 参数，当前为全量加载 + 前端过滤。
  //       若后续后端支持 ?search=&page=&size=，可将参数传入 listUsers() 实现真正的服务端搜索。
  // ──────────────────────────────────────────────────────────────
  const { data: users = [], isLoading, error, refetch } = useQuery<AdminUser[]>({
    queryKey: ['admin-users'],
    queryFn: listUsers,
    staleTime: 30_000,
  })

  // 筛选条件变化时回到第一页
  useEffect(() => { setPage(0) }, [debouncedSearch, roleFilter])

  const handleDelete = (u: AdminUser) => {
    if (u.isAdmin) {
      showAlert(t('admin.users.cannotDeleteAdmin'))
      return
    }
    askConfirm({
      title: t('admin.users.deleteTitle'),
      message: t('admin.users.confirmDelete', { username: u.username }),
      danger: true,
      onConfirm: async () => {
        try {
          await deleteUser(u.id)
          queryClient.invalidateQueries({ queryKey: ['admin-users'] })
        } catch { showAlert(t('admin.users.deleteFailed')) }
      },
    })
  }

  const handleResetPw = async () => {
    if (!pwUserId || pwValue.length < 6) {
      setPwOk(false)
      setPwMsg(t('admin.users.passwordMinLength'))
      return
    }
    setPwSaving(true); setPwMsg('')
    try {
      const res = await resetUserPassword(pwUserId, pwValue)
      if (res.ok) {
        setPwOk(true)
        setPwMsg(t('admin.users.passwordReset'))
        setPwUserId(null)
        setPwValue('')
      } else {
        setPwOk(false)
        setPwMsg(res.error || t('admin.users.resetFailed'))
      }
    } catch {
      setPwOk(false)
      setPwMsg(t('admin.users.requestFailed'))
    } finally { setPwSaving(false) }
  }

  const handlePwKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !pwSaving) {
      e.preventDefault()
      handleResetPw()
    }
    if (e.key === 'Escape') setPwUserId(null)
  }, [pwSaving])

  const handleToggleAdmin = (u: AdminUser) => {
    if (u.id === currentUser?.id) {
      showAlert(t('admin.users.selfLock'))
      return
    }
    askConfirm({
      title: t('admin.users.toggleAdminTitle'),
      message: t('admin.users.confirmToggleAdmin', { action: t(u.isAdmin ? 'admin.users.toggleAdminActionRemove' : 'admin.users.toggleAdminActionGrant'), username: u.username }),
      onConfirm: async () => {
        try {
          await toggleUserAdmin(u.id)
          queryClient.invalidateQueries({ queryKey: ['admin-users'] })
        } catch {
          showAlert(t('admin.users.operationFailed'))
        }
      },
    })
  }

  const handleApprove = (u: AdminUser, approved: boolean) => {
    askConfirm({
      title: approved ? t('admin.users.approveTitle') : t('admin.users.rejectTitle'),
      message: t('admin.users.confirmApprove', { action: t(approved ? 'admin.users.approveActionApprove' : 'admin.users.approveActionReject'), username: u.username }),
      danger: !approved,
      onConfirm: async () => {
        try {
          await approveUser(u.id, approved)
          queryClient.invalidateQueries({ queryKey: ['admin-users'] })
        } catch {
          showAlert(t('admin.users.operationFailed'))
        }
      },
    })
  }

  const handleKick = (u: AdminUser) => {
    if (u.id === currentUser?.id) {
      showAlert(t('admin.users.cannotKickSelf'))
      return
    }
    askConfirm({
      title: t('admin.users.kickTitle'),
      message: t('admin.users.confirmKick', { username: u.username }),
      danger: true,
      onConfirm: async () => {
        try {
          await kickUser(u.id)
          queryClient.invalidateQueries({ queryKey: ['admin-users'] })
        } catch {
          showAlert(t('admin.users.operationFailed'))
        }
      },
    })
  }

  // 防抖后的搜索关键词用于过滤，避免每次按键都重新计算
  const pendingUsers = useMemo(() => {
    const q = debouncedSearch.trim().toLowerCase()
    return users.filter(u => !u.approved && !u.isAdmin && (!q || u.username.toLowerCase().includes(q)))
  }, [users, debouncedSearch])

  const filteredUsers = useMemo(() => {
    const q = debouncedSearch.trim().toLowerCase()
    return users.filter(u => {
      if (u.approved || u.isAdmin) {
        if (roleFilter === 'admin' && !u.isAdmin) return false
        if (roleFilter === 'user' && u.isAdmin) return false
        return !q || u.username.toLowerCase().includes(q)
      }
      return false
    })
  }, [users, roleFilter, debouncedSearch])

  // 删除/筛选后页码越界自动修正
  const totalPages = Math.max(1, Math.ceil(filteredUsers.length / PAGE_SIZE))
  useEffect(() => {
    setPage(p => Math.min(p, totalPages - 1))
  }, [totalPages])

  const pageUsers = useMemo(() => filteredUsers.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE), [filteredUsers, page])

  if (isLoading) return <SkeletonLoader type="card" lines={5} />
  if (error) {
    return (
      <div className="admin-error">
        <p>{t('admin.users.loadFailed')}</p>
        <button className="admin-btn" onClick={() => refetch()}>{t('common.retry')}</button>
      </div>
    )
  }

  return (
    <div className="admin-tab-content">
      <div className="admin-toolbar">
        <span className="admin-toolbar-info">
          {t('admin.users.total', { count: users.length })}
          {pendingUsers.length > 0 ? `，${t('admin.users.pending', { count: pendingUsers.length })}` : ''}
        </span>
        <div className="admin-search">
          <input type="search" value={searchInput} onChange={e => setSearchInput(e.target.value)} placeholder={t('admin.users.username')} aria-label={t('admin.users.username')} />
        </div>
        <select className="admin-btn" value={roleFilter} onChange={e => setRoleFilter(e.target.value as RoleFilter)} aria-label={t('admin.users.roleFilter')}>
          <option value="all">{t('admin.users.allRoles')}</option>
          <option value="admin">{t('admin.users.admin')}</option>
          <option value="user">{t('admin.users.regularUser')}</option>
        </select>
        <button className="admin-btn" onClick={() => refetch()}>{t('admin.users.refresh')}</button>
      </div>

      {pendingUsers.length > 0 && (
        <div className="admin-section">
          <h3 className="admin-section-title">{t('admin.stats.pending')}</h3>
          <div className="admin-card">
            {pendingUsers.map(u => (
              <div key={u.id} className="admin-pending-item">
                <div className="admin-user-avatar-sm admin-user-avatar-pending">{u.username[0]?.toUpperCase() || '?'}</div>
                <div className="admin-pending-info">
                  <div className="admin-pending-name">{u.username}</div>
                  <div className="admin-pending-date">{u.createdAt ? new Date(u.createdAt).toLocaleString('zh-CN') : ''}</div>
                </div>
                <div className="admin-pending-actions">
                  <button className="admin-btn admin-btn-primary" onClick={() => handleApprove(u, true)}>{t('admin.users.approve')}</button>
                  <button className="admin-btn admin-btn-danger" onClick={() => handleApprove(u, false)}>{t('admin.users.reject')}</button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {pageUsers.length === 0 ? (
        <div className="admin-empty">{t('admin.users.noMatch')}</div>
      ) : (
        <>
          <div className="admin-table-wrap">
            <table className="admin-table">
              <thead>
                <tr>
                  <th className="admin-col-avatar">{t('admin.users.avatar')}</th>
                  <th>{t('admin.users.username')}</th>
                  <th className="admin-col-status">{t('admin.users.status')}</th>
                  <th className="admin-col-date">{t('admin.users.registerDate')}</th>
                  <th className="admin-col-actions" style={{ minWidth: 140 }}>{t('admin.users.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {pageUsers.map(u => {
                  const isSelf = u.id === currentUser?.id
                  return (
                    <tr key={u.id}>
                      <td className="admin-col-avatar"><div className="admin-user-avatar-sm">{u.username[0]?.toUpperCase() || '?'}</div></td>
                      <td>
                        {u.username}
                        {u.isAdmin && <span className="admin-badge">{t('admin.users.admin')}</span>}
                        {isSelf && <span className="admin-badge">{t('admin.users.currentAccount')}</span>}
                      </td>
                      <td className="admin-col-status">
                        <span className={`admin-status-dot ${u.hasActiveToken ? 'online' : ''}`} />
                        {u.hasActiveToken ? t('admin.users.online') : t('admin.users.offline')}
                      </td>
                      <td className="admin-col-date">{u.createdAt ? new Date(u.createdAt).toLocaleDateString('zh-CN') : '--'}</td>
                      <td className="admin-col-actions">
                        <button className="admin-icon-btn" title={t('admin.users.resetPassword')} aria-label={`${t('admin.users.resetPassword')}：${u.username}`} onClick={() => { setPwUserId(u.id); setPwValue(''); setPwMsg('') }}>
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                        </button>
                        {!isSelf && (
                          <>
                            <button className="admin-icon-btn" title={u.isAdmin ? t('admin.users.unsetAdmin') : t('admin.users.setAdmin')} aria-label={`${u.isAdmin ? t('admin.users.unsetAdmin') : t('admin.users.setAdmin')}：${u.username}`} onClick={() => handleToggleAdmin(u)}>
                              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
                            </button>
                            {u.hasActiveToken && (
                              <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.users.forceKick')} aria-label={`${t('admin.users.forceKick')}：${u.username}`} onClick={() => handleKick(u)}>
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
                              </button>
                            )}
                            {!u.isAdmin && (
                              <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.users.delete')} aria-label={`${t('admin.users.delete')}：${u.username}`} onClick={() => handleDelete(u)}>
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                              </button>
                            )}
                          </>
                        )}
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          {totalPages > 1 && (
            <div className="admin-pagination">
              <button disabled={page === 0} onClick={() => setPage(p => p - 1)}>{t('admin.media.prevPage')}</button>
              <span>{t('admin.users.pageInfo', { page: page + 1, total: totalPages, count: filteredUsers.length })}</span>
              <button disabled={page >= totalPages - 1} onClick={() => setPage(p => p + 1)}>{t('admin.media.nextPage')}</button>
            </div>
          )}
        </>
      )}

      {pwUserId && (
        <AdminModal
          title={t('admin.users.resetPassword')}
          onClose={() => setPwUserId(null)}
          actions={
            <>
              <button className="admin-btn" onClick={() => setPwUserId(null)}>{t('admin.users.cancel')}</button>
              <button className="admin-btn admin-btn-primary" onClick={handleResetPw} disabled={pwSaving}>{pwSaving ? t('admin.users.resetting') : t('admin.users.reset')}</button>
            </>
          }
        >
          <label><span>{t('admin.users.newPassword')}</span><input type="password" value={pwValue} onChange={e => setPwValue(e.target.value)} onKeyDown={handlePwKeyDown} placeholder={t('admin.users.passwordPlaceholder')} autoFocus autoComplete="new-password" maxLength={128} /></label>
          {pwMsg && <div className="admin-result" style={{ color: pwOk ? undefined : '#ef4444' }}>{pwMsg}</div>}
        </AdminModal>
      )}

      <ConfirmDialog
        open={confirmDialog.open}
        title={confirmDialog.title}
        message={confirmDialog.message}
        danger={confirmDialog.danger}
        confirmText={t('common.confirm')}
        onConfirm={confirmDialog.onConfirm}
        onCancel={handleCancel}
      />

      <AlertDialog
        open={!!alertMsg}
        message={alertMsg}
        onClose={closeAlert}
      />
    </div>
  )
}
