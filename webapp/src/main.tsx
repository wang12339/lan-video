import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { initTrackRouter } from './utils/track'
import { queryClient } from './lib/queryClient'
import './i18n'
import './styles/globals.css'

// 初始化用户操作追踪
initTrackRouter()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
)
