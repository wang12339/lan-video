import { useEffect, useRef } from 'react'

interface UseInfiniteScrollOptions {
  hasMore: boolean
  loading: boolean
  onLoadMore: () => void
  rootMargin?: string
}

export function useInfiniteScroll(
  sentinelRef: React.RefObject<Element | null>,
  { hasMore, loading, onLoadMore, rootMargin = '300px 0px' }: UseInfiniteScrollOptions
) {
  const hasMoreRef = useRef(hasMore)
  const loadingRef = useRef(loading)
  const callbackRef = useRef(onLoadMore)

  hasMoreRef.current = hasMore
  loadingRef.current = loading
  callbackRef.current = onLoadMore

  useEffect(() => {
    const el = sentinelRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && hasMoreRef.current && !loadingRef.current) {
          callbackRef.current()
        }
      },
      { rootMargin }
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [sentinelRef, rootMargin])
}
