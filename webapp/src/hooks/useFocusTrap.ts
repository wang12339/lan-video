import { useEffect, useRef } from 'react'

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    el => el.offsetParent !== null || el === document.activeElement
  )
}

interface UseFocusTrapOptions {
  /** 自动聚焦到容器内首个可聚焦元素（默认 true） */
  autoFocus?: boolean
  /** 关闭时恢复焦点到触发元素（默认 true） */
  restoreFocus?: boolean
}

/**
 * 通用焦点陷阱 hook —— 将 Tab/Shift+Tab 循环限制在容器内。
 * 支持自动聚焦和关闭后恢复焦点。
 */
export function useFocusTrap(
  ref: React.RefObject<HTMLElement | null>,
  enabled: boolean,
  options: UseFocusTrapOptions = {},
) {
  const { autoFocus = true, restoreFocus = true } = options
  const prevFocusRef = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!enabled || !ref.current) return
    const container = ref.current

    if (restoreFocus) {
      prevFocusRef.current =
        document.activeElement instanceof HTMLElement ? document.activeElement : null
    }

    if (autoFocus) {
      const timer = setTimeout(() => {
        const first = getFocusableElements(container)[0]
        if (first) first.focus()
        else container.focus()
      }, 50)
      return () => clearTimeout(timer)
    }
  }, [enabled, autoFocus, restoreFocus, ref])

  useEffect(() => {
    if (!enabled || !ref.current) return
    const container = ref.current

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return
      const focusables = getFocusableElements(container)
      if (focusables.length === 0) {
        e.preventDefault()
        return
      }
      const first = focusables[0]!
      const last = focusables[focusables.length - 1]!
      const active = document.activeElement
      const inside =
        active instanceof HTMLElement && container.contains(active)

      if (e.shiftKey) {
        if (!inside || active === first) {
          e.preventDefault()
          last.focus()
        }
      } else if (!inside || active === last) {
        e.preventDefault()
        first.focus()
      }
    }

    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('keydown', onKeyDown)
      if (restoreFocus) {
        prevFocusRef.current?.focus()
        prevFocusRef.current = null
      }
    }
  }, [enabled, restoreFocus, ref])
}
