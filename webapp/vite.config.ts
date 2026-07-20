/// <reference types="vitest" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  base: '/webapp/',
  plugins: [
    react(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes('node_modules/react-dom') || id.includes('node_modules/react/')) {
              return 'react-vendor'
            }
            if (id.includes('node_modules/react-router-dom') || id.includes('node_modules/@remix-run')) {
              return 'router'
            }
            if (id.includes('node_modules/@tanstack/react-query')) {
              return 'query'
            }
          },
        },
      },
  },
  server: {
    port: 5173,
    proxy: {
      '/videos': 'http://localhost:8082',
      '/auth': 'http://localhost:8082',
      '/admin': 'http://localhost:8082',
      '/playback': 'http://localhost:8082',
      '/media': 'http://localhost:8082',
      '/health': 'http://localhost:8082',
      '/server': 'http://localhost:8082',
      '/docs': 'http://localhost:8082',
      '/tags': 'http://localhost:8082',
      '/recommendations': 'http://localhost:8082',
      '/share': 'http://localhost:8082',
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
  },
})
