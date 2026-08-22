import { useState, useEffect, useRef, useCallback, useMemo } from 'react'

/**
 * 虚拟列表配置选项
 */
interface VirtualListOptions {
  /** 每个列表项的固定高度（像素） */
  itemHeight: number
  /** 可视区域外额外渲染的项目数，用于减少滚动时的白屏闪烁，默认 5 */
  overscan?: number
}

/**
 * useVirtualList Hook 的返回值
 */
interface VirtualListResult<T> {
  /** 当前可见的虚拟列表项数组，包含索引、数据和绝对定位样式 */
  virtualItems: Array<{
    /** 该项在原始数据数组中的索引 */
    index: number
    /** 该项对应的数据 */
    data: T
    /** 用于绝对定位的 CSS 样式（position: absolute, top, height） */
    style: React.CSSProperties
  }>
  /** 虚拟列表的总高度（像素），用于撑开滚动容器 */
  totalHeight: number
  /** 绑定到滚动容器元素的 ref，必须传递给外层 div */
  containerRef: React.RefObject<HTMLDivElement | null>
  /** 滚动到指定索引位置（平滑滚动） */
  scrollToIndex: (index: number) => void
}

/**
 * 虚拟列表 Hook，通过只渲染可视区域内的列表项来优化长列表性能。
 *
 * 原理：监听容器滚动位置，计算当前可见的起止索引，仅渲染该范围内的元素，
 * 并通过绝对定位将其放置在正确位置，同时用 `totalHeight` 撑开容器保持滚动条正确。
 *
 * @typeParam T - 列表项数据的类型
 * @param items - 完整的数据数组
 * @param options - 配置项
 * @param options.itemHeight - 每个列表项的固定高度（像素）
 * @param options.overscan - 可视区域外额外渲染的项目数，默认 5
 * @returns 包含虚拟列表项、总高度、容器 ref 和滚动方法的对象
 *
 * @example
 * ```tsx
 * const { virtualItems, totalHeight, containerRef } = useVirtualList(data, {
 *   itemHeight: 60,
 *   overscan: 5,
 * })
 *
 * return (
 *   <div ref={containerRef} style={{ height: 400, overflow: 'auto' }}>
 *     <div style={{ height: totalHeight, position: 'relative' }}>
 *       {virtualItems.map(({ index, data, style }) => (
 *         <div key={index} style={style}>{data.name}</div>
 *       ))}
 *     </div>
 *   </div>
 * )
 * ```
 */
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

/**
 * 虚拟列表组件的 Props
 */
interface VirtualListProps<T> {
  /** 完整的数据数组 */
  items: T[]
  /** 每个列表项的固定高度（像素） */
  itemHeight: number
  /** 可视区域外额外渲染的项目数，默认 5 */
  overscan?: number
  /** 渲染单个列表项的函数，接收该项数据和索引，返回 React 节点 */
  renderItem: (item: T, index: number) => React.ReactNode
  /** 外层容器的 CSS 类名 */
  className?: string
  /** 外层容器的内联样式 */
  style?: React.CSSProperties
}

/**
 * 虚拟列表组件，基于 {@link useVirtualList} Hook 封装，提供开箱即用的虚拟滚动能力。
 *
 * 只渲染可视区域内的列表项，适用于大数据量场景（如数千条记录），
 * 能显著减少 DOM 节点数量，提升渲染和滚动性能。
 *
 * @typeParam T - 列表项数据的类型
 *
 * @example
 * ```tsx
 * <VirtualList
 *   items={largeDataArray}
 *   itemHeight={60}
 *   overscan={5}
 *   renderItem={(item, index) => (
 *     <div className="list-item">{item.name}</div>
 *   )}
 *   style={{ height: 400 }}
 * />
 * ```
 */
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
