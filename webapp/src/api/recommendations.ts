// 推荐 API

import { request } from './client';
import type { MappedVideo } from './types';

export interface RecommendationItem {
  id: number;
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

export async function getRecommendations(): Promise<MappedVideo[]> {
  const res = await request<RecommendationResponse>('/recommendations');
  return res.items.map(mapRecommendation);
}

export async function getTrendingVideos(): Promise<MappedVideo[]> {
  const res = await request<RecommendationResponse>('/recommendations/trending');
  return res.items.map(mapRecommendation);
}

export async function getRecentVideos(): Promise<MappedVideo[]> {
  const res = await request<RecommendationResponse>('/recommendations/recent');
  return res.items.map(mapRecommendation);
}

export async function getSimilarVideos(videoId: number): Promise<MappedVideo[]> {
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
    stream: '',
    thumb: item.thumbUrl,
    category: item.category || '',
    views: 0,
    duration: 0,
    date: '',
    progress: 0,
    hasVariants: false,
  };
}
