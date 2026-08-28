import React from 'react'
import { useTranslation } from 'react-i18next'
import type { MappedVideo } from '../../api/types'
import VideoCard from '../../components/VideoCard/VideoCard'

const VideoCardMemo = React.memo(VideoCard)

interface TrendingSectionProps {
  trending: MappedVideo[]
  viewMode: 'grid' | 'list'
}

export default function TrendingSection({ trending, viewMode }: TrendingSectionProps) {
  const { t } = useTranslation()

  if (trending.length === 0) return null

  return (
    <section className="trending-section" aria-label={t('home.trending')}>
      <h2 className="trending-title">{t('home.trending')}</h2>
      <div className={`video-grid ${viewMode === 'list' ? 'list-view' : ''}`}>
        {trending.map((video, i) => (
          <div key={`trend-${video.id}`} style={{ '--card-index': i } as React.CSSProperties}>
            <VideoCardMemo video={video} eager={i < 4} />
          </div>
        ))}
      </div>
    </section>
  )
}
