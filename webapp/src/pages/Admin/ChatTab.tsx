import { useTranslation } from 'react-i18next'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { clearAllChatMessages, fetchChatStats } from '../../api'
import { useConfirmDialog } from '../../hooks/useConfirmDialog'
import { useAlertDialog } from '../../hooks/useAlertDialog'
import { ConfirmDialog, AlertDialog, SkeletonLoader } from './components'

export default function ChatTab() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { confirmDialog, askConfirm, handleCancel } = useConfirmDialog()
  const { alertMsg, showAlert, closeAlert } = useAlertDialog()

  const { data: stats, isLoading, isError } = useQuery({
    queryKey: ['admin-chat-stats'],
    queryFn: fetchChatStats,
  })

  const count = stats?.count

  const handleClear = () => {
    if (count === 0) {
      showAlert(t('admin.chatTab.cleared', { count: 0 }))
      return
    }
    askConfirm({
      title: t('admin.chatTab.clearConfirmTitle'),
      message: t('admin.chatTab.clearConfirmText'),
      danger: true,
      onConfirm: async () => {
        try {
          const res = await clearAllChatMessages()
          queryClient.invalidateQueries({ queryKey: ['admin-chat-stats'] })
          showAlert(t('admin.chatTab.cleared', { count: res.deleted }))
        } catch {
          showAlert(t('admin.chatTab.clearFailed'))
        }
      },
    })
  }

  return (
    <div className="admin-tab-content">
      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.chatTab.title')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.chatTab.messageCount')}</span>
            <span className="admin-info-value">
              {isLoading ? <SkeletonLoader width={60} height={20} /> : isError ? t('admin.chatTab.countLoadFailed') : (count ?? '--')}
            </span>
          </div>
          <div className="admin-info-row">
            <span className="admin-info-label">&nbsp;</span>
            <button
              type="button"
              className="admin-btn admin-btn-danger"
              onClick={handleClear}
              disabled={isLoading || isError}
            >
              {t('admin.chatTab.clearConfirmTitle')}
            </button>
          </div>
        </div>
      </div>

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
        open={alertMsg !== ''}
        message={alertMsg}
        onClose={closeAlert}
      />
    </div>
  )
}
