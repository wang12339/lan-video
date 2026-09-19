import { useState, useCallback, useMemo } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { useToast } from '../../../components/Toast/Toast'

/** 与 api/types.ts 的 ImageExif 保持一致（只读子集） */
export interface GalleryExif {
  takenAt?: string
  lat?: number
  lon?: number
  camera?: string
  lens?: string
  aperture?: number
  shutter?: string
  iso?: number
  focalLength?: number
  orientation?: number
}

interface ExifPanelProps {
  /** EXIF 数据（可能为 undefined） */
  exif: GalleryExif | undefined
  /** 面板是否打开 */
  open: boolean
  /** 关闭回调 */
  onClose: () => void
  /** 容器宽度断点：< 640px 时使用底部抽屉 */
  isMobile: boolean
}

interface ExifAction {
  label: string
  onClick?: () => void
  href?: string
}

interface ExifRow {
  label: string
  value?: string
  actions?: ExifAction[]
}

function formatAperture(aperture?: number): string | null {
  if (aperture == null || !Number.isFinite(aperture)) return null
  return `f/${aperture.toFixed(aperture % 1 === 0 ? 0 : 1)}`
}

function formatFocalLength(focalLength?: number): string | null {
  if (focalLength == null || !Number.isFinite(focalLength)) return null
  return `${Math.round(focalLength)}mm`
}

function formatCoords(lat?: number, lon?: number): string | null {
  if (lat == null || lon == null || !Number.isFinite(lat) || !Number.isFinite(lon)) return null
  return `${lat.toFixed(5)}, ${lon.toFixed(5)}`
}

function formatDateTime(takenAt: string | undefined, locale: string): string | null {
  if (!takenAt) return null
  try {
    const date = new Date(takenAt)
    if (isNaN(date.getTime())) return takenAt
    return date.toLocaleString(locale, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return takenAt
  }
}

/** 复制文本：优先 Clipboard API，失败降级到临时 textarea */
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    // 降级：创建临时 textarea + execCommand
  }
  const ta = document.createElement('textarea')
  ta.value = text
  ta.style.position = 'fixed'
  ta.style.opacity = '0'
  document.body.appendChild(ta)
  ta.select()
  let ok = false
  try {
    ok = document.execCommand('copy')
  } catch {
    ok = false
  }
  document.body.removeChild(ta)
  return ok
}

export default function ExifPanel({ exif, open, onClose, isMobile }: ExifPanelProps) {
  const { t, i18n } = useTranslation()
  const { toast } = useToast()
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(
    async (text: string) => {
      const ok = await copyText(text)
      if (!ok) {
        toast(t('common.saveFailed'), 'error')
        return
      }
      setCopied(true)
      toast(t('gallery.exifCopied'), 'success')
      setTimeout(() => setCopied(false), 1500)
    },
    [toast, t]
  )

  // 仅构建存在的字段，避免空行
  const rows = useMemo((): ExifRow[] => {
    if (!exif) return []
    const result: ExifRow[] = []

    const dateStr = formatDateTime(exif.takenAt, i18n.language)
    if (dateStr) result.push({ label: t('gallery.exifTakenAt'), value: dateStr })

    if (exif.camera) result.push({ label: t('gallery.exifCamera'), value: exif.camera })
    if (exif.lens) result.push({ label: t('gallery.exifLens'), value: exif.lens })

    const apertureStr = formatAperture(exif.aperture)
    if (apertureStr) result.push({ label: t('gallery.exifAperture'), value: apertureStr })

    if (exif.shutter) result.push({ label: t('gallery.exifShutter'), value: exif.shutter })
    if (exif.iso != null) result.push({ label: t('gallery.exifIso'), value: `ISO ${exif.iso}` })

    const focalStr = formatFocalLength(exif.focalLength)
    if (focalStr) result.push({ label: t('gallery.exifFocal'), value: focalStr })

    const coordsStr = formatCoords(exif.lat, exif.lon)
    if (coordsStr && exif.lat != null && exif.lon != null) {
      result.push({
        label: t('gallery.exifLocation'),
        value: coordsStr,
        actions: [
          {
            label: copied ? t('gallery.exifCopied') : t('gallery.exifCopyCoords'),
            onClick: () => void handleCopy(coordsStr),
          },
          {
            label: t('gallery.exifOpenMap'),
            href: `https://www.openstreetmap.org/?mlat=${exif.lat}&mlon=${exif.lon}#map=15/${exif.lat}/${exif.lon}`,
          },
        ],
      })
    }

    return result
  }, [exif, t, i18n.language, copied, handleCopy])

  if (!open) return null

  return createPortal(
    <div
      className={`exif-panel ${isMobile ? 'exif-panel--mobile' : ''}`}
      role="dialog"
      aria-modal="true"
      aria-label={t('gallery.exif')}
    >
      <div className="exif-panel-backdrop" onClick={onClose} />
      <div className="exif-panel-content" onClick={(e) => e.stopPropagation()}>
        <div className="exif-panel-header">
          <h3>{t('gallery.exif')}</h3>
          <button className="exif-panel-close" onClick={onClose} aria-label={t('gallery.close')}>
            ✕
          </button>
        </div>

        {rows.length === 0 ? (
          <div className="exif-panel-empty">{t('gallery.exifEmpty')}</div>
        ) : (
          <div className="exif-panel-body">
            <dl className="exif-list">
              {rows.map((row) => (
                <div key={row.label} className="exif-row">
                  <dt className="exif-label">{row.label}</dt>
                  <dd className="exif-value">
                    {row.value && <span className="exif-text">{row.value}</span>}
                    {row.actions && (
                      <span className="exif-actions">
                        {row.actions.map((action) =>
                          action.href ? (
                            <a
                              key={action.label}
                              className="exif-action-link"
                              href={action.href}
                              target="_blank"
                              rel="noopener noreferrer"
                            >
                              {action.label}
                            </a>
                          ) : (
                            <button
                              key={action.label}
                              type="button"
                              className="exif-action-btn"
                              onClick={action.onClick}
                            >
                              {action.label}
                            </button>
                          )
                        )}
                      </span>
                    )}
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        )}
      </div>
    </div>,
    document.body
  )
}
