import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

/** 行高估算函数的入参 */
export interface RowHeightContext {
  /** 容器内容宽度（px） */
  containerWidth: number
  /** 当前列数 */
  columns: number
  /** 行列间距（px） */
  gap: number
}

/** 行高估算：固定值或按容器宽度/列数动态计算 */
export type RowHeightEstimator = number | ((context: RowHeightContext) => number)

/** useVirtualGrid 配置选项 */
export interface VirtualGridOptions {
  /** 数据总条数 */
  itemCount: number
  /** 单列最小宽度（与 CSS minmax 的第一个参数保持一致），用于按容器宽度估算列数 */
  minItemWidth: number
  /** 估算行高（卡片自身高度，不含行间距） */
  rowHeight: RowHeightEstimator
  /** 行列间距（与 CSS gap 保持一致），默认 0 */
  gap?: number
  /** 视口上下各额外渲染的行数，默认 2 */
  overscan?: number
  /** 固定列数（如单列列表视图）；传入后跳过测量与公式计算 */
  fixedColumns?: number
  /** 列数相关布局的标识（如 grid/wide 切换），变化时重新测量列数 */
  layoutKey?: string | number
  /**
   * 窗口化后，渲染窗口到达“已加载末尾”时触发。
   * 每个 itemCount 至多触发一次，避免同一批数据反复请求；
   * 窗口化失效（降级渲染全部）时不会触发，由外层哨兵负责。
   */
  onReachEnd?: () => void
}

/** useVirtualGrid 返回值 */
export interface VirtualGridResult {
  /** 绑定到网格容器的 ref（回调用法，直接 <div ref={containerRef}>） */
  containerRef: (node: HTMLDivElement | null) => void
  /** 是否已进入窗口化渲染；false 表示降级为渲染全部条目 */
  windowed: boolean
  /** 当前列数 */
  columns: number
  /** 首个渲染条目在完整数据中的下标 */
  startIndex: number
  /** 最后一个渲染条目在完整数据中的下标（含） */
  endIndex: number
  /** 需要渲染的条目下标数组；降级时为 0..itemCount-1 */
  virtualItems: number[]
  /** 网格顶部占位高度（px），用于撑起被跳过的行 */
  paddingTop: number
  /** 网格底部占位高度（px） */
  paddingBottom: number
}

/**
 * 按容器宽度与最小列宽计算列数，与 CSS `repeat(auto-fill, minmax(min, 1fr))` 的列数一致：
 * columns = max(1, floor((width + gap) / (min + gap)))
 */
export function computeGridColumns(
  containerWidth: number,
  minItemWidth: number,
  gap: number
): number {
  if (!(containerWidth > 0) || !(minItemWidth > 0)) return 1
  const safeGap = gap > 0 ? gap : 0
  return Math.max(1, Math.floor((containerWidth + safeGap) / (minItemWidth + safeGap)))
}

/**
 * 解析浏览器 getComputedStyle 返回的 grid-template-columns 实际轨道数。
 * 只接受已解析为具体尺寸（如 "300px 300px"）的结果；
 * 含 `repeat(`/`minmax(` 等未解析值或 `none` 时返回 null，交给公式兜底。
 */
export function parseGridColumnCount(value: string | null | undefined): number | null {
  if (!value) return null
  const trimmed = value.trim()
  if (!trimmed || trimmed === 'none' || trimmed.includes('(')) return null
  const tracks = trimmed.split(/\s+/)
  if (tracks.length === 0) return null
  for (const track of tracks) {
    if (!/^-?\d*\.?\d+(px|%|em|rem|vw|vh)?$/.test(track)) return null
  }
  return tracks.length
}

/** computeVisibleRange 入参 */
export interface VisibleRangeOptions {
  itemCount: number
  columns: number
  rowHeight: number
  gap: number
  /** 相对网格容器顶部的滚动偏移（可为负，表示容器尚在视口下方） */
  scrollTop: number
  viewportHeight: number
  overscan: number
}

/** computeVisibleRange 返回值 */
export interface VisibleRange {
  startRow: number
  endRow: number
  startIndex: number
  endIndex: number
}

/**
 * 计算需要渲染的行范围与条目范围（纯函数，便于单测）。
 * 行距 rowPitch = rowHeight + gap；不可见时由调用方决定降级。
 */
