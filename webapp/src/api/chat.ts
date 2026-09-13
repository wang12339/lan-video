// 公共聊天室 API：历史分页（HTTP）+ 实时事件（WebSocket）

import { request } from './client';

export interface ChatMessage {
  id: number;
  userId?: number;
  username: string;
  isGuest: boolean;
  content: string;
  /** 0=文本、1=图片、2=视频 */
  msgType: number;
  /** 图片消息的 /media/chat/ 地址（仅 msgType=1） */
  imageUrl?: string;
  /** 视频消息的 /media/chat/ 地址（仅 msgType=2） */
  videoUrl?: string;
  ts: string;
}

export interface ChatHistoryResponse {
  items: ChatMessage[];
  hasMore: boolean;
}

export type ChatEvent =
  | ({ type: 'message' } & ChatMessage)
  | { type: 'online'; count: number; names: string[] }
  | { type: 'deleted'; id: number }
  | { type: 'cleared' }
  | { type: 'error'; message: string };

/** 历史分页（id 倒序返回，调用方自行反转拼接） */
export async function fetchChatHistory(
  beforeId?: number,
  limit = 50
): Promise<ChatHistoryResponse> {
  const params = new URLSearchParams();
  if (beforeId !== undefined) params.set('before_id', String(beforeId));
  params.set('limit', String(limit));
  const qs = params.toString();
  return request<ChatHistoryResponse>(`/chat/messages${qs ? `?${qs}` : ''}`);
}

/** 聊天图片上传（≤10MB，JPG/PNG/WebP/GIF，服务端按 magic bytes 校验） */
export async function uploadChatImage(file: File): Promise<string> {
  const form = new FormData();
  form.append('file', file);
  const resp = await request<{ ok: boolean; imageUrl: string }>('/chat/image', {
    method: 'POST',
    body: form,
    timeout: 120000,
  });
  return resp.imageUrl;
}

/** 聊天视频上传（≤50MB，MP4/WebM，服务端按 magic bytes 校验；流式写盘） */
export async function uploadChatVideo(file: File): Promise<string> {
  const form = new FormData();
  form.append('file', file);
  const resp = await request<{ ok: boolean; videoUrl: string }>('/chat/video', {
    method: 'POST',
    body: form,
    timeout: 300000,
  });
  return resp.videoUrl;
}

/** 聊天室消息总数（管理员） */
export async function fetchChatStats(): Promise<{ ok: boolean; count: number }> {
  return request<{ ok: boolean; count: number }>('/admin/chat/stats');
}

/** 清空聊天室全部消息（管理员；在线客户端会收到 cleared 事件清屏） */
export async function clearAllChatMessages(): Promise<{ ok: boolean; deleted: number }> {
  return request<{ ok: boolean; deleted: number }>('/admin/chat/messages', {
    method: 'DELETE',
  });
}

export interface ChatClientHandlers {
  onEvent: (ev: ChatEvent) => void;
  /** 连接状态变化：connected / reconnecting / closed（手动） */
  onStatus: (s: 'connected' | 'reconnecting' | 'closed') => void;
}

/**
 * 聊天室 WebSocket 客户端。
 * - cookie 会话鉴权（同源 WS 升级自动携带 HttpOnly cookie）
 * - 指数退避自动重连（1s 起步、上限 30s，抖动防惊群）
 * - 发送在未连接/连接中时抛错，由调用方提示
 */
export class ChatClient {
  private ws: WebSocket | null = null;
  private closedByUser = false;
  private retry = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(private handlers: ChatClientHandlers) {}

  connect(): void {
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) {
      return;
    }
    this.closedByUser = false;
    const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${window.location.host}/ws/chat`);
    this.ws = ws;

    ws.onopen = () => {
      this.retry = 0;
      this.handlers.onStatus('connected');
    };
    ws.onmessage = (e) => {
      try {
        const ev = JSON.parse(e.data as string) as ChatEvent;
        this.handlers.onEvent(ev);
      } catch {
        // 非 JSON 帧：忽略
      }
    };
    ws.onclose = () => {
      this.handlers.onStatus(this.closedByUser ? 'closed' : 'reconnecting');
      if (this.closedByUser) return;
      this.scheduleReconnect();
    };
    ws.onerror = () => {
      // onclose 会跟随触发，重连逻辑在 onclose 里
    };
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return;
    const delay = Math.min(30000, 1000 * 2 ** this.retry) + Math.random() * 500;
    this.retry += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  /** 发送一条文本消息。未连接时抛错。 */
  send(content: string): void {
    this.sendRaw({ type: 'msg', content });
  }

  /** 发送一条图片消息（imageUrl 为 /chat/image 上传返回的地址）。未连接时抛错。 */
  sendImage(imageUrl: string): void {
    this.sendRaw({ type: 'img', imageUrl });
  }

  /** 发送一条视频消息（videoUrl 为 /chat/video 上传返回的地址）。未连接时抛错。 */
  sendVideo(videoUrl: string): void {
    this.sendRaw({ type: 'video', videoUrl });
  }

  private sendRaw(payload: Record<string, unknown>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('chat.notConnected');
    }
    this.ws.send(JSON.stringify(payload));
  }

  close(): void {
    this.closedByUser = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
