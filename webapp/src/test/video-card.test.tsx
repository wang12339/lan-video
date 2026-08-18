import { describe, it, expect } from 'vitest'
import { vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import VideoCard, { VideoCardSkeleton } from '../components/VideoCard/VideoCard'
import type { MappedVideo } from '../api/types'

function makeVideo(overrides: Partial<MappedVideo> = {}): MappedVideo {
  return {
    id: '1',
    title: '夏日旅行 Vlog',
    category: '记录',
    description: '',
    thumb: '/media/thumb.jpg',
    stream: '/media/v.mp4',
    cover: null,
    sourceType: 'local_video',
    duration: 65,
    views: 1234,
    date: '2026-01-01T00:00:00Z',
    progress: 0,
    ...overrides,
  }
}

function renderCard(video: MappedVideo, props: Partial<React.ComponentProps<typeof VideoCard>> = {}) {
  return render(
    <MemoryRouter>
      <VideoCard video={video} {...props} />
    </MemoryRouter>
  )
}

describe('VideoCard', () => {
  it('renders title, category badge, duration and views', () => {
    const { container } = renderCard(makeVideo())
    expect(screen.getByRole('button', { name: '夏日旅行 Vlog' })).toBeInTheDocument()
    expect(container.querySelector('.cat-badge')).toHaveTextContent('记录')
    expect(container.querySelector('.dur')).toHaveTextContent('01:05')
    expect(container.querySelector('.views')).toHaveTextContent('1.2k 次播放')
    expect(container.querySelector('.card')).toHaveAttribute('data-cat', '记录')
    expect(container.querySelector('.card')).toHaveAttribute('data-id', '1')
  })

  it('hides duration and views when absent', () => {
    const { container } = renderCard(makeVideo({ duration: 0, views: 0 }))
    expect(container.querySelector('.dur')).toBeNull()
    expect(container.querySelector('.views')).toBeNull()
  })

  it('renders a progress bar only when progress > 0', () => {
    const { container } = renderCard(makeVideo({ progress: 50 }))
    const fill = container.querySelector('.progress-fill') as HTMLElement
    expect(fill).not.toBeNull()
    expect(fill.style.width).toBe('50%')
  })

  it('clamps progress at 100', () => {
    const { container } = renderCard(makeVideo({ progress: 250 }))
    expect((container.querySelector('.progress-fill') as HTMLElement).style.width).toBe('100%')
  })

  it('renders no progress bar when progress is 0', () => {
    const { container } = renderCard(makeVideo({ progress: 0 }))
    expect(container.querySelector('.progress-bar')).toBeNull()
  })

  it('renders the thumbnail image with lazy loading', () => {
    const { container } = renderCard(makeVideo())
    const img = container.querySelector('img.card-img') as HTMLImageElement
    expect(img).not.toBeNull()
    expect(img).toHaveAttribute('src', '/media/thumb.jpg')
    expect(img).toHaveAttribute('alt', '夏日旅行 Vlog')
    expect(img).toHaveAttribute('loading', 'lazy')
  })

  it('shows an emoji fallback when the image fails to load', () => {
    const { container } = renderCard(makeVideo())
    const img = container.querySelector('img.card-img') as HTMLImageElement
    fireEvent.error(img)
    const fallback = container.querySelector('.thumb-fallback')
    expect(fallback).toHaveAttribute('role', 'img')
    expect(fallback).toHaveAttribute('aria-label', '夏日旅行 Vlog')
    expect(fallback).toHaveTextContent('📷')
  })

  it('shows a fallback when there is no thumbnail', () => {
    const { container } = renderCard(makeVideo({ thumb: null }))
    expect(container.querySelector('img.card-img')).toBeNull()
    expect(container.querySelector('.thumb-fallback')).toHaveTextContent('📷')
  })

  it('sets --cat-color for known categories only', () => {
    const known = renderCard(makeVideo())
    const card = known.container.querySelector('.card') as HTMLElement
    expect(card.style.getPropertyValue('--cat-color')).not.toBe('')
    const unknown = renderCard(makeVideo({ category: '杂项' }))
    const card2 = unknown.container.querySelector('.card') as HTMLElement
    expect(card2.style.getPropertyValue('--cat-color')).toBe('')
  })

  it('shows the play overlay in grid mode and hides it in list mode', () => {
    const grid = renderCard(makeVideo())
    expect(grid.container.querySelector('.play-over')).not.toBeNull()
    const list = renderCard(makeVideo(), { isList: true })
    expect(list.container.querySelector('.play-over')).toBeNull()
  })

  it('calls onSelect when clicked', () => {
    const onSelect = vi.fn()
    renderCard(makeVideo(), { onSelect })
    fireEvent.click(screen.getByRole('button', { name: '夏日旅行 Vlog' }))
    expect(onSelect).toHaveBeenCalledTimes(1)
    expect(onSelect.mock.calls[0]?.[0]).toBe('1')
  })

  it('calls onSelect on Enter key', () => {
    const onSelect = vi.fn()
    renderCard(makeVideo(), { onSelect })
    fireEvent.keyDown(screen.getByRole('button', { name: '夏日旅行 Vlog' }), { key: 'Enter' })
    expect(onSelect).toHaveBeenCalledTimes(1)
  })

  it('applies the selected class when selected', () => {
    const { container } = renderCard(makeVideo(), { selected: true })
    expect(container.querySelector('.card')).toHaveClass('selected')
  })
})

describe('VideoCardSkeleton', () => {
  it('renders the requested number of skeleton cards', () => {
    const { container } = render(<VideoCardSkeleton count={4} />)
    expect(container.querySelectorAll('.card-skeleton')).toHaveLength(4)
  })

  it('defaults to 6 skeleton cards', () => {
    const { container } = render(<VideoCardSkeleton />)
    expect(container.querySelectorAll('.card-skeleton')).toHaveLength(6)
  })
})
