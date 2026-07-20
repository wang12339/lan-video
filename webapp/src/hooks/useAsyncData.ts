import { useState, useEffect, useCallback, useRef } from 'react'

interface UseAsyncDataResult<T> {
  data: T | null
  loading: boolean
  error: unknown
  refresh: () => void
}

export function useAsyncData<T>(
  fetcher: () => Promise<T>,
  deps: unknown[] = [],
): UseAsyncDataResult<T> {
  const [data, setData] = useState<T | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<unknown>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const mountedRef = useRef(true)

  const refresh = useCallback(() => {
    setRefreshKey((k) => k + 1)
  }, [])

  useEffect(() => {
    mountedRef.current = true
    let cancelled = false
    setLoading(true)
    setError(null)

    fetcher()
      .then((d) => {
        if (!cancelled && mountedRef.current) {
          setData(d)
          setLoading(false)
        }
      })
      .catch((e) => {
        if (!cancelled && mountedRef.current) {
          setError(e)
          setLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey, ...deps])

  return { data, loading, error, refresh }
}
