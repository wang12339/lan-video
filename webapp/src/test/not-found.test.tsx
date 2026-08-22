import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import NotFound from '../pages/NotFound/NotFound'

function renderNotFound(route = '/nonexistent') {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <NotFound />
    </MemoryRouter>
  )
}

describe('NotFound 404 页面', () => {
  // ── 1. 404 页面渲染 ──────────────────────────────────────────────────────────

  it('渲染 404 数字视觉元素', () => {
    const { container } = renderNotFound()

    // 三个数字位 4、0、4
    expect(container.querySelector('.not-found__digit--4a')).toHaveTextContent('4')
    expect(container.querySelector('.not-found__digit--0')).toBeInTheDocument()
    expect(container.querySelector('.not-found__digit--4b')).toHaveTextContent('4')
  })

  it('渲染 glitch 覆盖层（辅助装饰）', () => {
    const { container } = renderNotFound()
    const glitch = container.querySelector('.not-found__glitch')
    expect(glitch).toBeInTheDocument()
    expect(glitch).toHaveAttribute('aria-hidden', 'true')
    // glitch 内三个 span 分别是 4、0、4
    const spans = glitch!.querySelectorAll('span')
    expect(spans).toHaveLength(3)
    expect(spans[0]).toHaveTextContent('4')
    expect(spans[1]).toHaveTextContent('0')
    expect(spans[2]).toHaveTextContent('4')
  })

  it('渲染背景装饰元素（光球与网格）', () => {
    const { container } = renderNotFound()

    expect(container.querySelector('.not-found__bg')).toBeInTheDocument()
    expect(container.querySelectorAll('.not-found__orb')).toHaveLength(3)
    expect(container.querySelector('.not-found__grid')).toBeInTheDocument()
  })

  it('渲染 12 个浮动粒子', () => {
    const { container } = renderNotFound()
    const particles = container.querySelectorAll('.not-found__particle')
    expect(particles).toHaveLength(12)
    // 验证编号从 1 到 12
    particles.forEach((p, i) => {
      expect(p).toHaveClass(`not-found__particle--${i + 1}`)
    })
  })

  it('显示「页面未找到」提示消息', () => {
    renderNotFound()
    expect(screen.getByText('页面未找到')).toBeInTheDocument()
  })

  it('404 视觉区域存在零环装饰', () => {
    const { container } = renderNotFound()
    const rings = container.querySelectorAll('.not-found__zero-ring')
    expect(rings).toHaveLength(2)
    expect(rings[1]).toHaveClass('not-found__zero-ring--inner')
  })

  // ── 2. 返回首页链接 ──────────────────────────────────────────────────────────

  it('渲染「返回首页」链接并指向根路径', () => {
    renderNotFound()
    const link = screen.getByRole('link', { name: /返回首页/ })
    expect(link).toBeInTheDocument()
    expect(link).toHaveAttribute('href', '/')
  })

  it('「返回首页」按钮包含房屋图标 SVG', () => {
    const { container } = renderNotFound()
    const btn = container.querySelector('.not-found__btn')
    expect(btn).toBeInTheDocument()
    const svg = btn!.querySelector('svg')
    expect(svg).toBeInTheDocument()
    expect(svg).toHaveAttribute('width', '18')
    expect(svg).toHaveAttribute('height', '18')
  })

  it('「返回首页」按钮包含箭头指示符', () => {
    const { container } = renderNotFound()
    const arrow = container.querySelector('.not-found__btn-arrow')
    expect(arrow).toHaveTextContent('→')
  })

  it('按钮使用 Link 组件（客户端导航，非整页刷新）', () => {
    const { container } = renderNotFound()
    const link = container.querySelector('a.not-found__btn')
    expect(link).toBeInTheDocument()
    // Link 渲染为 <a> 标签
    expect(link!.tagName).toBe('A')
  })

  // ── 3. 搜索建议（页面结构为搜索入口提供上下文） ─────────────────────────────

  it('404 页面内容层级清晰，便于搜索引擎理解', () => {
    const { container } = renderNotFound()
    // 主内容区域存在
    const content = container.querySelector('.not-found__content')
    expect(content).toBeInTheDocument()

    // 消息文本与链接在主内容区域内
    expect(content!.querySelector('.not-found__message')).toBeInTheDocument()
    expect(content!.querySelector('.not-found__btn')).toBeInTheDocument()
  })

  it('页面提供明确的导航路径（首页链接），减少用户迷失', () => {
    renderNotFound()
    // 用户到达 404 页面后，至少有一条明确的返回路径
    const homeLinks = screen.getAllByRole('link')
    expect(homeLinks.length).toBeGreaterThanOrEqual(1)
    // 所有链接都指向首页
    homeLinks.forEach((link) => {
      expect(link).toHaveAttribute('href', '/')
    })
  })

  it('提示消息使用 i18n 文本，确保本地化正确', () => {
    renderNotFound()
    // setup.ts 固定为 zh-CN，验证中文渲染
    const message = screen.getByText('页面未找到')
    expect(message).toHaveClass('not-found__message')
  })

  it('返回首页按钮文字使用 i18n 文本', () => {
    renderNotFound()
    // 按钮文字包含中文「返回首页」
    expect(screen.getByText('返回首页')).toBeInTheDocument()
  })

  it('整体容器具备正确的 CSS 类名和结构', () => {
    const { container } = renderNotFound()
    const root = container.firstChild as HTMLElement
    expect(root).toHaveClass('not-found')
    // 三层结构：背景层、粒子层、内容层
    expect(root.querySelector('.not-found__bg')).toBeInTheDocument()
    expect(root.querySelector('.not-found__particles')).toBeInTheDocument()
    expect(root.querySelector('.not-found__content')).toBeInTheDocument()
  })
})
