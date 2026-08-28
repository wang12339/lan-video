import { useState, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { createShareLink, APIError } from '../../api'
import { trackClick } from '../../utils/track'
import type { MappedVideo } from '../../api/types'
import { SHARE_TOOLTIP_SUCCESS_MS, SHARE_TOOLTIP_ERROR_MS } from './constants'

export type ShareErrorType = 'auth' | 'not_found' | 'network' | 'unknown' | null

export function useShareHandler(user: { id: string } | null, video: MappedVideo | null) {
  const { t } = useTranslation()
  const [showShareTooltip, setShowShareTooltip] = useState(false)
  const [shareTooltipMsg, setShareTooltipMsg] = useState('')
  const [shareErrorType, setShareErrorType] = useState<ShareErrorType>(null)

  const handleShare = useCallback(async () => {
    if (!user || !video) return
    try {
      const res = await createShareLink(video.id)
      await navigator.clipboard.writeText(res.shareUrl)
      trackClick('分享')
      setShareTooltipMsg(t('player.linkCopied'))
      setShareErrorType(null)
      setShowShareTooltip(true)
      setTimeout(() => setShowShareTooltip(false), SHARE_TOOLTIP_SUCCESS_MS)
    } catch (e: unknown) {
      const errMsg = e instanceof Error ? e.message : ''
      if (e instanceof APIError && e.status === 401) {
        setShareTooltipMsg(t('player.shareExpired'))
        setShareErrorType('auth')
      } else if (e instanceof APIError && (e.status === 404 || e.status === 410)) {
        setShareTooltipMsg(t('player.shareOpFailed'))
        setShareErrorType('not_found')
      } else if (e instanceof APIError && e.status === 0) {
        setShareTooltipMsg(t('player.shareFailed'))
        setShareErrorType('network')
      } else {
        setShareTooltipMsg(errMsg || t('player.shareFailed'))
        setShareErrorType('unknown')
      }
      setShowShareTooltip(true)
      setTimeout(() => setShowShareTooltip(false), SHARE_TOOLTIP_ERROR_MS)
    }
  }, [user, video, t])

  return { handleShare, showShareTooltip, shareTooltipMsg, shareErrorType }
}
