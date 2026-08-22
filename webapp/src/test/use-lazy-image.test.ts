import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useLazyImage, useLazyLoad } from '../hooks/useLazyImage'

// Mock IntersectionObserver
const mockObserve = vi.fn()
const mockUnobserve = vi.fn()
const mockDisconnect = vi.fn()

beforeEach(() => {
  global.IntersectionObserver = vi.fn().mockImplementation(function(callback: any) {
    return {
      observe: mockObserve,
      unobserve: mockUnobserve,
      disconnect: mockDisconnect,
      callback
    } as any
  })
  // Mock Image to synchronously trigger onload for tests
  class MockImage {
    onload: (() => void) | null = null
    onerror: (() => void) | null = null
    _src = ''
    set src(v: string) {
      this._src = v
      // 同步触发 onload 以满足测试预期
      if (v) this.onload?.()
    }
    get src() { return this._src }
  }
  ;(global as any).Image = MockImage
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('useLazyImage', () => {
  describe('懒加载', () => {
    it('初始状态应为未加载', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      expect(result.current.isLoaded).toBe(false)
      expect(result.current.isError).toBe(false)
      expect(result.current.isVisible).toBe(false)
      expect(result.current.src).toBe('')
    })

    it('当 originalSrc 为 null 时，src 应为 placeholder', () => {
      const placeholder = 'https://example.com/placeholder.jpg'
      const { result } = renderHook(() =>
        useLazyImage(null, { placeholder })
      )

      expect(result.current.src).toBe(placeholder)
      expect(result.current.isVisible).toBe(false)
    })

    it('当 originalSrc 变化时应重置状态', () => {
      const { result, rerender } = renderHook(
        ({ src }) => useLazyImage(src),
        { initialProps: { src: 'https://example.com/image1.jpg' } }
      )

      // 模拟图片加载
      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })
      }

      expect(result.current.isLoaded).toBe(true)

      // 切换 src
      rerender({ src: 'https://example.com/image2.jpg' })

      expect(result.current.isLoaded).toBe(false)
      expect(result.current.isError).toBe(false)
      expect(result.current.src).toBe('')
    })

    it('当 placeholder 变化时应重置 src', () => {
      const { result, rerender } = renderHook(
        ({ placeholder }) => useLazyImage('https://example.com/image.jpg', { placeholder }),
        { initialProps: { placeholder: 'placeholder1.jpg' } }
      )

      expect(result.current.src).toBe('placeholder1.jpg')

      rerender({ placeholder: 'placeholder2.jpg' })

      expect(result.current.src).toBe('placeholder2.jpg')
    })
  })

  describe('IntersectionObserver', () => {
    it('应创建 IntersectionObserver 并观察元素', () => {
      renderHook(() => useLazyImage('https://example.com/image.jpg'))

      expect(global.IntersectionObserver).toHaveBeenCalledWith(
        expect.any(Function),
        expect.objectContaining({
          threshold: 0.1,
          rootMargin: '100px'
        })
      )
      expect(mockObserve).toHaveBeenCalled()
    })

    it('应使用自定义 threshold 和 rootMargin', () => {
      renderHook(() =>
        useLazyImage('https://example.com/image.jpg', {
          threshold: 0.5,
          rootMargin: '200px'
        })
      )

      expect(global.IntersectionObserver).toHaveBeenCalledWith(
        expect.any(Function),
        expect.objectContaining({
          threshold: 0.5,
          rootMargin: '200px'
        })
      )
    })

    it('元素进入视口时应设置 isVisible 为 true', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })
      }

      expect(result.current.isVisible).toBe(true)
    })

    it('元素未进入视口时不应设置 isVisible', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: false, target: document.createElement("div") }])
        })
      }

      expect(result.current.isVisible).toBe(false)
    })

    it('元素进入视口后应停止观察', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })
      }

      expect(mockUnobserve).toHaveBeenCalled()
    })

    it('组件卸载时应断开观察器', () => {
      const { unmount } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      unmount()

      expect(mockDisconnect).toHaveBeenCalled()
    })
  })

  describe('加载完成回调', () => {
    it('图片加载成功时应设置 isLoaded 和 src', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      // 模拟 Image 构造函数
      const mockImageInstance = {
        onload: null as (() => void) | null,
        onerror: null as (() => void) | null,
        src: ''
      }
      const originalImage = global.Image
      global.Image = vi.fn(function() { return mockImageInstance as any }) as any

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })

        // 模拟图片加载成功
        act(() => {
          if (mockImageInstance.onload) {
            mockImageInstance.onload()
          }
        })
      }

      expect(result.current.isLoaded).toBe(true)
      expect(result.current.isError).toBe(false)
      expect(result.current.isVisible).toBe(true)
      expect(result.current.src).toBe('https://example.com/image.jpg')

      global.Image = originalImage
    })

    it('图片加载失败时应设置 isError', () => {
      const { result } = renderHook(() =>
        useLazyImage('https://example.com/image.jpg')
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      // 模拟 Image 构造函数
      const mockImageInstance = {
        onload: null as (() => void) | null,
        onerror: null as (() => void) | null,
        src: ''
      }
      const originalImage = global.Image
      global.Image = vi.fn(function() { return mockImageInstance as any }) as any

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })

        // 模拟图片加载失败
        act(() => {
          if (mockImageInstance.onerror) {
            mockImageInstance.onerror()
          }
        })
      }

      expect(result.current.isLoaded).toBe(false)
      expect(result.current.isError).toBe(true)
      expect(result.current.isVisible).toBe(true)
      expect(result.current.src).toBe('')

      global.Image = originalImage
    })

    it('图片加载成功后应更新 src 为原始 URL', () => {
      const originalSrc = 'https://example.com/actual-image.jpg'
      const placeholder = 'https://example.com/placeholder.jpg'
      const { result } = renderHook(() =>
        useLazyImage(originalSrc, { placeholder })
      )

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      const mockImageInstance = {
        onload: null as (() => void) | null,
        onerror: null as (() => void) | null,
        src: ''
      }
      const originalImage = global.Image
      global.Image = vi.fn(function() { return mockImageInstance as any }) as any

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })

        act(() => {
          if (mockImageInstance.onload) {
            mockImageInstance.onload()
          }
        })
      }

      expect(result.current.src).toBe(originalSrc)
      expect(result.current.src).not.toBe(placeholder)

      global.Image = originalImage
    })
  })
})

