import { useState, useEffect } from 'react'

export interface ThrottledFunction<T extends (...args: any[]) => any> {
  (...args: Parameters<T>): void
  cancel: () => void
}

export function throttle<T extends (...args: any[]) => any>(
  fn: T,
  wait: number
): ThrottledFunction<T> {
  let last = 0
  let timer: ReturnType<typeof setTimeout> | null = null
  let lastArgs: Parameters<T> | null = null

  const invoke = () => {
    last = Date.now()
    timer = null
    if (lastArgs) {
      const a = lastArgs
      lastArgs = null
      fn(...a)
    }
  }

  const throttled = (...args: Parameters<T>) => {
    const now = Date.now()
    const remaining = wait - (now - last)
    lastArgs = args
    if (remaining <= 0) {
      if (timer) { clearTimeout(timer); timer = null }
      last = now
      fn(...args)
    } else if (!timer) {
      timer = setTimeout(invoke, remaining)
    }
  }

  throttled.cancel = () => {
    if (timer) { clearTimeout(timer); timer = null }
    lastArgs = null
  }

  return throttled
}

export interface DebouncedFunction<T extends (...args: any[]) => any> {
  (...args: Parameters<T>): void
  cancel: () => void
  flush: () => void
}

export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  wait: number
): DebouncedFunction<T> {
  let timer: ReturnType<typeof setTimeout> | null = null
  let lastArgs: Parameters<T> | null = null

  const debounced = (...args: Parameters<T>) => {
    if (timer) clearTimeout(timer)
    lastArgs = args
    timer = setTimeout(() => {
      timer = null
      if (lastArgs) {
        const a = lastArgs
        lastArgs = null
        fn(...a)
      }
    }, wait)
  }

  debounced.cancel = () => {
    if (timer) { clearTimeout(timer); timer = null }
    lastArgs = null
  }

  debounced.flush = () => {
    if (timer) { clearTimeout(timer); timer = null }
    if (lastArgs) {
      const a = lastArgs
      lastArgs = null
      fn(...a)
    }
  }

  return debounced
}

export function useDebouncedValue<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay)
    return () => clearTimeout(timer)
  }, [value, delay])
  return debounced
}