export function computeVisibleRange({
  itemCount,
  columns,
  rowHeight,
  gap,
  scrollTop,
  viewportHeight,
  overscan
}: VisibleRangeOptions): VisibleRange {
  const safeColumns = Math.max(1, Math.floor(columns) || 1)
  const totalRows = Math.ceil(Math.max(0, itemCount) / safeColumns)
  if (itemCount <= 0 || totalRows <= 0) {
    return { startRow: 0, endRow: -1, startIndex: 0, endIndex: -1 }
  }
  const rowPitch = Math.max(1, rowHeight + (gap > 0 ? gap : 0))
  const clampRow = (row: number) => Math.min(Math.max(row, 0), totalRows - 1)
  const firstVisibleRow = clampRow(Math.floor(scrollTop / rowPitch))
  const lastVisibleRow = clampRow(
    Math.floor((scrollTop + Math.max(0, viewportHeight) - 1) / rowPitch)
  )
  const safeOverscan = Math.max(0, Math.floor(overscan) || 0)
  const startRow = Math.max(0, firstVisibleRow - safeOverscan)
  const endRow = Math.min(totalRows - 1, lastVisibleRow + safeOverscan)
  return {
    startRow,
    endRow,
    startIndex: startRow * safeColumns,
    endIndex: Math.min(itemCount - 1, (endRow + 1) * safeColumns - 1)
  }
}

/** 计算网格上下占位高度（纯函数） */
export function computeRowSpacers(
  totalRows: number,
  startRow: number,
  endRow: number,
  rowPitch: number
): { paddingTop: number; paddingBottom: number } {
  if (totalRows <= 0 || endRow < startRow || rowPitch <= 0) {
    return { paddingTop: 0, paddingBottom: 0 }
  }
  return {
    paddingTop: startRow * rowPitch,
    paddingBottom: (totalRows - 1 - endRow) * rowPitch
  }
}

/** 读取浏览器解析后的实际列数（媒体查询强制列数时也能取准），失败返回 null */
function measureGridColumnCount(el: HTMLElement): number | null {
  if (typeof window === 'undefined' || typeof window.getComputedStyle !== 'function') return null
  try {
    return parseGridColumnCount(window.getComputedStyle(el).gridTemplateColumns)
  } catch {
    return null
  }
}

interface GridMetrics {
  /** 网格容器顶部相对文档的绝对偏移（px） */
  containerTop: number
  /** 容器宽度（px），为 0 表示无法测量 */
  containerWidth: number
  /** 视口高度（px），为 0 表示无法测量 */
  viewportHeight: number
  /** 窗口滚动位置（px） */
  scrollTop: number
}

const EMPTY_METRICS: GridMetrics = {
  containerTop: 0,
  containerWidth: 0,
  viewportHeight: 0,
  scrollTop: 0
}

/**
 * 响应式网格虚拟化 Hook（窗口滚动，非独立滚动容器）。
 *
 * 设计要点：
 * 1. 列数：优先读取 `getComputedStyle(gridTemplateColumns)` 的实际轨道数，
 *    失败时用容器宽度 + 最小列宽按 auto-fill 公式兜底；`fixedColumns` 可强制覆盖。
 * 2. 可见范围：以窗口滚动位置、容器文档偏移、视口高度和估算行距计算行范围，
 *    上下各多渲染 overscan 行；跳过的行用 paddingTop/paddingBottom 撑起，
 *    因此文档总高度基本保持不变，底部哨兵与滚动位置恢复依旧可用。
 * 3. 降级：容器宽度或视口高度为 0（jsdom、初始渲染、display:none）时
 *    `windowed=false`，渲染全部条目，测量到有效尺寸后自动切换为窗口化。
 * 4. 加载：窗口化后渲染窗口触达 itemCount-1 时调用 `onReachEnd`（每个 itemCount 一次）；
 *    降级时交给外层 IntersectionObserver 哨兵。
 */
