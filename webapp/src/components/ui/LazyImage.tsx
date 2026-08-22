import { useState, useCallback, memo, useMemo, useEffect } from 'react'
import { useLazyImage } from '../../hooks/useLazyImage'

// ===== WebP 支持检测 =====
let _webpSupported: boolean | null = null

/**
 * 检测浏览器是否支持 WebP 格式
 * 使用特征检测而非 UA 嗅探
 */
export function isWebPSupported(): boolean {
  if (_webpSupported !== null) return _webpSupported

  // 在 SSR 或 canvas 不可用时回退
  try {
    const canvas = document.createElement('canvas')
    canvas.width = 1
    canvas.height = 1
    _webpSupported = canvas.toDataURL('image/webp').indexOf('data:image/webp') === 0
  } catch {
    _webpSupported = false
  }
  return _webpSupported
}

// ===== 图片 URL 工具函数 =====

/**
 * 为图片 URL 添加 WebP 格式后缀（如果浏览器支持且 URL 为本地路径）
 * 示例：/media/cover_123.jpg → /media/cover_123.webp
 *
 * 当前后端不生成 WebP 文件，因此暂时禁用自动转换。
 * 待后端支持 WebP 转码后再启用：去掉下面的 return url 即可。
 */
function getWebPUrl(url: string | null | undefined): string | null | undefined {
  if (!url) return url

  // 后端未生成 .webp 文件，直接返回原始 URL 避免 404
  return url
}

/**
 * 生成响应式图片的 srcSet 字符串
 * 假设后端支持通过查询参数获取不同尺寸：/media/cover_123.jpg?w=320
 *
 * 当前后端不支持图片调整大小，srcSet 的 ?w= 参数无意义，暂时禁用。
 * 待后端支持图片代理/调整大小后再启用。
 */
function generateSrcSet(
  _baseUrl: string,
  _sizes: number[] = [320, 640, 960, 1280]
): string | undefined {
  // 后端不支持图片调整大小，返回 undefined 禁用 srcSet
  return undefined
}

/**
 * 根据使用场景生成默认的 sizes 属性
 */
function getDefaultSizes(context?: 'thumbnail' | 'card' | 'hero'): string {
  switch (context) {
    case 'thumbnail':
      return '(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 25vw'
    case 'card':
      return '(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 33vw'
    case 'hero':
      return '100vw'
    default:
      return '(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 33vw'
  }
}

// ===== 占位符样式 =====

/** 加载中占位符：骨架屏闪烁效果 */
const PLACEHOLDER_LOADING_STYLE: React.CSSProperties = {
  backgroundColor: 'var(--bg3, #f0f0f0)',
  backgroundImage: 'linear-gradient(90deg, var(--bg3, #f0f0f0) 25%, var(--surface, #e8e8e8) 50%, var(--bg3, #f0f0f0) 75%)',
  backgroundSize: '200% 100%',
  animation: 'shimmer 1.5s ease-in-out infinite'
}

