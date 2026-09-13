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
  // 哈希 Worker 内动态 import hash-wasm 需要 ES module worker（IIFE 不支持
  // code-splitting）；现代浏览器均支持 { type: 'module' } worker。
  worker: {
    format: 'es',
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    target: 'es2020',
    cssCodeSplit: true,
    sourcemap: false,
    chunkSizeWarningLimit: 90,
    reportCompressedSize: true,
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
          if (id.includes('node_modules/i18next') || id.includes('node_modules/react-i18next')) {
            return 'i18n'
          }
          if (id.includes('node_modules/hls.js')) {
            return 'hls-vendor'
          }
        },
      },
    },
    esbuild: {
      drop: process.env.NODE_ENV === 'production' ? ['console', 'debugger'] : [],
      legalComments: 'none',
    },
    minify: 'esbuild',
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
      '/playlists': 'http://localhost:8082',
      '/comments': 'http://localhost:8082',
      // 公共聊天室：REST + WebSocket（ws 代理需 ws:true）
      '/chat': 'http://localhost:8082',
      '/ws': { target: 'ws://localhost:8082', ws: true },
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
  },
})
