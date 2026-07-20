import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { listUsers, deleteUser, resetUserPassword, toggleUserAdmin, approveUser, kickUser } from '../../api/admin'
import type { AdminUser } from '../../api/admin'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from './components'

export default function UsersTab() {
  const { t } = useTranslation()
  const [users, setUsers] = useState<AdminUser[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')

  // Password reset
  const [pwUserId, setPwUserId] = useState<number | null>(null)
  const [pwValue, setPwValue] = useState('')
  const [pwSaving, setPwSaving] = useState(false)
  const [pwMsg, setPwMsg] = useState('')

  // Dialogs
  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean
    title: string
    message: string
    danger?: boolean
    onConfirm: () => void
  }>({ open: false, title: '', message: '', onConfirm: () => {} })

  const [alertDialog, setAlertDialog] = useState<{
    open: boolean
    message: string
  }>({ open: false, message: '' })

  const loadUsers = useCallback(async () => {
    setLoading(true)
    try { setUsers(await listUsers()); setError('') }
    catch { setError(t('admin.users.loadFailed')) }
    finally { setLoading(false) }
  }, [])

  useEffect(() => { loadUsers() }, [loadUsers])

  const handleDelete = (u: AdminUser) => {
    if (u.isAdmin) {
      setAlertDialog({ open: true, message: t('admin.users.cannotDeleteAdmin') })
      return
    }
    setConfirmDialog({
      open: true,
      title: t('admin.users.deleteTitle'),
      message: t('admin.users.confirmDelete', { username: u.username }),
      danger: true,
      onConfirm: async () => {
        try { await deleteUser(u.id); setUsers(prev => prev.filter(x => x.id !== u.id)) }
        catch { setAlertDialog({ open: true, message: t('admin.users.deleteFailed') }) }
      },
    })
  }

  const handleResetPw = async () => {
    if (!pwUserId || pwValue.length < 6) {
      setAlertDialog({ open: true, message: t('admin.users.passwordMinLength') })
      return
    }
    setPwSaving(true); setPwMsg('')
    try {
      const res = await resetUserPassword(pwUserId, pwValue)
      if (res.ok) { setPwMsg(t('admin.users.passwordReset')); setPwUserId(null); setPwValue('') }
      else setPwMsg(res.error || t('admin.users.resetFailed'))
    } catch { setPwMsg(t('admin.users.requestFailed')) } finally { setPwSaving(false) }
  }

  const handleToggleAdmin = (u: AdminUser) => {
    setConfirmDialog({
      open: true,
      title: t('admin.users.toggleAdminTitle'),
      message: t('admin.users.confirmToggleAdmin', { action: t(u.isAdmin ? 'admin.users.toggleAdminActionRemove' : 'admin.users.toggleAdminActionGrant'), username: u.username }),
      onConfirm: async () => {
        const prev = users.find(x => x.id === u.id)
        setUsers(prev => prev.map(x => x.id === u.id ? { ...x, isAdmin: !x.isAdmin } : x))
        try {
          await toggleUserAdmin(u.id)
        } catch {
          if (prev) setUsers(p => p.map(x => x.id === u.id ? prev : x))
          setAlertDialog({ open: true, message: t('admin.users.operationFailed') })
        }
      },
    })
  }

  const handleApprove = (u: AdminUser, approved: boolean) => {
    setConfirmDialog({
      open: true,
      title: approved ? t('admin.users.approveTitle') : t('admin.users.rejectTitle'),
      message: t('admin.users.confirmApprove', { action: t(approved ? 'admin.users.approveActionApprove' : 'admin.users.approveActionReject'), username: u.username }),
      danger: !approved,
      onConfirm: async () => {
        const prev = users.find(x => x.id === u.id)
        if (approved) {
          setUsers(prev => prev.map(x => x.id === u.id ? { ...x, approved: true } : x))
        } else {
          setUsers(prev => prev.filter(x => x.id !== u.id))
        }
        try {
          await approveUser(u.id, approved)
        } catch {
          if (prev) setUsers(p => p.map(x => x.id === u.id ? prev : x))
          setAlertDialog({ open: true, message: t('admin.users.operationFailed') })
        }
      },
    })
  }

  const handleKick = (u: AdminUser) => {
    setConfirmDialog({
      open: true,
      title: t('admin.users.kickTitle'),
      message: t('admin.users.confirmKick', { username: u.username }),
      danger: true,
      onConfirm: async () => {
        const prev = users.find(x => x.id === u.id)
        setUsers(prev => prev.map(x => x.id === u.id ? { ...x, hasActiveToken: false } : x))
        try {
          await kickUser(u.id)
        } catch {
          if (prev) setUsers(p => p.map(x => x.id === u.id ? prev : x))
          setAlertDialog({ open: true, message: t('admin.users.operationFailed') })
        }
      },
    })
  }

  const pendingUsers = users.filter(u => !u.approved && !u.isAdmin)
  const approvedUsers = users.filter(u => u.approved || u.isAdmin)

  if (loading) return <SkeletonLoader type="card" lines={5} />
  if (error) return <div className="admin-error">{error}</div>

  return (
    <div className="admin-tab-content">
      <div className="admin-toolbar">
        <span className="admin-toolbar-info">{t('admin.users.total', { count: users.length })}{pendingUsers.length > 0 ? `，${t('admin.users.pending', { count: pendingUsers.length })}` : ''}</span>
        <button className="admin-btn" onClick={loadUsers}>{t('admin.users.refresh')}</button>
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
            {approvedUsers.map(u => (
              <tr key={u.id}>
                <td className="admin-col-avatar"><div className="admin-user-avatar-sm">{u.username[0]?.toUpperCase() || '?'}</div></td>
                <td>
                  {u.username}
                  {u.isAdmin && <span className="admin-badge">{t('admin.users.admin')}</span>}
                </td>
                <td className="admin-col-status">
                  <span className={`admin-status-dot ${u.hasActiveToken ? 'online' : ''}`} />
                  {u.hasActiveToken ? t('admin.users.online') : t('admin.users.offline')}
                </td>
                <td className="admin-col-date">{u.createdAt ? new Date(u.createdAt).toLocaleDateString('zh-CN') : '--'}</td>
                <td className="admin-col-actions">
                  <button className="admin-icon-btn" title={t('admin.users.resetPassword')} onClick={() => { setPwUserId(u.id); setPwValue(''); setPwMsg('') }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                  </button>
                  <button className="admin-icon-btn" title={u.isAdmin ? t('admin.users.unsetAdmin') : t('admin.users.setAdmin')} onClick={() => handleToggleAdmin(u)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
                  </button>
                  {u.hasActiveToken && (
                    <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.users.forceKick')} onClick={() => handleKick(u)}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
                    </button>
                  )}
                  {!u.isAdmin && (
                    <button className="admin-icon-btn admin-icon-btn-danger" title={t('admin.users.delete')} onClick={() => handleDelete(u)}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {pwUserId && (
        <div className="admin-modal-overlay" onClick={() => setPwUserId(null)}>
          <div className="admin-modal" onClick={e => e.stopPropagation()}>
            <h3>{t('admin.users.resetPassword')}</h3>
            <label><span>{t('admin.users.newPassword')}</span><input type="password" value={pwValue} onChange={e => setPwValue(e.target.value)} placeholder={t('admin.users.passwordPlaceholder')} autoFocus autoComplete="new-password" /></label>
            {pwMsg && <div className="admin-result">{pwMsg}</div>}
            <div className="admin-modal-actions">
              <button className="admin-btn" onClick={() => setPwUserId(null)}>{t('admin.users.cancel')}</button>
              <button className="admin-btn admin-btn-primary" onClick={handleResetPw} disabled={pwSaving}>{pwSaving ? t('admin.users.resetting') : t('admin.users.reset')}</button>
            </div>
          </div>
        </div>
      )}

      <ConfirmDialog
        open={confirmDialog.open}
        title={confirmDialog.title}
        message={confirmDialog.message}
        danger={confirmDialog.danger}
        confirmText={t('common.confirm')}
        onConfirm={confirmDialog.onConfirm}
        onCancel={() => setConfirmDialog(prev => ({ ...prev, open: false }))}
      />

      <AlertDialog
        open={alertDialog.open}
        message={alertDialog.message}
        onClose={() => setAlertDialog({ open: false, message: '' })}
      />
    </div>
  )
}
