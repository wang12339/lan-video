import { useState, useCallback } from 'react'

export interface ConfirmState {
  open: boolean
  title: string
  message: string
  danger?: boolean
  onConfirm: () => void | Promise<void>
}

export function useConfirmDialog() {
  const [confirmDialog, setConfirmDialog] = useState<ConfirmState>({
    open: false,
    title: '',
    message: '',
    onConfirm: () => {},
  })

  const askConfirm = useCallback((state: Omit<ConfirmState, 'open'>) => {
    setConfirmDialog({ open: true, ...state })
  }, [])

  const handleCancel = useCallback(() => {
    setConfirmDialog(prev => ({ ...prev, open: false }))
  }, [])

  return { confirmDialog, askConfirm, handleCancel }
}
