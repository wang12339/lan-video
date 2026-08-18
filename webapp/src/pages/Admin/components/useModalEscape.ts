import { useEffect, useRef } from 'react'

/** 弹窗通用：按 ESC 关闭。用 ref 持有回调，避免每次渲染重建监听器。 */
export function useModalEscape(onClose: () => void, open = true) {
  const onCloseRef = useRef(onClose)
  useEffect(() => { onCloseRef.current = onClose }, [onClose])

  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCloseRef.current()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [open])
}
