import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useNetworkState, useOfflineAlert } from '../hooks/useNetworkState'

describe('useNetworkState', () => {
  let originalNavigator: Navigator
  
  beforeEach(() => {
    // 保存原始 navigator
    originalNavigator = { ...navigator }
    
    // Mock navigator.onLine
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: true
    })
    
    // Mock navigator.connection
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: {
        effectiveType: '4g',
        downlink: 10,
        rtt: 50,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }
    })
  })
  
  afterEach(() => {
    // 恢复 navigator
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: originalNavigator.onLine
    })
    
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: originalNavigator.connection
    })
    
    vi.restoreAllMocks()
  })

  it('应该返回初始在线状态', () => {
    const { result } = renderHook(() => useNetworkState())

    expect(result.current.isOnline).toBe(true)
    expect(result.current.isSlowConnection).toBe(false)
    expect(result.current.connectionType).toBe('4g')
    expect(result.current.downlink).toBe(10)
    expect(result.current.rtt).toBe(50)
  })

  it('应该在离线时更新状态', () => {
    const { result } = renderHook(() => useNetworkState())

    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.isOnline).toBe(false)
  })

  it('应该在上线时更新状态', () => {
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: false
    })
    
    const { result } = renderHook(() => useNetworkState())

    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: true
      })
      window.dispatchEvent(new Event('online'))
    })

    expect(result.current.isOnline).toBe(true)
  })

  it('应该检测慢速连接（slow-2g）', () => {
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: {
        effectiveType: 'slow-2g',
        downlink: 10,
        rtt: 50,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }
    })
    
    const { result } = renderHook(() => useNetworkState())

    expect(result.current.isSlowConnection).toBe(true)
    expect(result.current.connectionType).toBe('slow-2g')
  })

  it('应该检测慢速连接（2g）', () => {
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: {
        effectiveType: '2g',
        downlink: 10,
        rtt: 50,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }
    })
    
    const { result } = renderHook(() => useNetworkState())

    expect(result.current.isSlowConnection).toBe(true)
    expect(result.current.connectionType).toBe('2g')
  })

  it('应该检测慢速连接（downlink < 1.5）', () => {
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: {
        effectiveType: '4g',
        downlink: 1.0,
        rtt: 50,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }
    })
    
    const { result } = renderHook(() => useNetworkState())

    expect(result.current.isSlowConnection).toBe(true)
    expect(result.current.downlink).toBe(1.0)
  })

  it('应该处理 connection API 不存在的情况', () => {
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: undefined
    })
    
    const { result } = renderHook(() => useNetworkState())

    expect(result.current.isOnline).toBe(true)
    expect(result.current.connectionType).toBe('unknown')
    expect(result.current.downlink).toBeNull()
    expect(result.current.rtt).toBeNull()
  })

  it('应该监听 connection change 事件', () => {
    const mockConnection = {
      effectiveType: '4g',
      downlink: 10,
      rtt: 50,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }
    
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: mockConnection
    })
    
    renderHook(() => useNetworkState())

    expect(mockConnection.addEventListener).toHaveBeenCalledWith(
      'change',
      expect.any(Function)
    )
  })

  it('应该在卸载时移除事件监听器', () => {
    const mockConnection = {
      effectiveType: '4g',
      downlink: 10,
      rtt: 50,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }
    
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: mockConnection
    })
    
    const { unmount } = renderHook(() => useNetworkState())

    unmount()

    expect(mockConnection.removeEventListener).toHaveBeenCalledWith(
      'change',
      expect.any(Function)
    )
  })

  it('应该响应 connection change 事件更新状态', () => {
    const mockConnection = {
      effectiveType: '4g',
      downlink: 10,
      rtt: 50,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }
    
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: mockConnection
    })
    
    const { result } = renderHook(() => useNetworkState())

    // 模拟 connection change 事件
    const changeHandler = mockConnection.addEventListener.mock.calls[0]?.[1]
    if (changeHandler) {
      mockConnection.effectiveType = '3g'
      mockConnection.downlink = 2
      mockConnection.rtt = 100

      act(() => {
        changeHandler()
      })

      expect(result.current.connectionType).toBe('3g')
      expect(result.current.downlink).toBe(2)
      expect(result.current.rtt).toBe(100)
      expect(result.current.isSlowConnection).toBe(false)
    }
  })
})

describe('useOfflineAlert', () => {
  let originalNavigator: Navigator
  
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers()
    
    // 保存原始 navigator
    originalNavigator = { ...navigator }
    
    // Mock navigator.onLine
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: true
    })
    
    // Mock navigator.connection
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: {
        effectiveType: '4g',
        downlink: 10,
        rtt: 50,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }
    })
  })
  
  afterEach(() => {
    vi.useRealTimers()
    
    // 恢复 navigator
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: originalNavigator.onLine
    })
    
    Object.defineProperty(navigator, 'connection', {
      writable: true,
      value: originalNavigator.connection
    })
    
    vi.restoreAllMocks()
  })

  it('应该在在线时不显示提示', () => {
    const { result } = renderHook(() => useOfflineAlert())

    expect(result.current.isOnline).toBe(true)
    expect(result.current.showAlert).toBe(false)
  })

  it('应该在离线时显示提示', () => {
    const { result } = renderHook(() => useOfflineAlert())

    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.isOnline).toBe(false)
    expect(result.current.showAlert).toBe(true)
  })

  it('应该在恢复在线时显示提示并在3秒后自动关闭', () => {
    Object.defineProperty(navigator, 'onLine', {
      writable: true,
      value: false
    })
    
    const { result } = renderHook(() => useOfflineAlert())

    // 先离线
    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.showAlert).toBe(true)

    // 恢复在线
    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: true
      })
      window.dispatchEvent(new Event('online'))
    })

    expect(result.current.showAlert).toBe(true)
    expect(result.current.isOnline).toBe(true)

    // 3秒后自动关闭
    act(() => {
      vi.advanceTimersByTime(3000)
    })

    expect(result.current.showAlert).toBe(false)
  })

  it('应该支持手动关闭提示', () => {
    const { result } = renderHook(() => useOfflineAlert())

    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.showAlert).toBe(true)

    act(() => {
      result.current.dismissAlert()
    })

    expect(result.current.showAlert).toBe(false)
  })

  it('应该在从未离线的情况下恢复在线时不显示提示', () => {
    const { result } = renderHook(() => useOfflineAlert())

    // 直接触发 online 事件（从未离线过）
    act(() => {
      window.dispatchEvent(new Event('online'))
    })

    expect(result.current.showAlert).toBe(false)
  })

  it('应该正确处理多次离线/在线切换', () => {
    const { result } = renderHook(() => useOfflineAlert())

    // 第一次离线
    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.showAlert).toBe(true)

    // 恢复在线
    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: true
      })
      window.dispatchEvent(new Event('online'))
    })

    expect(result.current.showAlert).toBe(true)

    // 再次离线
    act(() => {
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      })
      window.dispatchEvent(new Event('offline'))
    })

    expect(result.current.showAlert).toBe(true)
    expect(result.current.isOnline).toBe(false)
  })
})
