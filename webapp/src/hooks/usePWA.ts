import { useState, useEffect, useCallback } from 'react'

interface BeforeInstallPromptEvent extends Event {
  readonly platforms: string[]
  readonly userChoice: Promise<{
    outcome: 'accepted' | 'dismissed'
    platform: string
  }>
  prompt(): Promise<void>
}

export function usePWA() {
  const [installPrompt, setInstallPrompt] = useState<BeforeInstallPromptEvent | null>(null)
  const [isInstallable, setIsInstallable] = useState(false)
  const [isInstalled, setIsInstalled] = useState(false)
  const [isStandalone, setIsStandalone] = useState(false)

  useEffect(() => {
    // 检查是否已安装
    const isStandaloneMode = window.matchMedia('(display-mode: standalone)').matches
    const isInWebAppiOS = (window.navigator as unknown as Record<string, boolean>).standalone === true
    setIsStandalone(isStandaloneMode || isInWebAppiOS)

    // 检查是否已安装（Android）
    if (window.matchMedia('(display-mode: standalone)').matches) {
      setIsInstalled(true)
    }

    // 监听 beforeinstallprompt 事件
    const handleBeforeInstallPrompt = (e: Event) => {
      e.preventDefault()
      setInstallPrompt(e as BeforeInstallPromptEvent)
      setIsInstallable(true)
    }

    // 监听 appinstalled 事件
    const handleAppInstalled = () => {
      setIsInstalled(true)
      setIsInstallable(false)
      setInstallPrompt(null)
    }

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt)
    window.addEventListener('appinstalled', handleAppInstalled)

    return () => {
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt)
      window.removeEventListener('appinstalled', handleAppInstalled)
    }
  }, [])

  const install = useCallback(async () => {
    if (!installPrompt) return false

    try {
      await installPrompt.prompt()
      const { outcome } = await installPrompt.userChoice
      if (outcome === 'accepted') {
        setIsInstalled(true)
        setIsInstallable(false)
        setInstallPrompt(null)
        return true
      }
      return false
    } catch {
      return false
    }
  }, [installPrompt])

  return {
    isInstallable,
    isInstalled,
    isStandalone,
    install
  }
}

// 注册Service Worker
export function useServiceWorker() {
  const [isRegistered, setIsRegistered] = useState(false)
  const [updateAvailable, setUpdateAvailable] = useState(false)

  useEffect(() => {
    if (!('serviceWorker' in navigator)) return

    let cancelled = false
    const cleanups: Array<() => void> = []

    const registerSW = async () => {
      try {
        const registration = await navigator.serviceWorker.register('/webapp/sw.js')
        if (cancelled) return
        setIsRegistered(true)

        const onStateChange = () => {
          if (registration.installing?.state === 'activated') {
            setUpdateAvailable(true)
          }
        }
        const onUpdateFound = () => {
          const newWorker = registration.installing
          if (newWorker) {
            newWorker.addEventListener('statechange', onStateChange)
            cleanups.push(() => {
              if (typeof newWorker.removeEventListener === 'function') {
                newWorker.removeEventListener('statechange', onStateChange)
              }
            })
          }
        }
        registration.addEventListener('updatefound', onUpdateFound)
        cleanups.push(() => {
          if (typeof registration.removeEventListener === 'function') {
            registration.removeEventListener('updatefound', onUpdateFound)
          }
        })
      } catch (error) {
        if (!cancelled) console.error('SW registration failed:', error)
      }
    }

    registerSW()

    return () => {
      cancelled = true
      for (const fn of cleanups) fn()
    }
  }, [])

  const update = useCallback(() => {
    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.ready
        .then(registration => {
          registration.update()
        })
        .catch(() => {})
    }
  }, [])

  return {
    isRegistered,
    updateAvailable,
    update
  }
}
