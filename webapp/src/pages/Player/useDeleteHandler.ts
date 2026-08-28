import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { deleteVideo } from '../../api'

export function useDeleteHandler(videoId: string | undefined) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [deleteAlertMsg, setDeleteAlertMsg] = useState('')

  const handleDelete = useCallback(() => {
    if (!videoId) return
    setShowDeleteDialog(true)
  }, [videoId])

  const handleDeleteConfirm = useCallback(async () => {
    if (!videoId) return
    try {
      await deleteVideo(videoId)
      navigate('/')
    } catch {
      setDeleteAlertMsg(t('player.deleteError'))
    }
  }, [videoId, navigate, t])

  return { handleDelete, handleDeleteConfirm, showDeleteDialog, setShowDeleteDialog, deleteAlertMsg, setDeleteAlertMsg }
}
