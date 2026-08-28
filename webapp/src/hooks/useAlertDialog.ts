import { useState, useCallback } from 'react'

export function useAlertDialog(initialMessage = '') {
  const [alertMsg, setAlertMsg] = useState(initialMessage)

  const showAlert = useCallback((message: string) => setAlertMsg(message), [])
  const closeAlert = useCallback(() => setAlertMsg(''), [])

  return { alertMsg, showAlert, closeAlert, setAlertMsg }
}
