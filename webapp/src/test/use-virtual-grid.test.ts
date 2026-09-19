import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, fireEvent, renderHook, waitFor } from '@testing-library/react'
import {
  computeGridColumns,
  parseGridColumnCount,
  computeVisibleRange,
  computeRowSpacers,
  useVirtualGrid,
} from '../hooks/useVirtualGrid'

// ── 布局测量 stub（jsdom 无布局：clientWidth/clientHeight/getBoundingClientRect 全为 0） ──

let rectTop = 0
const originalDescriptors: Array<{ target: object; key: string; descriptor?: PropertyDescriptor }> = []

function overrideProperty(target: object, key: string, descriptor: PropertyDescriptor) {
  originalDescriptors.push({
    target,
    key,
    descriptor: Object.getOwnPropertyDescriptor(target, key),
  })
  Object.defineProperty(target, key, descriptor)
}

/** 模拟一个 1000x800 的真实浏览器视口 */
function installLayout() {
  rectTop = 0
  overrideProperty(Element.prototype, 'clientWidth', { configurable: true, get: () => 1000 })
  overrideProperty(Element.prototype, 'clientHeight', { configurable: true, get: () => 800 })
  overrideProperty(Element.prototype, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({
      top: rectTop,
      left: 0,
      right: 1000,
      bottom: 0,
      width: 1000,
      height: 0,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }),
  })
}

afterEach(() => {
  for (const { target, key, descriptor } of originalDescriptors.splice(0)) {
    if (descriptor) Object.defineProperty(target, key, descriptor)
    else Reflect.deleteProperty(target, key)
  }
  vi.restoreAllMocks()
})

// ── 列数计算 ──────────────────────────────────────────────────────────────────

describe('computeGridColumns', () => {
  it('按容器宽度与最小列宽计算列数', () => {
    expect(computeGridColumns(1000, 200, 0)).toBe(5)
    expect(computeGridColumns(1000, 220, 20)).toBe(4)
  })

  it('边界：刚好放不下下一列时取更少列', () => {
    // (575+16)/(280+16)=1.996 → 1；(576+16)/296=2 → 2
    expect(computeGridColumns(575, 280, 16)).toBe(1)
    expect(computeGridColumns(576, 280, 16)).toBe(2)
  })

  it('宽度或最小列宽非法时至少返回 1 列', () => {
    expect(computeGridColumns(0, 200, 10)).toBe(1)
    expect(computeGridColumns(-100, 200, 10)).toBe(1)
    expect(computeGridColumns(1000, 0, 10)).toBe(1)
  })

  it('负间距按 0 处理', () => {
    expect(computeGridColumns(1000, 200, -10)).toBe(5)
  })
})

describe('parseGridColumnCount', () => {
  it('解析浏览器解析后的 px 轨道', () => {
    expect(parseGridColumnCount('300px 300px 300px')).toBe(3)
    expect(parseGridColumnCount('  1000px ')).toBe(1)
  })

  it('未解析的声明值返回 null（交给公式兜底）', () => {
    expect(parseGridColumnCount('none')).toBeNull()
    expect(parseGridColumnCount('repeat(auto-fill, minmax(280px, 1fr))')).toBeNull()
    expect(parseGridColumnCount('320px auto')).toBeNull()
    expect(parseGridColumnCount('1fr')).toBeNull()
    expect(parseGridColumnCount('')).toBeNull()
    expect(parseGridColumnCount(null)).toBeNull()
  })
})

// ── 可见范围计算 ──────────────────────────────────────────────────────────────

describe('computeVisibleRange', () => {
  it('首屏：按行距取窗口并加 overscan', () => {
    const range = computeVisibleRange({
      itemCount: 100,
      columns: 5,
      rowHeight: 100,
      gap: 10,
      scrollTop: 0,
      viewportHeight: 500,
      overscan: 2,
    })
    expect(range.startRow).toBe(0)
    expect(range.endRow).toBe(6) // floor(499/110)=4 + 2
    expect(range.startIndex).toBe(0)
    expect(range.endIndex).toBe(34) // (6+1)*5-1
  })

  it('滚动到中部：起始行与结束行都随滚动前移', () => {
    const range = computeVisibleRange({
      itemCount: 100,
      columns: 5,
      rowHeight: 100,
      gap: 10,
      scrollTop: 1110, // 第 10 行
      viewportHeight: 220, // 覆盖约 2 行
      overscan: 2,
    })
    expect(range.startRow).toBe(8)
    expect(range.endRow).toBe(14)
    expect(range.startIndex).toBe(40)
    expect(range.endIndex).toBe(74)
  })

  it('尾部越界时收敛到最后一行/最后一条', () => {
    const range = computeVisibleRange({
      itemCount: 23,
      columns: 5,
      rowHeight: 100,
      gap: 0,
      scrollTop: 999999,
      viewportHeight: 800,
      overscan: 2,
    })
    expect(range.endRow).toBe(4) // ceil(23/5)-1
    expect(range.endIndex).toBe(22)
    expect(range.startIndex).toBeGreaterThanOrEqual(0)
  })

  it('容器尚在视口下方（负滚动偏移）时从第一行开始', () => {
    const range = computeVisibleRange({
      itemCount: 100,
      columns: 5,
      rowHeight: 100,
      gap: 10,
      scrollTop: -500,
      viewportHeight: 800,
      overscan: 2,
    })
    expect(range.startRow).toBe(0)
    expect(range.startIndex).toBe(0)
  })

  it('空列表返回 endIndex=-1', () => {
    const range = computeVisibleRange({
      itemCount: 0,
      columns: 3,
      rowHeight: 100,
      gap: 10,
      scrollTop: 0,
      viewportHeight: 800,
      overscan: 2,
    })
    expect(range.endIndex).toBe(-1)
    expect(range.endRow).toBe(-1)
  })

  it('行高非法时按 1px 行距兜底，不产生 NaN', () => {
    const range = computeVisibleRange({
      itemCount: 10,
      columns: 2,
      rowHeight: 0,
      gap: 0,
      scrollTop: 0,
      viewportHeight: 100,
      overscan: 0,
    })
    expect(Number.isFinite(range.startIndex)).toBe(true)
    expect(Number.isFinite(range.endIndex)).toBe(true)
    expect(range.endIndex).toBeGreaterThanOrEqual(0)
  })
})

