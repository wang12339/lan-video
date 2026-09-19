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
    // 以下 chunk 均无法再拆到 90kB 以下，故放宽阈值并在此说明：
    // - react-vendor：react + react-dom 单一职责，必需依赖
    // - hls-vendor：hls.js 为单个库，且只在懒加载的 /player 路由引入，不影响首屏
    // - index：入口 chunk 承载路由/上下文/API 等首屏必需代码，拆无可拆
    chunkSizeWarningLimit: 400,
    reportCompressedSize: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/react-dom') || id.includes('node_modules/react/')) {
            return 'react-vendor'
          }
          // react-router v7 主实现位于 node_modules/react-router，react-router-dom 仅做转发
          if (
            id.includes('node_modules/react-router-dom') ||
            id.includes('node_modules/react-router/') ||
            id.includes('node_modules/@remix-run')
          ) {
            return 'router'
          }
          // query-core 位于 node_modules/@tanstack，一并归入 query 分包
          if (id.includes('node_modules/@tanstack')) {
            return 'query'
          }
          if (id.includes('node_modules/i18next') || id.includes('node_modules/react-i18next')) {
            return 'i18n'
          }
          if (id.includes('node_modules/hls.js')) {
            return 'hls-vendor'
          }
          // 其余 node_modules 兜底进 vendor：业务代码迭代不会让第三方依赖
          // 的缓存指纹随之变化，也避免未分类依赖散落进入口 chunk
          if (id.includes('node_modules')) {
            return 'vendor'
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
      '/track': 'http://localhost:8082',
      '/client-errors': 'http://localhost:8082',
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
