import { QueryClient } from '@tanstack/react-query'

interface ErrorWithStatus {
  status?: number
  response?: { status?: number }
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      gcTime: 5 * 60_000,
      retry: (failureCount, error) => {
        const err = error as unknown as ErrorWithStatus | null | undefined
        const status = typeof err?.status === 'number' ? err.status : err?.response?.status
        if (typeof status === 'number' && status >= 400 && status < 500) {
          return false
        }
        return failureCount < 2
      },
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30_000),
      refetchOnWindowFocus: true,
    },
    mutations: {
      retry: 0,
    },
  },
})
