import { useMemo } from 'react'
import './SkeletonLoader.css'

type SkeletonShape = 'rect' | 'circle' | 'rounded'
type SkeletonAnimation = 'shimmer' | 'pulse' | 'wave' | 'none'

interface SkeletonProps {
  lines?: number
  type?: 'text' | 'card' | 'table' | 'stats' | 'video-card' | 'custom'
  shape?: SkeletonShape
  animation?: SkeletonAnimation
  width?: string | number
  height?: string | number
  borderRadius?: string | number
  className?: string
  style?: React.CSSProperties
}

const TABLE_COLUMNS = 7

export default function SkeletonLoader({
  lines = 3,
  type = 'text',
  shape = 'rect',
  animation = 'shimmer',
  width,
  height,
  borderRadius,
  className = '',
  style
}: SkeletonProps) {
  const widths = useMemo(() =>
    Array.from({ length: lines }, (_, i) => `${65 + (i % 3) * 15}%`),
    [lines]
  )

  const getShapeClass = (shape: SkeletonShape): string => {
    switch (shape) {
      case 'circle': return 'skeleton-shape-circle'
      case 'rounded': return 'skeleton-shape-rounded'
      default: return 'skeleton-shape-rect'
    }
  }

  const getAnimationClass = (animation: SkeletonAnimation): string => {
    switch (animation) {
      case 'pulse': return 'skeleton-animate-pulse'
      case 'wave': return 'skeleton-animate-wave'
      case 'none': return ''
      default: return 'skeleton-animate-shimmer'
    }
  }

  const shapeClass = getShapeClass(shape)
  const animClass = getAnimationClass(animation)

  const lineStyle = useMemo(() => ({
    ...(width !== undefined && { width: typeof width === 'number' ? `${width}px` : width }),
    ...(height !== undefined && { height: typeof height === 'number' ? `${height}px` : height }),
    ...(borderRadius !== undefined && { borderRadius: typeof borderRadius === 'number' ? `${borderRadius}px` : borderRadius }),
  }), [width, height, borderRadius])

  if (type === 'stats') {
    const statCount = Math.max(6, lines)
    return (
      <div className={`skeleton-stats ${className}`} style={style} aria-hidden="true">
        {Array.from({ length: statCount }).map((_, i) => (
          <div key={i} className="skeleton-stat-card">
            <div className={`skeleton-line skeleton-value ${shapeClass} ${animClass}`} style={lineStyle} />
            <div className={`skeleton-line skeleton-label ${shapeClass} ${animClass}`} style={lineStyle} />
          </div>
        ))}
      </div>
    )
  }

  if (type === 'table') {
    return (
      <div className={`skeleton-table ${className}`} style={style} aria-hidden="true">
        <div className="skeleton-table-header">
          {Array.from({ length: TABLE_COLUMNS }).map((_, i) => (
            <div key={i} className={`skeleton-line skeleton-th ${shapeClass} ${animClass}`} style={lineStyle} />
          ))}
        </div>
        {Array.from({ length: lines }).map((_, i) => (
          <div key={i} className="skeleton-table-row">
            {Array.from({ length: TABLE_COLUMNS }).map((_, j) => (
              <div key={j} className={`skeleton-line skeleton-td ${shapeClass} ${animClass}`} style={lineStyle} />
            ))}
          </div>
        ))}
      </div>
    )
  }

  if (type === 'card') {
    return (
      <div className={`skeleton-card ${className}`} style={style} aria-hidden="true">
        {Array.from({ length: lines }).map((_, i) => (
          <div key={i} className="skeleton-card-item">
            <div className={`skeleton-line skeleton-card-avatar ${shapeClass} ${animClass}`} style={lineStyle} />
            <div className="skeleton-card-content">
              <div className={`skeleton-line skeleton-card-title ${shapeClass} ${animClass}`} style={lineStyle} />
              <div className={`skeleton-line skeleton-card-desc ${shapeClass} ${animClass}`} style={lineStyle} />
            </div>
          </div>
        ))}
      </div>
    )
  }

  if (type === 'video-card') {
    return (
      <>
        {Array.from({ length: lines }).map((_, i) => (
          <div key={i} className={`skeleton-video-card ${className}`} style={style} aria-hidden="true">
            <div className={`skeleton-line skeleton-video-thumb ${shapeClass} ${animClass}`} />
            <div className="skeleton-video-info">
              <div className={`skeleton-line skeleton-video-title ${shapeClass} ${animClass}`} />
              <div className={`skeleton-line skeleton-video-meta ${shapeClass} ${animClass}`} />
            </div>
          </div>
        ))}
      </>
    )
  }


  if (type === 'custom') {
    return (
      <div className={`skeleton-custom ${className}`} style={style} aria-hidden="true">
        <div className={`skeleton-line ${shapeClass} ${animClass}`} style={lineStyle} />
      </div>
    )
  }

  return (
    <div className={`skeleton-text ${className}`} style={style} aria-hidden="true">
      {Array.from({ length: lines }).map((_, i) => (
        <div
          key={i}
          className={`skeleton-line ${shapeClass} ${animClass}`}
          style={{ width: widths[i], ...lineStyle }}
        />
      ))}
    </div>
  )
}