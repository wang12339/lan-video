import { describe, it, expect } from 'vitest'
import { SkeletonLoader } from '../components/ui'
import { render } from '@testing-library/react'

describe('SkeletonLoader', () => {
  it('renders text skeleton with N lines and progressive widths', () => {
    const { container } = render(<SkeletonLoader lines={3} type="text" />)
    const root = container.querySelector('.skeleton-text')
    expect(root).toHaveAttribute('aria-hidden', 'true')
    const lines = container.querySelectorAll('.skeleton-text .skeleton-line')
    expect(lines).toHaveLength(3)
    const widths = Array.from(lines).map(l => (l as HTMLElement).style.width)
    expect(widths).toEqual(['65%', '80%', '95%'])
  })

  it('defaults to 3 text lines', () => {
    const { container } = render(<SkeletonLoader />)
    expect(container.querySelectorAll('.skeleton-text .skeleton-line')).toHaveLength(3)
  })

  it('renders card skeleton with the requested item count', () => {
    const { container } = render(<SkeletonLoader type="card" lines={2} />)
    expect(container.querySelectorAll('.skeleton-card-item')).toHaveLength(2)
    expect(container.querySelectorAll('.skeleton-card-title')).toHaveLength(2)
  })

  it('renders table skeleton with header row and N body rows', () => {
    const { container } = render(<SkeletonLoader type="table" lines={4} />)
    expect(container.querySelectorAll('.skeleton-table-header .skeleton-th')).toHaveLength(7)
    expect(container.querySelectorAll('.skeleton-table-row')).toHaveLength(4)
    expect(container.querySelectorAll('.skeleton-table-row .skeleton-td')).toHaveLength(28)
  })

  it('renders at least 6 stat cards', () => {
    const { container } = render(<SkeletonLoader type="stats" lines={3} />)
    expect(container.querySelectorAll('.skeleton-stat-card')).toHaveLength(6)
  })

  it('uses lines directly for stats when larger than 6', () => {
    const { container } = render(<SkeletonLoader type="stats" lines={8} />)
    expect(container.querySelectorAll('.skeleton-stat-card')).toHaveLength(8)
  })
})
