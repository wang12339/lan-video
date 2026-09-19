// 公共聊天室全局连接：单例 WebSocket + 事件分发 + 未读计数。
// 挂在 AuthProvider 内（依赖登录态），登录期间全程保活，
// 聊天页只是众多消费者之一 —— 未读提醒的实时性依赖连接不随页面关闭。

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useLocation } from 'react-router-dom'
import { ChatClient } from '../api'
import type { ChatEvent } from '../api'
import { useAuth } from './AuthContext'

type ChatStatus = 'connected' | 'reconnecting' | 'closed' | 'init'

interface ChatContextValue {
  /** 连接状态（聊天页显示重连横幅/禁用发送用） */
  status: ChatStatus
  /** 未读消息数（不在 /chat 页时收到新消息则 +1；进入 /chat 清零） */
  unread: number
  clearUnread: () => void
  /** 订阅实时事件（聊天页消费消息流/在线名单；返回退订函数） */
  subscribe: (cb: (ev: ChatEvent) => void) => () => void
  /** 发送客户端（未登录时为 null） */
  client: ChatClient | null
  /** 最新在线名单（连接保活期间持续更新，聊天页挂载即可拿到） */
  online: { count: number; names: string[] }
}

const ChatContext = createContext<ChatContextValue | null>(null)

export function ChatProvider({ children }: { children: React.ReactNode }) {
  const { user } = useAuth()
  // 仅以 userId 作为连接标识：资料刷新/401 复验会返回新的 user 对象引用，
  // 但同一账号不应因此断开并重建 WebSocket。
  const userId = user?.id
  const location = useLocation()
  const [status, setStatus] = useState<ChatStatus>('init')
  const [unread, setUnread] = useState(0)
  const [online, setOnline] = useState<{ count: number; names: string[] }>({
    count: 0,
    names: [],
  })
  const [client, setClient] = useState<ChatClient | null>(null)
  const subscribersRef = useRef(new Set<(ev: ChatEvent) => void>())
  const onChatPageRef = useRef(location.pathname === '/chat')

  // 渲染阶段只读 ref：页面归属在 effect 中更新（可与未读清零合并）
  useEffect(() => {
    onChatPageRef.current = location.pathname === '/chat'
    if (location.pathname === '/chat') setUnread(0)
  }, [location.pathname])

  useEffect(() => {
    if (!userId) {
      // 登出：断开并复位
      setClient((prev) => {
        prev?.close()
        return null
      })
      setOnline({ count: 0, names: [] })
      setUnread(0)
      setStatus('init')
      return
    }
    const c = new ChatClient({
      onEvent: (ev: ChatEvent) => {
        if (ev.type === 'message' && !onChatPageRef.current) {
          setUnread((n) => n + 1)
        }
        if (ev.type === 'cleared') {
          // 聊天室被管理员清空：未读一并归零
          setUnread(0)
        }
        if (ev.type === 'online') {
          setOnline({ count: ev.count, names: ev.names })
        }
        subscribersRef.current.forEach((cb) => cb(ev))
      },
      onStatus: (s) => setStatus(s),
    })
    setClient(c)
    c.connect()
    return () => {
      c.close()
      setClient(null)
    }
  }, [userId])

  const subscribe = useCallback((cb: (ev: ChatEvent) => void) => {
    subscribersRef.current.add(cb)
    return () => {
      subscribersRef.current.delete(cb)
    }
  }, [])

  const clearUnread = useCallback(() => setUnread(0), [])

  const value = useMemo(
    () => ({ status, unread, clearUnread, subscribe, client, online }),
    [status, unread, clearUnread, subscribe, client, online]
  )

  return <ChatContext.Provider value={value}>{children}</ChatContext.Provider>
}

export function useChatRoom(): ChatContextValue {
  const ctx = useContext(ChatContext)
  if (!ctx) throw new Error('useChatRoom must be used within ChatProvider')
  return ctx
}
