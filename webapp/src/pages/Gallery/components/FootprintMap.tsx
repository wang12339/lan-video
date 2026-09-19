import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { Map as LeafletMap } from 'leaflet'
import './FootprintMap.css'

export interface FootprintPoint {
  id: string
  lat: number
  lon: number
  title: string
  thumb?: string
}

interface FootprintMapProps {
  points: FootprintPoint[]
  open: boolean
  onClose: () => void
}

/**
 * GPS 足迹地图：leaflet 懒加载（仅打开时加载 JS/CSS），瓦片来自 OSM
 * （后端 CSP 已放行 tile.openstreetmap.org；无网络时仅显示空白底图）。
 * 标记使用 divIcon，避免 bundler 下 leaflet 默认图标路径失效。
 */
export default function FootprintMap({ points, open, onClose }: FootprintMapProps) {
  const { t } = useTranslation()
  const containerRef = useRef<HTMLDivElement>(null)
  const [ready, setReady] = useState(false)

  // Esc 关闭
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, onClose])

  useEffect(() => {
    if (!open || points.length === 0) return
    let disposed = false
    let map: LeafletMap | null = null

    void (async () => {
      const L = await import('leaflet')
      await import('leaflet/dist/leaflet.css')
      if (disposed || !containerRef.current) return

      map = L.map(containerRef.current, { scrollWheelZoom: true, worldCopyJump: true })
      L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
        maxZoom: 19,
        attribution: '&copy; OpenStreetMap contributors',
      }).addTo(map)

      const icon = L.divIcon({ className: 'footprint-marker', html: '<span></span>', iconSize: [14, 14] })
      for (const point of points) {
        const marker = L.marker([point.lat, point.lon], { icon }).addTo(map)
        // 用 DOM 构建 popup，避免标题/缩略图注入 HTML
        const popup = document.createElement('div')
        popup.className = 'footprint-popup'
        if (point.thumb) {
          const img = document.createElement('img')
          img.src = point.thumb
          img.alt = ''
          img.loading = 'lazy'
          popup.appendChild(img)
        }
        const title = document.createElement('strong')
        title.textContent = point.title
        popup.appendChild(title)
        marker.bindPopup(popup)
      }

      const first = points[0]!
      if (points.length === 1) {
        map.setView([first.lat, first.lon], 13)
      } else {
        map.fitBounds(
          L.latLngBounds(points.map((p) => [p.lat, p.lon] as [number, number])).pad(0.2)
        )
      }
      setReady(true)
    })().catch(() => {
      // leaflet 加载失败（离线/被拦截）：保持容器空白，不阻塞关闭
    })

    return () => {
      disposed = true
      map?.remove()
      map = null
    }
  }, [open, points])

  if (!open) return null

  return createPortal(
    <div className="footprint-modal" role="dialog" aria-modal="true" aria-label={t('gallery.footprint')}>
      <div className="footprint-backdrop" onClick={onClose} />
      <div className="footprint-content">
        <div className="footprint-header">
          <div>
            <h3>{t('gallery.footprint')}</h3>
            <span className="footprint-count">
              {t('gallery.footprintPoints', { count: points.length })}
            </span>
          </div>
          <button className="footprint-close" onClick={onClose} aria-label={t('gallery.close')}>
            ✕
          </button>
        </div>
        {points.length === 0 ? (
          <div className="footprint-empty">{t('gallery.footprintEmpty')}</div>
        ) : (
          <div className="footprint-map-wrap">
            {!ready && <div className="footprint-loading">{t('common.loading')}</div>}
            <div ref={containerRef} className="footprint-map" />
          </div>
        )}
      </div>
    </div>,
    document.body
  )
}