describe('useLazyLoad', () => {
  describe('懒加载', () => {
    it('初始状态应为未可见', () => {
      const { result } = renderHook(() => useLazyLoad())

      expect(result.current.isVisible).toBe(false)
    })

    it('应创建 IntersectionObserver 并观察元素', () => {
      renderHook(() => useLazyLoad())

      expect(global.IntersectionObserver).toHaveBeenCalledWith(
        expect.any(Function),
        expect.objectContaining({
          threshold: 0.1,
          rootMargin: '50px'
        })
      )
      expect(mockObserve).toHaveBeenCalled()
    })

    it('应使用自定义 threshold 和 rootMargin', () => {
      renderHook(() => useLazyLoad(0.5, '200px'))

      expect(global.IntersectionObserver).toHaveBeenCalledWith(
        expect.any(Function),
        expect.objectContaining({
          threshold: 0.5,
          rootMargin: '200px'
        })
      )
    })

    it('元素进入视口时应设置 isVisible 为 true', () => {
      const { result } = renderHook(() => useLazyLoad())

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })
      }

      expect(result.current.isVisible).toBe(true)
    })

    it('元素未进入视口时不应设置 isVisible', () => {
      const { result } = renderHook(() => useLazyLoad())

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: false, target: document.createElement("div") }])
        })
      }

      expect(result.current.isVisible).toBe(false)
    })

    it('元素进入视口后应停止观察', () => {
      const { result } = renderHook(() => useLazyLoad())

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })
      }

      expect(mockUnobserve).toHaveBeenCalled()
    })

    it('组件卸载时应断开观察器', () => {
      const { unmount } = renderHook(() => useLazyLoad())

      unmount()

      expect(mockDisconnect).toHaveBeenCalled()
    })

    it('当 isVisible 为 true 后应保持 true', () => {
      const { result } = renderHook(() => useLazyLoad())

      const observerCallback = (global.IntersectionObserver as any).mock.calls[0][0]
      const mockElement = result.current.ref.current

      if (true) {
        act(() => {
          observerCallback([{ isIntersecting: true, target: document.createElement("div") }])
        })

        expect(result.current.isVisible).toBe(true)

        // 再次触发未进入视口的情况
        act(() => {
          observerCallback([{ isIntersecting: false, target: document.createElement("div") }])
        })

        expect(result.current.isVisible).toBe(true)
      }
    })
  })
})
