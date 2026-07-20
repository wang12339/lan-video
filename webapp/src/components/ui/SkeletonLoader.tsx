import { useMemo } from 'react'
import './SkeletonLoader.css'

interface SkeletonProps {
  lines?: number
  type?: 'text' | 'card' | 'table' | 'stats'
}

export default function SkeletonLoader({ lines = 3, type = 'text' }: SkeletonProps) {
  const widths = useMemo(() =>
    Array.from({ length: lines }, (_, i) => `${65 + (i % 3) * 15}%`),
    [lines]
  )

  if (type === 'stats') {
    return (
      <div className="skeleton-stats">
        {Array.from({ length: 6 }).map((_, i) => (
          <div key={i} className="skeleton-stat-card">
            <div className="skeleton-line skeleton-value" />
            <div className="skeleton-line skeleton-label" />
          </div>
        ))}
      </div>
    )
  }

  if (type === 'table') {
    return (
      <div className="skeleton-table">
        <div className="skeleton-table-header">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="skeleton-line skeleton-th" />
          ))}
        </div>
        {Array.from({ length: lines }).map((_, i) => (
          <div key={i} className="skeleton-table-row">
            {Array.from({ length: 5 }).map((_, j) => (
              <div key={j} className="skeleton-line skeleton-td" />
            ))}
          </div>
        ))}
      </div>
    )
  }

  if (type === 'card') {
    return (
      <div className="skeleton-card">
        {Array.from({ length: lines }).map((_, i) => (
          <div key={i} className="skeleton-card-item">
            <div className="skeleton-line skeleton-card-avatar" />
            <div className="skeleton-card-content">
              <div className="skeleton-line skeleton-card-title" />
              <div className="skeleton-line skeleton-card-desc" />
            </div>
          </div>
        ))}
      </div>
    )
  }

  return (
    <div className="skeleton-text">
      {Array.from({ length: lines }).map((_, i) => (
        <div
          key={i}
          className="skeleton-line"
          style={{ width: widths[i] }}
        />
      ))}
    </div>
  )
}