/** 错误回退占位符 */
const PLACEHOLDER_ERROR_STYLE: React.CSSProperties = {
  backgroundColor: 'var(--bg3, #f0f0f0)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  color: 'var(--text3, #999)',
  fontSize: '2rem'
}

// ===== Props 接口 =====

interface LazyImageProps {
  /** 图片源 URL */
  src: string | null | undefined
  /** 替代文本 */
  alt: string
  /** CSS 类名 */
  className?: string
  /** 占位图片 URL（加载完成前显示） */
  placeholder?: string
  /** 错误时的回退内容（React 节点） */
  fallback?: React.ReactNode
  /** 错误时的回退图片 URL */
  fallbackSrc?: string
  /** 加载完成回调 */
  onLoad?: () => void
  /** 加载失败回调 */
  onError?: () => void
  /** 内联样式 */
  style?: React.CSSProperties
  /** 是否启用 WebP 自动转换（默认 true） */
  enableWebP?: boolean
  /** 是否启用响应式图片（默认 true） */
  responsive?: boolean
  /** 响应式图片使用场景 */
  imageContext?: 'thumbnail' | 'card' | 'hero'
  /** 自定义 srcSet（覆盖自动生成） */
  srcSet?: string
  /** 自定义 sizes 属性 */
  sizes?: string
  /** 是否显示加载占位符（默认 true） */
  showPlaceholder?: boolean
  /** 立即加载，跳过 IntersectionObserver（用于首屏 LCP 关键图片） */
  eager?: boolean
}

// ===== 占位符组件 =====

function LoadingPlaceholder({ style, className }: { style?: React.CSSProperties; className?: string }) {
  return (
    <div
      className={`lazy-image-placeholder ${className || ''}`}
      style={{ ...PLACEHOLDER_LOADING_STYLE, ...style }}
      aria-hidden="true"
    />
  )
}

function ErrorPlaceholder({
  style,
  className,
  alt
}: {
  style?: React.CSSProperties
  className?: string
  alt: string
}) {
  return (
    <div
      className={`lazy-image-error ${className || ''}`}
      style={{ ...PLACEHOLDER_ERROR_STYLE, ...style }}
      role="img"
      aria-label={alt}
    >
      <span aria-hidden="true">🎬</span>
    </div>
  )
}

// ===== 主组件 =====

function LazyImageImpl({
  src,
  alt,
  className = '',
  placeholder,
  fallback,
  fallbackSrc,
  onLoad,
  onError,
  style,
  enableWebP = true,
  responsive = true,
  imageContext,
  srcSet: customSrcSet,
  sizes: customSizes,
  showPlaceholder = true,
  eager = false
}: LazyImageProps) {
  const [imgLoaded, setImgLoaded] = useState(false)
  const [imgError, setImgError] = useState(false)
  const [currentSrc, setCurrentSrc] = useState<string | null | undefined>(src)
  const [webpFailed, setWebpFailed] = useState(false)

  // 计算最终的图片 URL（支持 WebP，如果 WebP 失败则回退到原始格式）
  const finalSrc = useMemo(() => {
    if (!currentSrc) return currentSrc
    if (enableWebP && isWebPSupported() && !webpFailed) {
      return getWebPUrl(currentSrc) || currentSrc
    }
    return currentSrc
  }, [currentSrc, enableWebP, webpFailed])

  // 计算响应式属性
  const responsiveProps = useMemo(() => {
    if (!responsive || !finalSrc) return {}

    const computedSrcSet = customSrcSet || generateSrcSet(finalSrc)
    const computedSizes = customSizes || getDefaultSizes(imageContext)

    return {
      srcSet: computedSrcSet,
      sizes: computedSizes
    }
  }, [responsive, finalSrc, customSrcSet, customSizes, imageContext])

  // 当原始 src 变化时重置状态
  useEffect(() => {
    setCurrentSrc(src)
    setImgLoaded(false)
    setImgError(false)
    setWebpFailed(false)
  }, [src])

  const { src: lazySrc, isError, ref } = useLazyImage(eager ? null : finalSrc, {
    placeholder,
    rootMargin: '200px'
  })

  const handleLoad = useCallback(() => {
    setImgLoaded(true)
    onLoad?.()
  }, [onLoad])

  const handleError = useCallback(() => {
    // 如果启用了 WebP 且失败了，标记 WebP 失败并重试原始格式
    if (enableWebP && isWebPSupported() && !webpFailed && currentSrc) {
      const webpUrl = getWebPUrl(currentSrc)
      // 如果当前尝试的是 WebP 版本，标记失败并重试
      if (finalSrc === webpUrl) {
        setWebpFailed(true)
        return
      }
    }

    // 如果有 fallbackSrc，尝试使用它
    if (fallbackSrc && currentSrc !== fallbackSrc) {
      setCurrentSrc(fallbackSrc)
      return
    }

    setImgError(true)
    onError?.()
  }, [enableWebP, webpFailed, currentSrc, finalSrc, fallbackSrc, onError])

  // 显示错误回退
  if (isError || imgError) {
    if (fallback) return <>{fallback}</>
    if (showPlaceholder) {
      return <ErrorPlaceholder style={style} className={className} alt={alt} />
    }
    return null
  }

  // 计算图片样式
  const imgStyle: React.CSSProperties = {
    ...style,
    opacity: imgLoaded ? 1 : 0,
    transition: 'opacity 0.3s ease'
  }

  const effectiveSrc = eager ? (finalSrc || placeholder) : (lazySrc || placeholder)

  return (
    <>
      {!imgLoaded && showPlaceholder && !eager && (
        <LoadingPlaceholder style={style} className={className} />
      )}
      <img
        ref={ref as React.RefObject<HTMLImageElement>}
        src={effectiveSrc || placeholder}
        alt={alt}
        className={`${className} ${imgLoaded ? 'loaded' : 'loading'}`}
        style={imgStyle}
        onLoad={handleLoad}
        onError={handleError}
        loading={eager ? 'eager' : 'lazy'}
        // @ts-expect-error fetchPriority is valid for img but React types don't include it yet
        fetchpriority={eager ? 'high' : 'auto'}
        decoding="async"
        {...responsiveProps}
      />
    </>
  )
}

const LazyImage = memo(LazyImageImpl)
export default LazyImage

// ===== 带骨架屏的懒加载图片 =====

export function LazyImageWithSkeleton({
  src,
  alt,
  className = '',
  skeletonClassName = '',
  style,
  enableWebP = true,
  responsive = true,
  imageContext
}: LazyImageProps & { skeletonClassName?: string }) {
  const [imgLoaded, setImgLoaded] = useState(false)
  const [imgError, setImgError] = useState(false)
  const [currentSrc, setCurrentSrc] = useState<string | null | undefined>(src)

  // 计算最终的图片 URL（支持 WebP）
  const finalSrc = useMemo(() => {
    if (!currentSrc) return currentSrc
    if (enableWebP && isWebPSupported()) {
      return getWebPUrl(currentSrc) || currentSrc
    }
    return currentSrc
  }, [currentSrc, enableWebP])

  // 计算响应式属性
  const responsiveProps = useMemo(() => {
    if (!responsive || !finalSrc) return {}
    return {
      srcSet: generateSrcSet(finalSrc),
      sizes: getDefaultSizes(imageContext)
    }
  }, [responsive, finalSrc, imageContext])

  // 当原始 src 变化时重置状态
  useEffect(() => {
    setCurrentSrc(src)
    setImgLoaded(false)
    setImgError(false)
  }, [src])

  const { src: lazySrc, isLoaded, isError } = useLazyImage(finalSrc, {
    rootMargin: '200px'
  })

  const handleLoad = useCallback(() => {
    setImgLoaded(true)
  }, [])

  const handleError = useCallback(() => {
    // WebP 回退
    if (enableWebP && isWebPSupported() && currentSrc) {
      const originalUrl = currentSrc
      const webpUrl = getWebPUrl(originalUrl)
      if (finalSrc === webpUrl && finalSrc !== originalUrl) {
        setCurrentSrc(originalUrl)
        return
      }
    }
    setImgError(true)
  }, [enableWebP, currentSrc, finalSrc])

  if (isError || imgError) {
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
      {!isLoaded && !imgLoaded && (
        <div className={`${skeletonClassName} skeleton-loading`} />
      )}
      <img
        src={lazySrc || ''}
        alt={alt}
        className={`${className} ${isLoaded || imgLoaded ? 'loaded' : ''}`}
        style={{
          opacity: isLoaded || imgLoaded ? 1 : 0,
          transition: 'opacity 0.3s ease'
        }}
        onLoad={handleLoad}
        onError={handleError}
        loading="lazy"
        decoding="async"
        {...responsiveProps}
      />
    </div>
  )
}
