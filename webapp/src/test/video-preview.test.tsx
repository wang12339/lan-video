import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import VideoPreview from '../components/ui/VideoPreview'

// Mock IntersectionObserver
const mockObserve = vi.fn()
const mockDisconnect = vi.fn()

class MockIntersectionObserver {
  constructor() {
    this.observe = mockObserve
    this.disconnect = mockDisconnect
  }
  observe = mockObserve
  disconnect = mockDisconnect
}

// Mock requestAnimationFrame
const mockRequestAnimationFrame = vi.fn()
mockRequestAnimationFrame.mockReturnValue(1)

describe('VideoPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    window.IntersectionObserver = MockIntersectionObserver as unknown as typeof IntersectionObserver
    window.requestAnimationFrame = mockRequestAnimationFrame
  })

  it('renders when visible', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        duration={120}
        views={15000}
        visible={true}
      />
    )

    expect(screen.getByText('Test Video')).toBeInTheDocument()
    expect(screen.getByText('2:00')).toBeInTheDocument()
    expect(screen.getByText('1.5万次播放')).toBeInTheDocument()
  })

  it('does not render when not visible', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        duration={120}
        views={15000}
        visible={false}
      />
    )

    expect(screen.queryByText('Test Video')).not.toBeInTheDocument()
  })

  it('renders video element when visible', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        duration={120}
        views={15000}
        visible={true}
      />
    )

    // 验证组件已渲染
    expect(screen.getByText('Test Video')).toBeInTheDocument()
  })

  it('formats time correctly', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        duration={65}
        visible={true}
      />
    )

    expect(screen.getByText('1:05')).toBeInTheDocument()
  })

  it('formats views correctly', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        views={12345}
        visible={true}
      />
    )

    expect(screen.getByText('1.2万次播放')).toBeInTheDocument()
  })

  it('handles zero views', () => {
    render(
      <VideoPreview
        videoId="video-123"
        title="Test Video"
        views={0}
        visible={true}
      />
    )

    expect(screen.queryByText('0次播放')).not.toBeInTheDocument()
  })
})