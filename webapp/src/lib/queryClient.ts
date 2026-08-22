import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // 数据被视为新鲜的时间（1分钟内不重新请求）
      staleTime: 60_000,
      // 缓存数据保留时间（5分钟，适合视频平台场景）
      gcTime: 5 * 60_000,
      // 重试策略：指数退避，最多重试2次，跳过4xx客户端错误
      retry: (failureCount, error: any) => {
        // 不重试客户端错误（4xx）
        if (error?.response?.status >= 400 && error?.response?.status < 500) {
          return false
        }
        // 最多重试2次
        return failureCount < 2
      },
      // 重试延迟：指数退避（1s, 2s）
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30_000),
      // 聚焦时重新获取（视频平台适合开启，确保数据最新）
      refetchOnWindowFocus: true,
    },
    mutations: {
      // mutations 不重试，避免重复提交
      retry: 0,
    },
  },
})
