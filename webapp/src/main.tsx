import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { initTrackRouter } from './utils/track'
import { queryClient } from './lib/queryClient'
import './i18n'
import './styles/globals.css'

initTrackRouter()

if (import.meta.env.DEV) {
  performance.mark('app:start')
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
)

if (import.meta.env.DEV) {
  requestAnimationFrame(() => {
    performance.mark('app:mounted')
    performance.measure('app:render', 'app:start', 'app:mounted')
    const measure = performance.getEntriesByName('app:render')[0]
    if (measure) {
      console.log(`[Perf] Initial render: ${measure.duration.toFixed(1)}ms`)
    }
  })
}
