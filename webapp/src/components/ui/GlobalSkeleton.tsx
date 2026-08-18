import './GlobalSkeleton.css'

interface SkeletonProps {
  width?: string | number
  height?: string | number
  borderRadius?: string | number
  className?: string
  count?: number
  inline?: boolean
  style?: React.CSSProperties
}

export function Skeleton({
  width,
  height = '1em',
  borderRadius = '4px',
  className = '',
  count = 1,
  inline = false
}: SkeletonProps) {
  const style: React.CSSProperties = {
    width: width ?? '100%',
    height,
    borderRadius
  }

  if (count === 1) {
    return (
      <span
        className={`skeleton ${className}`}
        style={style}
        aria-hidden="true"
      />
    )
  }

  return (
    <span className={`skeleton-group ${inline ? 'inline' : ''}`} aria-hidden="true">
      {Array.from({ length: count }).map((_, i) => (
        <span
          key={i}
          className={`skeleton ${className}`}
          style={{
            ...style,
            animationDelay: `${i * 0.15}s`
          }}
        />
      ))}
    </span>
  )
}

// 预设骨架屏组件
export function TextSkeleton({ lines = 3, widths }: { lines?: number; widths?: string[] }) {
  return (
    <div className="skeleton-text" aria-hidden="true">
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton
          key={i}
          height="12px"
          width={widths?.[i] ?? (i === lines - 1 ? '60%' : '100%')}
        />
      ))}
    </div>
  )
}

export function CardSkeleton() {
  return (
    <div className="skeleton-card" aria-hidden="true">
      <Skeleton height="0" style={{ paddingBottom: '56.25%' }} borderRadius="var(--radius) var(--radius) 0 0" />
      <div className="skeleton-card-info">
        <Skeleton height="10px" width="30%" />
        <Skeleton height="14px" width="90%" />
        <Skeleton height="12px" width="60%" />
      </div>
    </div>
  )
}

export function AvatarSkeleton({ size = 40 }: { size?: number }) {
  return (
    <Skeleton
      width={size}
      height={size}
      borderRadius="50%"
      className="skeleton-avatar"
    />
  )
}