describe('computeRowSpacers', () => {
  it('跳过的行换算为上下占位高度', () => {
    expect(computeRowSpacers(20, 5, 10, 110)).toEqual({ paddingTop: 550, paddingBottom: 990 })
  })

  it('无行或非法行距时占位为 0', () => {
    expect(computeRowSpacers(0, 0, -1, 100)).toEqual({ paddingTop: 0, paddingBottom: 0 })
    expect(computeRowSpacers(10, 3, 2, 100)).toEqual({ paddingTop: 0, paddingBottom: 0 })
    expect(computeRowSpacers(10, 0, 9, 0)).toEqual({ paddingTop: 0, paddingBottom: 0 })
  })
})

// ── Hook：降级 / 窗口化 / 触底加载 ────────────────────────────────────────────

describe('useVirtualGrid', () => {
  it('无法测量时降级渲染全部条目（jsdom 默认 clientWidth=0）', () => {
    const { result } = renderHook(() =>
      useVirtualGrid({ itemCount: 25, minItemWidth: 200, rowHeight: 100, gap: 10 })
    )
    const el = document.createElement('div')
    act(() => result.current.containerRef(el))

    expect(result.current.windowed).toBe(false)
    expect(result.current.virtualItems).toHaveLength(25)
    expect(result.current.virtualItems[0]).toBe(0)
    expect(result.current.virtualItems[24]).toBe(24)
    expect(result.current.paddingTop).toBe(0)
    expect(result.current.paddingBottom).toBe(0)
  })

  it('测量有效后窗口化渲染，并给出上下占位', () => {
    installLayout()
    const { result } = renderHook(() =>
      useVirtualGrid({ itemCount: 100, minItemWidth: 200, rowHeight: 100, gap: 0, overscan: 2 })
    )
    const el = document.createElement('div')
    act(() => result.current.containerRef(el))

    expect(result.current.windowed).toBe(true)
    expect(result.current.columns).toBe(5) // 1000 / 200
    expect(result.current.startIndex).toBe(0)
    expect(result.current.endIndex).toBe(49) // 首屏 8 行 + 上下各 2 行
    expect(result.current.virtualItems).toHaveLength(50)
    expect(result.current.paddingTop).toBe(0)
    expect(result.current.paddingBottom).toBe(1000) // 剩余 10 行
  })

  it('窗口滚动后更新可见范围', async () => {
    installLayout()
    const { result } = renderHook(() =>
      useVirtualGrid({ itemCount: 100, minItemWidth: 200, rowHeight: 100, gap: 0, overscan: 2 })
    )
    const el = document.createElement('div')
    act(() => result.current.containerRef(el))

    // 容器随页面滚动上移 900px，等价于相对滚动 900px
    rectTop = -900
    fireEvent.scroll(window)

    await waitFor(() => expect(result.current.startIndex).toBe(35))
    expect(result.current.endIndex).toBe(94)
    expect(result.current.paddingTop).toBe(700)
  })

  it('渲染窗口到达已加载末尾时触发 onReachEnd，且每个 itemCount 至多一次', async () => {
    installLayout()
    const onReachEnd = vi.fn()
    const { result, rerender } = renderHook(
      ({ count }: { count: number }) =>
        useVirtualGrid({
          itemCount: count,
          minItemWidth: 200,
          rowHeight: 100,
          gap: 0,
          overscan: 2,
          onReachEnd,
        }),
      { initialProps: { count: 30 } }
    )
    const el = document.createElement('div')
    act(() => result.current.containerRef(el))

    // 30 条全部落在首屏窗口内 → 触达末尾，触发一次
    expect(onReachEnd).toHaveBeenCalledTimes(1)

    // 追加数据后窗口不再触底，不重复触发
    rerender({ count: 100 })
    expect(onReachEnd).toHaveBeenCalledTimes(1)

    // 滚动到底部再次触达末尾 → 第二次触发
    rectTop = -100000
    fireEvent.scroll(window)
    await waitFor(() => expect(onReachEnd).toHaveBeenCalledTimes(2))
  })

  it('降级渲染全部时不触发 onReachEnd（交给外层哨兵）', () => {
    const onReachEnd = vi.fn()
    const { result } = renderHook(() =>
      useVirtualGrid({ itemCount: 30, minItemWidth: 200, rowHeight: 100, onReachEnd })
    )
    const el = document.createElement('div')
    act(() => result.current.containerRef(el))

    expect(result.current.windowed).toBe(false)
    expect(onReachEnd).not.toHaveBeenCalled()
  })
})
