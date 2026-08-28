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
  const observerRef = useRef<IntersectionObserver | null>(null)

  hasMoreRef.current = hasMore
  loadingRef.current = loading
  callbackRef.current = onLoadMore

  useEffect(() => {
    const el = sentinelRef.current
    if (!el) return

    if (observerRef.current) {
      observerRef.current.disconnect()
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && hasMoreRef.current && !loadingRef.current) {
          callbackRef.current()
        }
      },
      { rootMargin }
    )
    observerRef.current = observer
    observer.observe(el)
    return () => {
      observer.disconnect()
      observerRef.current = null
    }
  }, [sentinelRef, rootMargin])

  useEffect(() => {
    if (!hasMore && observerRef.current) {
      observerRef.current.disconnect()
      observerRef.current = null
    }
  }, [hasMore])
}
