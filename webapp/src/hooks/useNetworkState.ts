import { useState, useEffect, useCallback } from 'react'

interface NetworkState {
  isOnline: boolean
  isSlowConnection: boolean
  connectionType: string
  downlink: number | null
  rtt: number | null
}

export function useNetworkState(): NetworkState {
  const [state, setState] = useState<NetworkState>(() => ({
    isOnline: navigator.onLine,
    isSlowConnection: false,
    connectionType: 'unknown',
    downlink: null,
    rtt: null
  }))

  useEffect(() => {
    const updateOnlineState = () => {
      setState(prev => ({ ...prev, isOnline: navigator.onLine }))
    }

    const updateConnectionInfo = () => {
      const connection = (navigator as unknown as { connection?: { effectiveType?: string; downlink?: number; rtt?: number } }).connection
      
      if (connection) {
        const isSlowConnection = 
          connection.effectiveType === 'slow-2g' || 
          connection.effectiveType === '2g' ||
          (connection.downlink !== undefined && connection.downlink < 1.5)

        setState(prev => ({
          ...prev,
          isSlowConnection,
          connectionType: connection.effectiveType || 'unknown',
          downlink: connection.downlink ?? null,
          rtt: connection.rtt ?? null
        }))
      }
    }

    window.addEventListener('online', updateOnlineState)
    window.addEventListener('offline', updateOnlineState)

    const connection = (navigator as unknown as { connection?: { addEventListener: (event: string, handler: () => void) => void; removeEventListener: (event: string, handler: () => void) => void } }).connection
    if (connection) {
      connection.addEventListener('change', updateConnectionInfo)
      updateConnectionInfo()
    }

    return () => {
      window.removeEventListener('online', updateOnlineState)
      window.removeEventListener('offline', updateOnlineState)
      if (connection) {
        connection.removeEventListener('change', updateConnectionInfo)
      }
    }
  }, [])

  return state
}

// 离线提示Hook
export function useOfflineAlert() {
  const { isOnline } = useNetworkState()
  const [showAlert, setShowAlert] = useState(false)
  const [wasOffline, setWasOffline] = useState(false)

  useEffect(() => {
    if (!isOnline) {
      setShowAlert(true)
      setWasOffline(true)
    } else if (wasOffline) {
      // 恢复在线时显示提示
      setShowAlert(true)
      const timer = setTimeout(() => setShowAlert(false), 3000)
      return () => clearTimeout(timer)
    }
  }, [isOnline, wasOffline])

  const dismissAlert = useCallback(() => {
    setShowAlert(false)
  }, [])

  return {
    isOnline,
    showAlert,
    dismissAlert
  }
}
