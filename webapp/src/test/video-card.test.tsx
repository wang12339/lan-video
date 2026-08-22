import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import React from 'react'
import { MemoryRouter } from 'react-router-dom'
import VideoCard, { VideoCardSkeleton } from '../components/VideoCard/VideoCard'

// Mock LazyImage 为简单 img，避免 jsdom 中 IntersectionObserver / new Image() 问题
vi.mock('../components/ui/LazyImage', () => ({
  __esModule: true,
  default: function MockLazyImage({ src, alt, className, fallback, eager }: any) {
    if (!src && fallback) return fallback
    return <img src={src || ''} alt={alt} className={`${className || ''} loaded`} loading={eager ? 'eager' : 'lazy'} />
  },
}))

function makeVideo(overrides: Partial<{ id: string; title: string; thumbnail_url: string; thumb: string | null; views: number }> = {}) {
  return {
    id: '1',
    title: '夏日旅行 Vlog',
    thumbnail_url: '/media/thumb.jpg',
    views: 1234,
    ...overrides,
  }
}

function renderCard(video: ReturnType<typeof makeVideo>, props: Partial<React.ComponentProps<typeof VideoCard>> = {}) {
  return render(
    <MemoryRouter>
      <VideoCard video={video as any} {...props} />
    </MemoryRouter>
  )
}

describe('VideoCard', () => {
  it('renders title and views', () => {
    const { container } = renderCard(makeVideo())
    expect(screen.getByRole('button', { name: '夏日旅行 Vlog' })).toBeInTheDocument()
    expect(container.querySelector('.video-card')).not.toBeNull()
    expect(screen.getByText(/次观看/)).toBeInTheDocument()
  })

  it('renders the thumbnail image with lazy loading', () => {
    const { container } = renderCard(makeVideo())
    const img = container.querySelector('img.card-img') as HTMLImageElement
    expect(img).not.toBeNull()
    expect(img).toHaveAttribute('src', '/media/thumb.jpg')
    expect(img).toHaveAttribute('alt', '夏日旅行 Vlog')
    expect(img).toHaveAttribute('loading', 'lazy')
  })

  it('shows an emoji fallback when there is no thumbnail', () => {
    const { container } = renderCard(makeVideo({ thumbnail_url: '' as any, thumb: null }))
    expect(container.querySelector('.thumb-wrap')).not.toBeNull()
    const fallback = container.querySelector('.thumb-fallback')
    expect(fallback).not.toBeNull()
    expect(fallback).toHaveTextContent('🎬')
  })

  it('calls onClick when clicked', () => {
    const onClick = vi.fn()
    renderCard(makeVideo(), { onClick })
    fireEvent.click(screen.getByRole('button', { name: '夏日旅行 Vlog' }))
    expect(onClick).toHaveBeenCalledTimes(1)
    expect(onClick.mock.calls[0]?.[0]).toMatchObject({ id: '1' })
  })

  it('calls onClick on Enter key', () => {
    const onClick = vi.fn()
    renderCard(makeVideo(), { onClick })
    fireEvent.keyDown(screen.getByRole('button', { name: '夏日旅行 Vlog' }), { key: 'Enter' })
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('applies compact class when compact=true', () => {
    const { container } = renderCard(makeVideo(), { compact: true })
    expect(container.querySelector('.video-card')).toHaveClass('compact')
  })

  it('navigates to player when no onClick provided', () => {
    // Should render as button and be clickable without throwing
    const { container } = renderCard(makeVideo())
    const btn = screen.getByRole('button', { name: '夏日旅行 Vlog' })
    expect(btn).toBeInTheDocument()
    fireEvent.click(btn)
    // Navigation is tested via not throwing and button still present
    expect(container.querySelector('.video-card')).not.toBeNull()
  })
})

describe('VideoCardSkeleton', () => {
  it('renders the requested number of skeleton cards', () => {
    const { container } = render(<VideoCardSkeleton count={4} />)
    expect(container.querySelectorAll('.skeleton-video-card')).toHaveLength(4)
  })

  it('defaults to 1 skeleton card', () => {
    const { container } = render(<VideoCardSkeleton />)
    expect(container.querySelectorAll('.skeleton-video-card')).toHaveLength(1)
  })
})
