import { QueryClient } from '@tanstack/react-query'

interface ErrorWithStatus {
  response?: { status?: number }
}

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      gcTime: 5 * 60_000,
      retry: (failureCount, error) => {
        const status = (error as unknown as ErrorWithStatus)?.response?.status
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
