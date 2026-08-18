// 推荐 API

import { request, mediaUrl } from './client';
import type { MappedVideo } from './types';

export interface RecommendationItem {
  id: string;
  title: string;
  category: string | null;
  thumbUrl: string | null;
  score: number;
  reason: string;
}

export interface RecommendationResponse {
  items: RecommendationItem[];
  total: number;
}

export async function getTrendingVideos(): Promise<MappedVideo[]> {
  const res = await request<RecommendationResponse>('/recommendations/trending');
  return res.items.map(mapRecommendation);
}

export async function getSimilarVideos(videoId: string): Promise<MappedVideo[]> {
  const res = await request<RecommendationResponse>(`/recommendations/similar/${videoId}`);
  return res.items.map(mapRecommendation);
}

function mapRecommendation(item: RecommendationItem): MappedVideo {
  return {
    id: item.id,
    title: item.title,
    description: '',
    sourceType: 'local_video',
    cover: null,
    // 推荐接口不返回流地址，置 null（而非空串），避免 <video src=""> 之类的边界情况
    stream: null,
    thumb: mediaUrl(item.thumbUrl),
    category: item.category || '',
    views: 0,
    duration: 0,
    date: '',
    progress: 0,
    hasVariants: false,
  };
}