export function useVirtualGrid(options: VirtualGridOptions): VirtualGridResult {
  const {
    itemCount,
    minItemWidth,
    rowHeight,
    gap = 0,
    overscan = 2,
    fixedColumns,
    layoutKey,
    onReachEnd
  } = options

  const [container, setContainer] = useState<HTMLDivElement | null>(null)
  const [metrics, setMetrics] = useState<GridMetrics>(EMPTY_METRICS)

  const containerRef = useCallback((node: HTMLDivElement | null) => {
    setContainer(node)
  }, [])

  const onReachEndRef = useRef(onReachEnd)
  onReachEndRef.current = onReachEnd

  // 监听窗口滚动/尺寸变化与容器尺寸变化，刷新测量值
  useEffect(() => {
    if (!container) return

    let frame = 0
    const measure = () => {
      frame = 0
      const rect = container.getBoundingClientRect()
      const docEl = document.documentElement
      const scrollTop = window.scrollY || docEl.scrollTop || 0
      const next: GridMetrics = {
        containerTop: rect.top + scrollTop,
        containerWidth: container.clientWidth,
        viewportHeight: docEl.clientHeight || window.innerHeight || 0,
        scrollTop
      }
      setMetrics((prev) =>
        prev.containerTop === next.containerTop &&
        prev.containerWidth === next.containerWidth &&
        prev.viewportHeight === next.viewportHeight &&
        prev.scrollTop === next.scrollTop
          ? prev
          : next
      )
    }
    const schedule = () => {
      if (frame) return
      frame = requestAnimationFrame(measure)
    }

    measure()
    window.addEventListener('scroll', schedule, { passive: true })
    window.addEventListener('resize', schedule)

    let observer: ResizeObserver | undefined
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(schedule)
      observer.observe(container)
      // 网格上方内容（如热门区块）高度变化会改变容器文档偏移，一并观察
      if (document.body && document.body !== container) observer.observe(document.body)
    }

    return () => {
      if (frame) cancelAnimationFrame(frame)
      window.removeEventListener('scroll', schedule)
      window.removeEventListener('resize', schedule)
      observer?.disconnect()
    }
  }, [container])

  // 列数：固定列数 > 浏览器实测 > 公式估算
  const columns = useMemo(() => {
    if (fixedColumns != null && fixedColumns > 0) return Math.floor(fixedColumns)
    if (metrics.containerWidth <= 0) return 1
    // layoutKey 仅用于布局切换（grid/wide、grid/list）时强制重新测量
    void layoutKey
    const measured = container ? measureGridColumnCount(container) : null
    if (measured != null) return measured
    return computeGridColumns(metrics.containerWidth, minItemWidth, gap)
  }, [fixedColumns, metrics.containerWidth, container, layoutKey, minItemWidth, gap])

  // 估算行高（不含行间距），无效值视为无法测量
  const resolvedRowHeight = useMemo(() => {
    const value = typeof rowHeight === 'function'
      ? rowHeight({ containerWidth: metrics.containerWidth, columns, gap })
      : rowHeight
    return Number.isFinite(value) && value > 0 ? value : 0
  }, [rowHeight, metrics.containerWidth, columns, gap])

  const windowed =
    metrics.containerWidth > 0 && metrics.viewportHeight > 0 && resolvedRowHeight > 0 && itemCount > 0

  const range = useMemo(() => {
    if (!windowed) {
      return { startRow: 0, endRow: -1, startIndex: 0, endIndex: itemCount - 1 }
    }
    return computeVisibleRange({
      itemCount,
      columns,
      rowHeight: resolvedRowHeight,
      gap,
      scrollTop: metrics.scrollTop - metrics.containerTop,
      viewportHeight: metrics.viewportHeight,
      overscan
    })
  }, [
    windowed,
    itemCount,
    columns,
    resolvedRowHeight,
    gap,
    metrics.scrollTop,
    metrics.containerTop,
    metrics.viewportHeight,
    overscan
  ])

  const virtualItems = useMemo(() => {
    if (itemCount <= 0) return []
    if (!windowed) return Array.from({ length: itemCount }, (_, i) => i)
    const length = Math.max(0, range.endIndex - range.startIndex + 1)
    return Array.from({ length }, (_, offset) => range.startIndex + offset)
  }, [itemCount, windowed, range.startIndex, range.endIndex])

  const { paddingTop, paddingBottom } = useMemo(() => {
    if (!windowed) return { paddingTop: 0, paddingBottom: 0 }
    const totalRows = Math.ceil(itemCount / Math.max(1, columns))
    const rowPitch = resolvedRowHeight + (gap > 0 ? gap : 0)
    return computeRowSpacers(totalRows, range.startRow, range.endRow, rowPitch)
  }, [windowed, itemCount, columns, resolvedRowHeight, gap, range.startRow, range.endRow])

  // 渲染窗口到达已加载末尾时触发加载；每个 itemCount 至多一次，离开末尾后重置
  const reachedEnd = windowed && itemCount > 0 && range.endIndex >= itemCount - 1
  const lastTriggeredCountRef = useRef(-1)
  useEffect(() => {
    if (!reachedEnd) {
      lastTriggeredCountRef.current = -1
      return
    }
    if (lastTriggeredCountRef.current === itemCount) return
    lastTriggeredCountRef.current = itemCount
    onReachEndRef.current?.()
  }, [reachedEnd, itemCount])

  return {
    containerRef,
    windowed,
    columns,
    startIndex: range.startIndex,
    endIndex: range.endIndex,
    virtualItems,
    paddingTop,
    paddingBottom
  }
}
