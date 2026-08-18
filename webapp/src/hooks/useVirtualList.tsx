import { useState, useEffect, useRef, useCallback, useMemo } from 'react'

interface VirtualListOptions {
  itemHeight: number
  overscan?: number // 额外渲染的项目数
}

interface VirtualListResult<T> {
  virtualItems: Array<{
    index: number
    data: T
    style: React.CSSProperties
  }>
  totalHeight: number
  containerRef: React.RefObject<HTMLDivElement | null>
  scrollToIndex: (index: number) => void
}

export function useVirtualList<T>(
  items: T[],
  options: VirtualListOptions
): VirtualListResult<T> {
  const { itemHeight, overscan = 5 } = options
  const [scrollTop, setScrollTop] = useState(0)
  const [containerHeight, setContainerHeight] = useState(0)
  const containerRef = useRef<HTMLDivElement | null>(null)

  // 监听滚动
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const handleScroll = () => {
      setScrollTop(container.scrollTop)
    }

    const handleResize = () => {
      setContainerHeight(container.clientHeight)
    }

    container.addEventListener('scroll', handleScroll, { passive: true })
    window.addEventListener('resize', handleResize)
    handleResize()

    return () => {
      container.removeEventListener('scroll', handleScroll)
      window.removeEventListener('resize', handleResize)
    }
  }, [])

  // 计算可见范围
  const { startIndex, endIndex } = useMemo(() => {
    const start = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan)
    const visibleCount = Math.ceil(containerHeight / itemHeight)
    const end = Math.min(items.length - 1, start + visibleCount + overscan * 2)
    return { startIndex: start, endIndex: end }
  }, [scrollTop, containerHeight, itemHeight, overscan, items.length])

  // 生成虚拟列表项
  const virtualItems = useMemo(() => {
    const result: Array<{
      index: number
      data: T
      style: React.CSSProperties
    }> = []

    for (let i = startIndex; i <= endIndex; i++) {
      if (i < items.length) {
        result.push({
          index: i,
          data: items[i] as T,
          style: {
            position: 'absolute',
            top: i * itemHeight,
            left: 0,
            right: 0,
            height: itemHeight
          }
        })
      }
    }

    return result
  }, [startIndex, endIndex, items, itemHeight])

  // 滚动到指定索引
  const scrollToIndex = useCallback((index: number) => {
    const container = containerRef.current
    if (!container) return

    const top = index * itemHeight
    container.scrollTo({ top, behavior: 'smooth' })
  }, [itemHeight])

  const totalHeight = items.length * itemHeight

  return {
    virtualItems,
    totalHeight,
    containerRef,
    scrollToIndex
  }
}

// 虚拟列表组件
interface VirtualListProps<T> {
  items: T[]
  itemHeight: number
  overscan?: number
  renderItem: (item: T, index: number) => React.ReactNode
  className?: string
  style?: React.CSSProperties
}

export function VirtualList<T>({
  items,
  itemHeight,
  overscan,
  renderItem,
  className,
  style
}: VirtualListProps<T>) {
  const { virtualItems, totalHeight, containerRef } = useVirtualList(items, {
    itemHeight,
    overscan
  })

  return (
    <div
      ref={containerRef as React.RefObject<HTMLDivElement>}
      className={className}
      style={{
        ...style,
        overflow: 'auto',
        position: 'relative'
      }}
    >
      <div style={{ height: totalHeight, position: 'relative' }}>
        {virtualItems.map(({ index, data, style: itemStyle }) => (
          <div key={index} style={itemStyle}>
            {renderItem(data, index)}
          </div>
        ))}
      </div>
    </div>
  )
}
