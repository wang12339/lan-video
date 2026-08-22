import { useState, useEffect, useCallback, useRef } from 'react'

interface NetworkState {
  isOnline: boolean
  isSlowConnection: boolean
  connectionType: string
  downlink: number | null
  rtt: number | null
  lastOnlineTime: number | null
  reconnectAttempts: number
  isReconnecting: boolean
}

interface SyncStatus {
  pending: number
  failed: number
  lastSyncTime: number | null
  isSyncing: boolean
}

export function useNetworkState(): NetworkState {
  const [state, setState] = useState<NetworkState>(() => ({
    isOnline: navigator.onLine,
    isSlowConnection: false,
    connectionType: 'unknown',
    downlink: null,
    rtt: null,
    lastOnlineTime: navigator.onLine ? Date.now() : null,
    reconnectAttempts: 0,
    isReconnecting: false
  }))

  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const maxReconnectAttempts = 5

  useEffect(() => {
    const updateOnlineState = () => {
      const isOnline = navigator.onLine
      setState(prev => ({ 
        ...prev, 
        isOnline,
        lastOnlineTime: isOnline ? Date.now() : prev.lastOnlineTime,
        reconnectAttempts: isOnline ? 0 : prev.reconnectAttempts,
        isReconnecting: false
      }))
      
      if (isOnline) {
        // Clear reconnect timer when back online
        if (reconnectTimerRef.current) {
          clearTimeout(reconnectTimerRef.current)
          reconnectTimerRef.current = null
        }
      }
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

    // Auto-reconnect logic
    const attemptReconnect = () => {
      setState(prev => {
        if (prev.isOnline || prev.reconnectAttempts >= maxReconnectAttempts) {
          return prev
        }
        
        const newAttempts = prev.reconnectAttempts + 1
        const delay = Math.min(1000 * Math.pow(2, newAttempts), 30000) // Exponential backoff, max 30s
        
        reconnectTimerRef.current = setTimeout(() => {
          // Attempt to fetch a lightweight resource to check connectivity
          fetch('/health', { 
            method: 'HEAD', 
            mode: 'no-cors',
            cache: 'no-cache'
          }).then(() => {
            // If fetch succeeds, we're back online
            updateOnlineState()
          }).catch(() => {
            // Still offline, try again
            attemptReconnect()
          })
        }, delay)
        
        return {
          ...prev,
          reconnectAttempts: newAttempts,
          isReconnecting: true
        }
      })
    }

    window.addEventListener('online', updateOnlineState)
    const handleOffline = () => {
      updateOnlineState()
      // Start auto-reconnect when going offline
      setTimeout(attemptReconnect, 1000)
    }
    window.addEventListener('offline', handleOffline)

    const connection = (navigator as unknown as { connection?: { addEventListener: (event: string, handler: () => void) => void; removeEventListener: (event: string, handler: () => void) => void } }).connection
    if (connection) {
      connection.addEventListener('change', updateConnectionInfo)
      updateConnectionInfo()
    }

    return () => {
      window.removeEventListener('online', updateOnlineState)
      window.removeEventListener('offline', handleOffline)
      if (connection) {
        connection.removeEventListener('change', updateConnectionInfo)
      }
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current)
      }
    }
  }, [])

  return state
}

// Offline alert hook with enhanced features
export function useOfflineAlert() {
  const { isOnline, isReconnecting, reconnectAttempts, connectionType, isSlowConnection } = useNetworkState()
  const [showAlert, setShowAlert] = useState(false)
  const [wasOffline, setWasOffline] = useState(false)
  const [syncStatus, setSyncStatus] = useState<SyncStatus>({
    pending: 0,
    failed: 0,
    lastSyncTime: null,
    isSyncing: false
  })
  
  // Track offline duration
  const offlineStartTime = useRef<number | null>(null)
  const [offlineDuration, setOfflineDuration] = useState(0)

  // Update offline duration
  useEffect(() => {
    if (!isOnline) {
      offlineStartTime.current = Date.now()
      const interval = setInterval(() => {
        if (offlineStartTime.current) {
          setOfflineDuration(Math.floor((Date.now() - offlineStartTime.current) / 1000))
        }
      }, 1000)
      return () => clearInterval(interval)
    } else {
      offlineStartTime.current = null
      setOfflineDuration(0)
    }
  }, [isOnline])

  // Simulate sync status (in a real app, this would come from your sync service)
  useEffect(() => {
    if (isOnline && wasOffline) {
      // Simulate syncing when coming back online
      setSyncStatus(prev => ({ ...prev, isSyncing: true }))
      
      // Simulate sync completion after a delay
      const timer = setTimeout(() => {
        setSyncStatus({
          pending: 0,
          failed: 0,
          lastSyncTime: Date.now(),
          isSyncing: false
        })
      }, 2000)
      
      return () => clearTimeout(timer)
    }
  }, [isOnline, wasOffline])

  useEffect(() => {
    if (!isOnline) {
      setShowAlert(true)
      setWasOffline(true)
      // Simulate pending sync items when offline
      setSyncStatus(prev => ({ 
        ...prev, 
        pending: Math.floor(Math.random() * 5) + 1 
      }))
    } else if (wasOffline) {
      // Show recovery alert briefly
      setShowAlert(true)
      const timer = setTimeout(() => setShowAlert(false), 3000)
      return () => clearTimeout(timer)
    }
  }, [isOnline, wasOffline])

  const dismissAlert = useCallback(() => {
    setShowAlert(false)
  }, [])

  const retryConnection = useCallback(() => {
    // Force a connection check
    window.dispatchEvent(new Event('online'))
  }, [])

  return {
    isOnline,
    showAlert,
    dismissAlert,
    retryConnection,
    isReconnecting,
    reconnectAttempts,
    connectionType,
    isSlowConnection,
    offlineDuration,
    syncStatus,
    wasOffline
  }
}

// Offline capabilities info
export function useOfflineCapabilities() {
  const [capabilities, setCapabilities] = useState({
    canPlayCached: false,
    canViewHistory: false,
    canBrowseFavorites: false,
    canManagePlaylists: false
  })

  useEffect(() => {
    // Check what's available offline
    const checkCapabilities = () => {
      // In a real app, you'd check your cache/storage
      setCapabilities({
        canPlayCached: 'caches' in window,
        canViewHistory: 'indexedDB' in window,
        canBrowseFavorites: 'indexedDB' in window,
        canManagePlaylists: 'indexedDB' in window
      })
    }
    
    checkCapabilities()
  }, [])

  return capabilities
}
