import { useState, useCallback, memo } from 'react'
import { useLazyImage } from '../../hooks/useLazyImage'

interface LazyImageProps {
  src: string | null | undefined
  alt: string
  className?: string
  placeholder?: string
  fallback?: React.ReactNode
  onLoad?: () => void
  onError?: () => void
  style?: React.CSSProperties
}

function LazyImageImpl({
  src,
  alt,
  className = '',
  placeholder,
  fallback,
  onLoad,
  onError,
  style
}: LazyImageProps) {
  const [imgLoaded, setImgLoaded] = useState(false)
  const [imgError, setImgError] = useState(false)

  const { src: lazySrc, isError, ref } = useLazyImage(src, {
    placeholder,
    rootMargin: '200px' // 提前200px开始加载
  })

  const handleLoad = useCallback(() => {
    setImgLoaded(true)
    onLoad?.()
  }, [onLoad])

  const handleError = useCallback(() => {
    setImgError(true)
    onError?.()
  }, [onError])

  // 显示fallback
  if (isError || imgError) {
    return fallback ? <>{fallback}</> : null
  }

  return (
    <img
      ref={ref as React.RefObject<HTMLImageElement>}
      src={lazySrc || placeholder}
      alt={alt}
      className={`${className} ${imgLoaded ? 'loaded' : 'loading'}`}
      style={{
        ...style,
        opacity: imgLoaded ? 1 : 0,
        transition: 'opacity 0.3s ease'
      }}
      onLoad={handleLoad}
      onError={handleError}
      loading="lazy"
      decoding="async"
    />
  )
}

const LazyImage = memo(LazyImageImpl)
export default LazyImage

// 带骨架屏的懒加载图片
export function LazyImageWithSkeleton({
  src,
  alt,
  className = '',
  skeletonClassName = '',
  style
}: LazyImageProps & { skeletonClassName?: string }) {
  const { src: lazySrc, isLoaded, isError } = useLazyImage(src, {
    rootMargin: '200px'
  })

  if (isError) {
    return (
      <div 
        className={`${skeletonClassName} skeleton-error`}
        style={style}
        role="img"
        aria-label={alt}
      >
        <span aria-hidden="true">🎬</span>
      </div>
    )
  }

  return (
    <div className="lazy-image-wrapper" style={style}>
      {!isLoaded && (
        <div className={`${skeletonClassName} skeleton-loading`} />
      )}
      <img
        src={lazySrc || ''}
        alt={alt}
        className={`${className} ${isLoaded ? 'loaded' : ''}`}
        style={{
          opacity: isLoaded ? 1 : 0,
          transition: 'opacity 0.3s ease'
        }}
        loading="lazy"
        decoding="async"
      />
    </div>
  )
}
