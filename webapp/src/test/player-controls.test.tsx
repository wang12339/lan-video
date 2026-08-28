import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import React from 'react'
import PlayerControls from '../pages/Player/PlayerControls'
import ProgressBar from '../pages/Player/components/ProgressBar'
import VolumeControl from '../pages/Player/components/VolumeControl'
import SpeedMenu from '../pages/Player/components/SpeedMenu'
import QualityMenu from '../pages/Player/components/QualityMenu'
import { SPEED_STEPS } from '../pages/Player/constants'
import type { VideoVariant } from '../api/types'
import type { TFunction } from 'i18next'

// ── Mocks ──────────────────────────────────────────────────────────────────────

vi.mock('../utils/track', () => ({
  trackClick: vi.fn(),
  trackVideo: vi.fn(),
}))

vi.mock('../pages/Player/hooks/usePlayerGestures', () => ({
  usePlayerGestures: () => ({
    gestureIndicator: null,
    gestureValue: 0,
    isLongPressing: false,
    gestureAreaRef: { current: null },
    handleTouchStart: vi.fn(),
    handleTouchMove: vi.fn(),
    handleTouchEnd: vi.fn(),
    handleMouseDown: vi.fn(),
    handleMouseUp: vi.fn(),
  }),
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

const t: TFunction = ((key: string, opts?: Record<string, unknown>) => {
  const map: Record<string, string> = {
    'player.playPause': '播放/暂停',
    'player.seekBackward': '快退',
    'player.seekForward': '快进',
    'player.progressAria': '播放进度',
    'player.timeDisplay': `${opts?.current ?? ''} / ${opts?.total ?? ''}`,
    'player.mute': '静音',
    'player.unmute': '取消静音',
    'player.volume': '音量',
    'player.speed': '倍速',
    'player.quality': '画质',
    'player.original': '原始',
    'player.fullscreen': '全屏',
    'player.pictureInPicture': '画中画',
    'player.videoControls': '视频控件',
  }
  return map[key] ?? key
}) as TFunction

function makeVideoRef(currentTime = 0, duration = 120, volume = 0.8, muted = false) {
  const el = document.createElement('video')
  Object.defineProperty(el, 'currentTime', { value: currentTime, writable: true })
  Object.defineProperty(el, 'duration', { value: duration, writable: true })
  Object.defineProperty(el, 'volume', { value: volume, writable: true })
  Object.defineProperty(el, 'muted', { value: muted, writable: true })
  Object.defineProperty(el, 'buffered', {
    value: {
      length: 0,
      end: vi.fn(() => 0),
    },
  })
  return { current: el } as React.RefObject<HTMLVideoElement | null>
}

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('PlayerControls', () => {
  const defaultProps: React.ComponentProps<typeof PlayerControls> = {
    videoRef: makeVideoRef(),
    controlsVisible: true,
    paused: true,
    duration: 120,
    speed: 1,
    showQualityMenu: false,
    showSpeedMenu: false,
    currentQuality: 'original',
    variants: [],
    togglePlay: vi.fn(),
    toggleMute: vi.fn(),
    toggleFullscreen: vi.fn(),
    togglePiP: vi.fn(),
    setSpeedValue: vi.fn(),
    setVolumeValue: vi.fn(),
    switchQuality: vi.fn(),
    seekBy: vi.fn(),
    resetHideTimer: vi.fn(),
    setShowQualityMenu: vi.fn(),
    setShowSpeedMenu: vi.fn(),
    t,
  }

  it('渲染所有控制按钮', () => {
    render(<PlayerControls {...defaultProps} />)

    expect(screen.getByRole('button', { name: '播放/暂停' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '快退' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '快进' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '全屏' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '画中画' })).toBeInTheDocument()
  })

  it('渲染进度条', () => {
    render(<PlayerControls {...defaultProps} />)

    expect(screen.getByRole('slider', { name: '播放进度' })).toBeInTheDocument()
  })

  it('渲染时间显示', () => {
    render(<PlayerControls {...defaultProps} />)

    expect(document.querySelector('.time-display')).toBeInTheDocument()
    expect(document.querySelector('.time-sep')).toHaveTextContent('/')
  })

  it('controlsVisible 控制显示类名', () => {
    const { rerender } = render(<PlayerControls {...defaultProps} controlsVisible={true} />)
    expect(document.querySelector('.player-controls')).toHaveClass('show')

    rerender(<PlayerControls {...defaultProps} controlsVisible={false} />)
    expect(document.querySelector('.player-controls')).not.toHaveClass('show')
  })

  it('点击播放/暂停按钮调用 togglePlay', () => {
    const togglePlay = vi.fn()
    render(<PlayerControls {...defaultProps} togglePlay={togglePlay} />)

    fireEvent.click(screen.getByRole('button', { name: '播放/暂停' }))
    expect(togglePlay).toHaveBeenCalledTimes(1)
  })

  it('点击快退按钮调用 seekBy(-10)', () => {
    const seekBy = vi.fn()
    render(<PlayerControls {...defaultProps} seekBy={seekBy} />)

    fireEvent.click(screen.getByRole('button', { name: '快退' }))
    expect(seekBy).toHaveBeenCalledWith(-10)
  })

  it('点击快进按钮调用 seekBy(10)', () => {
    const seekBy = vi.fn()
    render(<PlayerControls {...defaultProps} seekBy={seekBy} />)

    fireEvent.click(screen.getByRole('button', { name: '快进' }))
    expect(seekBy).toHaveBeenCalledWith(10)
  })

  it('点击全屏按钮调用 toggleFullscreen', () => {
    const toggleFullscreen = vi.fn()
    render(<PlayerControls {...defaultProps} toggleFullscreen={toggleFullscreen} />)

    fireEvent.click(screen.getByRole('button', { name: '全屏' }))
    expect(toggleFullscreen).toHaveBeenCalledTimes(1)
  })

  it('点击画中画按钮调用 togglePiP', () => {
    const togglePiP = vi.fn()
    render(<PlayerControls {...defaultProps} togglePiP={togglePiP} />)

    fireEvent.click(screen.getByRole('button', { name: '画中画' }))
    expect(togglePiP).toHaveBeenCalledTimes(1)
  })

  it('toolbar 存在且有正确的 aria-label', () => {
    render(<PlayerControls {...defaultProps} />)

    const toolbar = screen.getByRole('toolbar', { name: '视频控件' })
    expect(toolbar).toBeInTheDocument()
  })
})

describe('ProgressBar', () => {
  const defaultProps: React.ComponentProps<typeof ProgressBar> = {
    videoRef: makeVideoRef(30, 120),
    currentTime: 30,
    buffered: 50,
    duration: 120,
    setCurrentTime: vi.fn(),
    resetHideTimer: vi.fn(),
    t,
  }

  it('渲染进度条容器', () => {
    render(<ProgressBar {...defaultProps} />)

    expect(document.querySelector('.player-progress-wrap')).toBeInTheDocument()
    expect(document.querySelector('.player-progress-bar')).toBeInTheDocument()
  })

  it('显示正确的进度百分比', () => {
    render(<ProgressBar {...defaultProps} currentTime={60} duration={120} />)

    const current = document.querySelector('.player-progress-current')!
    expect(current).toHaveStyle({ width: '50%' })

    const dot = document.querySelector('.player-progress-dot')!
    expect(dot).toHaveStyle({ left: '50%' })
  })

  it('显示缓冲进度', () => {
    render(<ProgressBar {...defaultProps} buffered={75} />)

    const buffered = document.querySelector('.player-progress-buffered')!
    expect(buffered).toHaveStyle({ width: '75%' })
  })

  it('进度条是可访问的 slider', () => {
    render(<ProgressBar {...defaultProps} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    expect(slider).toHaveAttribute('tabindex', '0')
    expect(slider).toHaveAttribute('aria-valuemin', '0')
    expect(slider).toHaveAttribute('aria-valuemax', '100')
    expect(slider).toHaveAttribute('aria-valuenow', '25') // 30/120 = 25%
  })

  it('点击进度条更新 currentTime', () => {
    const setCurrentTime = vi.fn()
    const videoRef = makeVideoRef(0, 120)
    render(<ProgressBar {...defaultProps} videoRef={videoRef} setCurrentTime={setCurrentTime} />)

    const bar = document.querySelector('.player-progress-wrap') as HTMLElement
    bar.getBoundingClientRect = vi.fn(() => ({ left: 0, width: 100, top: 0, height: 4, right: 100, bottom: 4, x: 0, y: 0, toJSON: () => {} } as DOMRect))

    fireEvent.mouseDown(bar, { clientX: 50 })
    expect(setCurrentTime).toHaveBeenCalledWith(60) // 50/100 * 120
  })

  it('键盘 ArrowRight 前进 5 秒', () => {
    const setCurrentTime = vi.fn()
    const videoRef = makeVideoRef(30, 120)
    render(<ProgressBar {...defaultProps} videoRef={videoRef} setCurrentTime={setCurrentTime} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    fireEvent.keyDown(slider, { key: 'ArrowRight' })
    expect(setCurrentTime).toHaveBeenCalledWith(35)
  })

  it('键盘 ArrowLeft 后退 5 秒', () => {
    const setCurrentTime = vi.fn()
    const videoRef = makeVideoRef(30, 120)
    render(<ProgressBar {...defaultProps} videoRef={videoRef} setCurrentTime={setCurrentTime} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    fireEvent.keyDown(slider, { key: 'ArrowLeft' })
    expect(setCurrentTime).toHaveBeenCalledWith(25)
  })

  it('键盘 Home 跳到开头', () => {
    const setCurrentTime = vi.fn()
    const videoRef = makeVideoRef(30, 120)
    render(<ProgressBar {...defaultProps} videoRef={videoRef} setCurrentTime={setCurrentTime} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    fireEvent.keyDown(slider, { key: 'Home' })
    expect(setCurrentTime).toHaveBeenCalledWith(0)
  })

  it('键盘 End 跳到结尾', () => {
    const setCurrentTime = vi.fn()
    const videoRef = makeVideoRef(30, 120)
    render(<ProgressBar {...defaultProps} videoRef={videoRef} setCurrentTime={setCurrentTime} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    fireEvent.keyDown(slider, { key: 'End' })
    expect(setCurrentTime).toHaveBeenCalledWith(120)
  })

  it('duration 为 0 时进度为 0', () => {
    render(<ProgressBar {...defaultProps} duration={0} currentTime={0} />)

    const slider = screen.getByRole('slider', { name: '播放进度' })
    expect(slider).toHaveAttribute('aria-valuenow', '0')
  })
})

describe('VolumeControl', () => {
  const defaultProps: React.ComponentProps<typeof VolumeControl> = {
    volume: 0.8,
    muted: false,
    toggleMute: vi.fn(),
    setVolumeValue: vi.fn(),
    setVolume: vi.fn(),
    t,
  }

  it('渲染音量容器', () => {
    render(<VolumeControl {...defaultProps} />)

    expect(document.querySelector('.volume-wrap')).toBeInTheDocument()
  })

  it('渲染音量滑块', () => {
    render(<VolumeControl {...defaultProps} />)

    const slider = screen.getByRole('slider', { name: '音量' })
    expect(slider).toHaveAttribute('type', 'range')
    expect(slider).toHaveAttribute('min', '0')
    expect(slider).toHaveAttribute('max', '1')
    expect(slider).toHaveAttribute('step', '0.05')
  })

  it('显示正确的音量值', () => {
    render(<VolumeControl {...defaultProps} volume={0.6} />)

    const slider = screen.getByRole('slider', { name: '音量' })
    expect(slider).toHaveValue('0.6')
  })

  it('静音时显示 0', () => {
    render(<VolumeControl {...defaultProps} volume={0.8} muted={true} />)

    const slider = screen.getByRole('slider', { name: '音量' })
    expect(slider).toHaveValue('0')
  })

  it('未静音时按钮 aria-label 为静音', () => {
    render(<VolumeControl {...defaultProps} muted={false} />)

    expect(screen.getByRole('button', { name: '静音' })).toBeInTheDocument()
  })

  it('已静音时按钮 aria-label 为取消静音', () => {
    render(<VolumeControl {...defaultProps} muted={true} />)

    expect(screen.getByRole('button', { name: '取消静音' })).toBeInTheDocument()
  })

  it('点击静音按钮调用 toggleMute', () => {
    const toggleMute = vi.fn()
    render(<VolumeControl {...defaultProps} toggleMute={toggleMute} />)

    fireEvent.click(screen.getByRole('button', { name: '静音' }))
    expect(toggleMute).toHaveBeenCalledTimes(1)
  })

  it('改变滑块调用 setVolume 和 setVolumeValue', () => {
    const setVolume = vi.fn()
    const setVolumeValue = vi.fn()
    render(<VolumeControl {...defaultProps} setVolume={setVolume} setVolumeValue={setVolumeValue} />)

    fireEvent.change(screen.getByRole('slider', { name: '音量' }), { target: { value: '0.3' } })
    expect(setVolume).toHaveBeenCalledWith(0.3)
    expect(setVolumeValue).toHaveBeenCalledWith(0.3)
  })

  it('aria-valuenow 反映当前音量百分比', () => {
    render(<VolumeControl {...defaultProps} volume={0.5} muted={false} />)

    const slider = screen.getByRole('slider', { name: '音量' })
    expect(slider).toHaveAttribute('aria-valuenow', '50')
  })

  it('静音时 aria-valuenow 为 0', () => {
    render(<VolumeControl {...defaultProps} volume={0.8} muted={true} />)

    const slider = screen.getByRole('slider', { name: '音量' })
    expect(slider).toHaveAttribute('aria-valuenow', '0')
  })
})

describe('SpeedMenu', () => {
  const defaultProps: React.ComponentProps<typeof SpeedMenu> = {
    speed: 1,
    showSpeedMenu: false,
    setShowSpeedMenu: vi.fn(),
    setSpeedValue: vi.fn(),
    t,
  }

  it('渲染倍速按钮', () => {
    render(<SpeedMenu {...defaultProps} />)

    const btn = screen.getByRole('button', { name: '倍速' })
    expect(btn).toBeInTheDocument()
    expect(btn).toHaveTextContent('1×')
  })

  it('初始状态不显示菜单', () => {
    render(<SpeedMenu {...defaultProps} />)

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('showSpeedMenu=true 时显示菜单', () => {
    render(<SpeedMenu {...defaultProps} showSpeedMenu={true} />)

    expect(screen.getByRole('menu')).toBeInTheDocument()
  })

  it('菜单包含所有倍速选项', () => {
    render(<SpeedMenu {...defaultProps} showSpeedMenu={true} />)

    const menu = screen.getByRole('menu')
    const options = menu.querySelectorAll('.speed-opt')
    expect(options).toHaveLength(SPEED_STEPS.length)
    SPEED_STEPS.forEach((s) => {
      expect(menu).toHaveTextContent(`${s}×`)
    })
  })

  it('当前倍速高亮', () => {
    render(<SpeedMenu {...defaultProps} speed={1.5} showSpeedMenu={true} />)

    const active = document.querySelector('.speed-opt.active')
    expect(active).toHaveTextContent('1.5×')
    expect(active).toHaveAttribute('aria-current', 'true')
  })

  it('点击按钮切换菜单显示', () => {
    const setShowSpeedMenu = vi.fn()
    render(<SpeedMenu {...defaultProps} showSpeedMenu={false} setShowSpeedMenu={setShowSpeedMenu} />)

    fireEvent.click(screen.getByRole('button', { name: '倍速' }))
    expect(setShowSpeedMenu).toHaveBeenCalledWith(true)
  })

  it('点击已打开的按钮关闭菜单', () => {
    const setShowSpeedMenu = vi.fn()
    render(<SpeedMenu {...defaultProps} showSpeedMenu={true} setShowSpeedMenu={setShowSpeedMenu} />)

    fireEvent.click(screen.getByRole('button', { name: '倍速' }))
    expect(setShowSpeedMenu).toHaveBeenCalledWith(false)
  })

  it('点击倍速选项设置速度并关闭菜单', () => {
    const setSpeedValue = vi.fn()
    const setShowSpeedMenu = vi.fn()
    render(<SpeedMenu {...defaultProps} showSpeedMenu={true} setSpeedValue={setSpeedValue} setShowSpeedMenu={setShowSpeedMenu} />)

    fireEvent.click(screen.getByText('2×'))
    expect(setSpeedValue).toHaveBeenCalledWith(2)
    expect(setShowSpeedMenu).toHaveBeenCalledWith(false)
  })

  it('按钮有 aria-haspopup 和 aria-expanded', () => {
    const { rerender } = render(<SpeedMenu {...defaultProps} showSpeedMenu={false} />)
    const btn = screen.getByRole('button', { name: '倍速' })
    expect(btn).toHaveAttribute('aria-haspopup', 'menu')
    expect(btn).toHaveAttribute('aria-expanded', 'false')

    rerender(<SpeedMenu {...defaultProps} showSpeedMenu={true} />)
    expect(btn).toHaveAttribute('aria-expanded', 'true')
  })

  it('显示不同倍速值', () => {
    const { rerender } = render(<SpeedMenu {...defaultProps} speed={0.5} />)
    expect(screen.getByRole('button', { name: '倍速' })).toHaveTextContent('0.5×')

    rerender(<SpeedMenu {...defaultProps} speed={2} />)
    expect(screen.getByRole('button', { name: '倍速' })).toHaveTextContent('2×')
  })
})

describe('QualityMenu', () => {
  const variants: VideoVariant[] = [
    { resolution: '720p', url: '/media/v_720p.m3u8' },
    { resolution: '1080p', url: '/media/v_1080p.m3u8' },
  ]

  const defaultProps: React.ComponentProps<typeof QualityMenu> = {
    currentQuality: 'original',
    variants,
    showQualityMenu: false,
    setShowQualityMenu: vi.fn(),
    switchQuality: vi.fn(),
    t,
  }

  it('有 variants 时渲染画质按钮', () => {
    render(<QualityMenu {...defaultProps} />)

    expect(screen.getByRole('button', { name: '画质' })).toBeInTheDocument()
  })

  it('无 variants 时不渲染', () => {
    render(<QualityMenu {...defaultProps} variants={[]} />)

    expect(screen.queryByRole('button', { name: '画质' })).not.toBeInTheDocument()
  })

  it('currentQuality=original 显示原始', () => {
    render(<QualityMenu {...defaultProps} currentQuality="original" />)

    expect(screen.getByRole('button', { name: '画质' })).toHaveTextContent('原始')
  })

  it('currentQuality 非 original 显示分辨率名称', () => {
    render(<QualityMenu {...defaultProps} currentQuality="720p" />)

    expect(screen.getByRole('button', { name: '画质' })).toHaveTextContent('720p')
  })

  it('初始状态不显示菜单', () => {
    render(<QualityMenu {...defaultProps} />)

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('showQualityMenu=true 时显示菜单', () => {
    render(<QualityMenu {...defaultProps} showQualityMenu={true} />)

    expect(screen.getByRole('menu')).toBeInTheDocument()
  })

  it('菜单包含原始选项和所有 variants', () => {
    render(<QualityMenu {...defaultProps} showQualityMenu={true} />)

    const menu = screen.getByRole('menu')
    const options = menu.querySelectorAll('.quality-opt')
    expect(options).toHaveLength(3) // 原始 + 720p + 1080p
    expect(menu).toHaveTextContent('原始')
    expect(menu).toHaveTextContent('720p')
    expect(menu).toHaveTextContent('1080p')
  })

  it('当前画质高亮', () => {
    render(<QualityMenu {...defaultProps} currentQuality="720p" showQualityMenu={true} />)

    const active = document.querySelector('.quality-opt.active')
    expect(active).toHaveTextContent('720p')
    expect(active).toHaveAttribute('aria-current', 'true')
  })

  it('original 高亮', () => {
    render(<QualityMenu {...defaultProps} currentQuality="original" showQualityMenu={true} />)

    const active = document.querySelector('.quality-opt.active')
    expect(active).toHaveTextContent('原始')
  })

  it('点击按钮切换菜单显示', () => {
    const setShowQualityMenu = vi.fn()
    render(<QualityMenu {...defaultProps} showQualityMenu={false} setShowQualityMenu={setShowQualityMenu} />)

    fireEvent.click(screen.getByRole('button', { name: '画质' }))
    expect(setShowQualityMenu).toHaveBeenCalledWith(true)
  })

  it('点击已打开的按钮关闭菜单', () => {
    const setShowQualityMenu = vi.fn()
    render(<QualityMenu {...defaultProps} showQualityMenu={true} setShowQualityMenu={setShowQualityMenu} />)

    fireEvent.click(screen.getByRole('button', { name: '画质' }))
    expect(setShowQualityMenu).toHaveBeenCalledWith(false)
  })

  it('点击 variant 选项切换画质', () => {
    const switchQuality = vi.fn()
    render(<QualityMenu {...defaultProps} showQualityMenu={true} switchQuality={switchQuality} />)

    fireEvent.click(screen.getByText('1080p'))
    expect(switchQuality).toHaveBeenCalledWith('1080p')
  })

  it('点击原始选项切换到 original', () => {
    const switchQuality = vi.fn()
    render(<QualityMenu {...defaultProps} showQualityMenu={true} switchQuality={switchQuality} />)

    const menu = screen.getByRole('menu')
    const originalBtn = menu.querySelector('.quality-opt.active')!
    fireEvent.click(originalBtn)
    expect(switchQuality).toHaveBeenCalledWith('original')
  })

  it('按钮有 aria-haspopup 和 aria-expanded', () => {
    const { rerender } = render(<QualityMenu {...defaultProps} showQualityMenu={false} />)
    const btn = screen.getByRole('button', { name: '画质' })
    expect(btn).toHaveAttribute('aria-haspopup', 'menu')
    expect(btn).toHaveAttribute('aria-expanded', 'false')

    rerender(<QualityMenu {...defaultProps} showQualityMenu={true} />)
    expect(btn).toHaveAttribute('aria-expanded', 'true')
  })

  it('单个 variant 也正确渲染', () => {
    const singleVariant: VideoVariant[] = [{ resolution: '480p', url: '/media/v_480p.m3u8' }]
    render(<QualityMenu {...defaultProps} variants={singleVariant} showQualityMenu={true} />)

    const menu = screen.getByRole('menu')
    const options = menu.querySelectorAll('.quality-opt')
    expect(options).toHaveLength(2) // 原始 + 480p
  })
})
