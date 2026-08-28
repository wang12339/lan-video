import React from 'react'
import { useTranslation } from 'react-i18next'
import VideoCard from '../../components/VideoCard/VideoCard'

const VideoCardMemo = React.memo(VideoCard)

interface RecentVideo {
  id: string
  title: string
  thumbnail_url?: string
  thumb?: string
  views: number
  category?: string
  duration?: number
  date?: string
}

interface RecentSectionProps {
  recentVideos: RecentVideo[]
  viewMode: 'grid' | 'list'
}

export default function RecentSection({ recentVideos, viewMode }: RecentSectionProps) {
  const { t } = useTranslation()

  if (recentVideos.length === 0) return null

  return (
    <section className="trending-section recent-section" aria-label={t('home.recent')}>
      <h2 className="trending-title recent-title">{t('home.recent')}</h2>
      <div className={`video-grid recent-grid ${viewMode === 'list' ? 'list-view' : ''}`}>
        {recentVideos.map((video, i) => (
          <div key={`recent-${video.id}`} style={{ '--card-index': i } as React.CSSProperties}>
            <VideoCardMemo video={video} eager={i < 2} />
          </div>
        ))}
      </div>
    </section>
  )
}
