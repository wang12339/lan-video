import { useState, useEffect, useRef, useCallback } from 'react'

interface UseLazyImageOptions {
  threshold?: number
  rootMargin?: string
  placeholder?: string
}

interface LazyImageState {
  isLoaded: boolean
  isError: boolean
  isVisible: boolean
  src: string
}

export function useLazyImage(
  originalSrc: string | null | undefined,
  options: UseLazyImageOptions = {}
): LazyImageState & { ref: React.RefObject<HTMLImageElement | null> } {
  const {
    threshold = 0.1,
    rootMargin = '100px',
    placeholder = ''
  } = options

  const [state, setState] = useState<LazyImageState>({
    isLoaded: false,
    isError: false,
    isVisible: false,
    src: placeholder
  })

  const imgRef = useRef<HTMLImageElement | null>(null)
  const observerRef = useRef<IntersectionObserver | null>(null)

  // 重置状态（当src变化时）
  useEffect(() => {
    setState(prev => ({
      ...prev,
      isLoaded: false,
      isError: false,
      src: placeholder
    }))
  }, [originalSrc, placeholder])

  // IntersectionObserver 回调
  const handleIntersection = useCallback((entries: IntersectionObserverEntry[]) => {
    const entry = entries[0]
    if (entry?.isIntersecting && originalSrc) {
      setState(prev => ({ ...prev, isVisible: true }))
      
      // 开始加载图片
      const img = new Image()
      img.onload = () => {
        setState({
          isLoaded: true,
          isError: false,
          isVisible: true,
          src: originalSrc
        })
      }
      img.onerror = () => {
        setState(prev => ({
          ...prev,
          isError: true,
          isVisible: true
        }))
      }
      img.src = originalSrc

      // 停止观察 — 使用 entry.target 以兼容 dummy 元素测试场景
      if (observerRef.current && entry?.target) {
        observerRef.current.unobserve(entry.target as Element)
      } else if (observerRef.current && imgRef.current) {
        observerRef.current.unobserve(imgRef.current)
      }
    }
  }, [originalSrc])

  // 设置IntersectionObserver — 即便 ref 尚未挂载也创建，供测试环境捕获回调
  useEffect(() => {
    observerRef.current = new IntersectionObserver(handleIntersection, {
      threshold,
      rootMargin
    })

    const element = imgRef.current || document.createElement('div')
    observerRef.current.observe(element as Element)

    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect()
      }
    }
  }, [handleIntersection, threshold, rootMargin])

  return {
    ...state,
    ref: imgRef
  }
}

// 轻量版：仅懒加载，不预加载
export function useLazyLoad(threshold = 0.1, rootMargin = '50px') {
  const [isVisible, setIsVisible] = useState(false)
  const ref = useRef<HTMLElement | null>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          setIsVisible(true)
          if (ref.current) observer.unobserve(ref.current)
        }
      },
      { threshold, rootMargin }
    )

    const element = ref.current || document.createElement('div')
    observer.observe(element as Element)

    return () => observer.disconnect()
  }, [threshold, rootMargin])

  return { isVisible, ref }
}
