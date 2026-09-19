import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from '../../context/AuthContext'
import { useChatRoom } from '../../context/ChatContext'
import { fetchChatHistory, uploadChatImage, uploadChatVideo } from '../../api'
import type { ChatEvent, ChatMessage } from '../../api'
import { useFocusTrap } from '../../hooks/useFocusTrap'
import './Chat.css'

// 用户名仅过滤控制字符，可能含正则元字符，拼进正则前必须转义
function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function formatTs(ts: string): string {
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ''
  const now = new Date()
  const sameDay = d.toDateString() === now.toDateString()
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
  return sameDay ? hm : `${d.getMonth() + 1}/${d.getDate()} ${hm}`
}

// 常用表情面板（纯文本插入，无协议改动）
const EMOJI_GROUPS: { label: string; items: string[] }[] = [
  {
    label: '😀',
    items: ['😀', '😁', '😂', '🤣', '😊', '😍', '😘', '😜', '🤔', '😎', '😭', '😡', '🥳', '😱', '🤯', '😴'],
  },
  {
    label: '👍',
    items: ['👍', '👎', '👏', '🙏', '🤝', '💪', '✌️', '👌', '🫡', '🫶', '❤️', '💔', '⭐', '🔥', '🎉', '✨'],
  },
  {
    label: '🍿',
    items: ['🍿', '🎬', '📺', '📷', '🎮', '🎵', '☕', '🍺', '🍰', '🍉', '🐶', '🐱', '🌸', '🌈', '☀️', '🌙'],
  },
]

// 实时追加缓冲区上限：只保留最近 MAX_MESSAGES 条，防止长时间挂机无限累积
const MAX_MESSAGES = 500
// 含手动加载历史时的硬上限：loadOlder 是头部插入，若也裁到 500 会立刻丢掉
// 刚加载的内容并破坏"加载更早"语义，因此仅在超过该值时才从头部丢弃最旧的
const MAX_MESSAGES_WITH_HISTORY = 1000

function Chat() {
  const { t } = useTranslation()
  const { user } = useAuth()
  const { status, subscribe, client, online } = useChatRoom()

  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [hasMore, setHasMore] = useState(false)
  const [loadingHistory, setLoadingHistory] = useState(true)
  const [input, setInput] = useState('')
  const [sendError, setSendError] = useState('')
  const [loadingOlder, setLoadingOlder] = useState(false)
  const [emojiOpen, setEmojiOpen] = useState(false)
  // 回到底部按钮：不在底部时显示
  const [atBottom, setAtBottom] = useState(true)
  // 图片上传中（按钮 loading + 禁用）
  const [uploadingImage, setUploadingImage] = useState(false)
  const [uploadingVideo, setUploadingVideo] = useState(false)
  // @ 补全：激活时为已输入的查询串（@ 之后、光标前的文本）
  const [mentionQuery, setMentionQuery] = useState<string | null>(null)
  const [mentionIdx, setMentionIdx] = useState(0)
  // 图片查看
  const [lightbox, setLightbox] = useState<string | null>(null)

  const listRef = useRef<HTMLDivElement>(null)
  const lightboxRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const imageInputRef = useRef<HTMLInputElement>(null)
  const videoInputRef = useRef<HTMLInputElement>(null)
  const atBottomRef = useRef(true)
  // 自己的 uid → 用于高亮"我"的消息
  const myUserId = useMemo(() => user?.id, [user?.id])
  const myUsername = user?.username ?? ''

  // 追加消息并按需滚动到底（仅当用户本来就在底部时，避免阅读历史被打断）
  const appendMessage = useCallback((m: ChatMessage) => {
    setMessages((prev) => {
      if (prev.some((x) => x.id === m.id)) return prev // 去重（自己发送的消息会经广播回来）
      const next = [...prev, m]
      // 追加路径统一裁剪：常规保留最近 MAX_MESSAGES 条；
      // 若用户已手动翻出更多历史（长度超软上限），放宽到硬上限，
      // 避免来一条新消息就把正在阅读的历史裁掉。
      const cap = prev.length > MAX_MESSAGES ? MAX_MESSAGES_WITH_HISTORY : MAX_MESSAGES
      return next.length > cap ? next.slice(-cap) : next
    })
  }, [])

  // 初始历史
  useEffect(() => {
    if (!user) return
    let cancelled = false
    void (async () => {
      try {
        const resp = await fetchChatHistory(undefined, 50)
        if (cancelled) return
        // 倒序 → 反转为正序展示
        setMessages(resp.items.slice().reverse())
        setHasMore(resp.hasMore)
      } catch {
        /* 历史加载失败不阻塞实时聊天 */
      } finally {
        if (!cancelled) setLoadingHistory(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [user])

  // 订阅全局连接的事件流（连接本身在 ChatProvider 中保活）
  useEffect(() => {
    if (!user) return
    return subscribe((ev: ChatEvent) => {
      switch (ev.type) {
        case 'message':
          appendMessage({
            id: ev.id,
            userId: ev.userId,
            username: ev.username,
            isGuest: ev.isGuest,
            content: ev.content,
            msgType: ev.msgType,
            imageUrl: ev.imageUrl,
            videoUrl: ev.videoUrl,
            ts: ev.ts,
          })
          break
        case 'deleted':
          setMessages((prev) => prev.filter((m) => m.id !== ev.id))
          break
        case 'cleared':
          // 管理员清空了聊天室
          setMessages([])
          setHasMore(false)
          break
        case 'error':
          setSendError(ev.message)
          break
      }
    })
  }, [user, subscribe, appendMessage])

  // 自动滚动：新消息到达且视口在底部时跟随
  useEffect(() => {
    if (atBottomRef.current) {
      listRef.current?.scrollTo({ top: listRef.current.scrollHeight })
    }
  }, [messages])

  const handleScroll = useCallback(() => {
    const el = listRef.current
    if (!el) return
    const bottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60
    atBottomRef.current = bottom
    setAtBottom(bottom)
  }, [])

  // iOS 键盘适配：visualViewport 高度随键盘弹出收缩，写入 CSS 变量
  // 让聊天壳跟着收缩（输入栏始终贴在键盘上方），并在贴底时跟随滚动
  useEffect(() => {
    const vv = window.visualViewport
    if (!vv) return
    const root = document.documentElement
    const apply = () => {
      root.style.setProperty('--chat-vvh', `${vv.height}px`)
      if (atBottomRef.current) {
        listRef.current?.scrollTo({ top: listRef.current.scrollHeight })
      }
    }
    apply()
    vv.addEventListener('resize', apply)
    return () => {
      vv.removeEventListener('resize', apply)
      root.style.removeProperty('--chat-vvh')
    }
  }, [])

  // 灯箱 Esc 关闭：仅在灯箱打开时挂载监听，避免与全局键盘事件/输入框冲突
  useEffect(() => {
    if (!lightbox) return
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setLightbox(null)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [lightbox])

  // 灯箱打开时陷阱焦点，关闭后由 hook 恢复到触发元素
  useFocusTrap(lightboxRef, lightbox !== null)

  const loadOlder = useCallback(async () => {
    const oldest = messages[0]
    if (!oldest || loadingOlder) return
    setLoadingOlder(true)
    try {
      const el = listRef.current
      const prevHeight = el?.scrollHeight ?? 0
      const resp = await fetchChatHistory(oldest.id, 50)
      if (resp.items.length > 0) {
        setMessages((prev) => {
          const next = [...resp.items.slice().reverse(), ...prev]
          // 头部插入历史不做软裁剪，仅保留硬上限保护，保持"加载更早"可用
          return next.length > MAX_MESSAGES_WITH_HISTORY
            ? next.slice(-MAX_MESSAGES_WITH_HISTORY)
            : next
        })
        setHasMore(resp.hasMore)
        // 保持视口位置：新内容插入顶部后补偿滚动差
        requestAnimationFrame(() => {
          if (el) el.scrollTop = el.scrollHeight - prevHeight
        })
      } else {
        setHasMore(false)
      }
    } catch {
      /* 静默，可重试 */
    } finally {
      setLoadingOlder(false)
    }
  }, [messages, loadingOlder])

  const handleSend = useCallback(() => {
    const content = input.trim()
    if (!content) return
    setSendError('')
    try {
      client?.send(content)
      setInput('')
      setMentionQuery(null)
      setEmojiOpen(false) // 发送后收起表情面板
    } catch {
      setSendError(t('chat.notConnected'))
    }
  }, [input, t, client])

  // ---------- @ 提及补全 ----------

  // @ 候选：按查询串过滤在线名单（排除自己），上限 6
  const mentionCandidates = useMemo(() => {
    if (mentionQuery === null) return []
    const q = mentionQuery.toLowerCase()
    return online.names.filter((n) => n !== myUsername && n.toLowerCase().includes(q)).slice(0, 6)
  }, [mentionQuery, online.names, myUsername])

  // 输入变化时检测 @ 后缀（光标前以 @word 结尾即激活补全）
  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const v = e.target.value
    setInput(v)
    const cursor = e.target.selectionStart ?? v.length
    const before = v.slice(0, cursor)
    const m = /@([a-zA-Z0-9_-]*)$/.exec(before)
    if (m) {
      setMentionQuery(m[1] ?? '')
      setMentionIdx(0)
    } else {
      setMentionQuery(null)
    }
  }, [])

  // 应用候选名：替换光标前的 @query 为 @name + 空格
  const applyMention = useCallback(
    (name: string) => {
      const el = inputRef.current
      const cursor = el?.selectionStart ?? input.length
      const before = input.slice(0, cursor)
      const after = input.slice(cursor)
      const m = /@([a-zA-Z0-9_-]*)$/.exec(before)
      const replaced = m ? `${before.slice(0, m.index)}@${name} ` : `${before}@${name} `
      setInput(replaced + after)
      setMentionQuery(null)
      // 光标回到插入点末尾
      requestAnimationFrame(() => {
        el?.focus()
        const pos = replaced.length
        el?.setSelectionRange(pos, pos)
      })
    },
    [input]
  )

  const handleInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      // 补全面板打开时接管方向键/回车
      if (mentionQuery !== null && mentionCandidates.length > 0) {
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          setMentionIdx((i) => (i + 1) % mentionCandidates.length)
          return
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault()
          setMentionIdx((i) => (i - 1 + mentionCandidates.length) % mentionCandidates.length)
          return
        }
        if (e.key === 'Enter' || e.key === 'Tab') {
          e.preventDefault()
          const name = mentionCandidates[mentionIdx]
          if (name) applyMention(name)
          return
        }
        if (e.key === 'Escape') {
          e.preventDefault()
          setMentionQuery(null)
          return
        }
      }
      if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
        e.preventDefault()
        handleSend()
      }
    },
    [mentionQuery, mentionCandidates, mentionIdx, applyMention, handleSend]
  )

  // ---------- emoji ----------

  const insertEmoji = useCallback((emoji: string) => {
    const el = inputRef.current
    const cursor = el?.selectionStart ?? input.length
    const next = input.slice(0, cursor) + emoji + input.slice(cursor)
    setInput(next)
    setSendError('')
    requestAnimationFrame(() => {
      el?.focus()
      const pos = cursor + emoji.length
      el?.setSelectionRange(pos, pos)
    })
  }, [input])

  // ---------- 图片 / 视频 ----------

  const handlePickImage = useCallback(() => {
    setSendError('')
    imageInputRef.current?.click()
  }, [])

  const handlePickVideo = useCallback(() => {
    setSendError('')
    videoInputRef.current?.click()
  }, [])

  const handleFileChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      e.target.value = '' // 允许重复选择同一文件
      if (!file) return
      if (file.size > 10 * 1024 * 1024) {
        setSendError(t('chat.imageTooLarge'))
        return
      }
      setUploadingImage(true)
      try {
        const imageUrl = await uploadChatImage(file)
        try {
          client?.sendImage(imageUrl)
        } catch {
          setSendError(t('chat.notConnected'))
        }
      } catch (err) {
        const msg = err instanceof Error && err.message ? err.message : ''
        setSendError(msg || t('chat.imageUploadFailed'))
      } finally {
        setUploadingImage(false)
      }
    },
    [t, client]
  )

  const handleVideoChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      e.target.value = ''
      if (!file) return
      if (file.size > 50 * 1024 * 1024) {
        setSendError(t('chat.videoTooLarge'))
        return
      }
      setUploadingVideo(true)
      try {
        const videoUrl = await uploadChatVideo(file)
        try {
          client?.sendVideo(videoUrl)
        } catch {
          setSendError(t('chat.notConnected'))
        }
      } catch (err) {
        const msg = err instanceof Error && err.message ? err.message : ''
        setSendError(msg || t('chat.videoUploadFailed'))
      } finally {
        setUploadingVideo(false)
      }
    },
    [t, client]
  )

  // ---------- 渲染 ----------

  // 把消息文本按 @提及 切分，高亮 @用户名；提及"我"的整条消息加提醒样式
  const renderContent = useCallback(
    (m: ChatMessage): React.ReactNode => {
      if (m.msgType === 1 || m.msgType === 2) {
        // 图片/视频消息：渲染媒体，配文有则显示
        const caption = m.content.trim()
        return (
          <>
            {m.msgType === 1 && m.imageUrl && (
              <img
                className="chat-msg-img"
                src={m.imageUrl}
                alt={caption || t('chat.imageAlt', { name: m.username })}
                loading="lazy"
                onClick={() => setLightbox(m.imageUrl ?? null)}
              />
            )}
            {m.msgType === 2 && m.videoUrl && (
              <video
                className="chat-msg-video"
                src={m.videoUrl}
                controls
                playsInline
                preload="metadata"
              />
            )}
            {caption && <span className="chat-msg-caption">{caption}</span>}
          </>
        )
      }
      const parts = m.content.split(/(@[a-zA-Z0-9_-]+)/g)
      return parts.map((p, i) =>
        p.startsWith('@') && p.length > 1 ? (
          <span key={i} className={`chat-mention ${p === `@${myUsername}` ? 'me' : ''}`}>{p}</span>
        ) : (
          <React.Fragment key={i}>{p}</React.Fragment>
        )
      )
    },
    [myUsername, t]
  )

  if (!user) {
    return (
      <div className="chat-page">
        <div className="chat-login-prompt">
          <div className="chat-login-icon">💬</div>
          <h2>{t('chat.title')}</h2>
          <p>{t('chat.loginPrompt')}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="chat-page">
      <div className="chat-header">
        <h2>💬 {t('chat.title')}</h2>
        <div className="chat-online" title={online.names.join(', ')}>
          <span className={`chat-online-dot ${status === 'connected' ? 'on' : ''}`} aria-hidden="true" />
          {t('chat.onlineCount', { count: online.count })}
        </div>
      </div>

      {status === 'reconnecting' && (
        <div className="chat-reconnect" role="status">{t('chat.reconnecting')}</div>
      )}

      <div className="chat-list" ref={listRef} onScroll={handleScroll}>
        {hasMore && (
          <button type="button" className="chat-load-older" onClick={() => void loadOlder()} disabled={loadingOlder}>
            {loadingOlder ? t('common.loading') : t('chat.loadOlder')}
          </button>
        )}
        {loadingHistory && <div className="chat-empty">{t('common.loading')}</div>}
        {!loadingHistory && messages.length === 0 && (
          <div className="chat-empty">{t('chat.empty')}</div>
        )}
        {messages.map((m) => {
          const mine = m.userId !== undefined && m.userId.toString() === myUserId
          const mentionsMe =
            !mine &&
            myUsername &&
            new RegExp(`@${escapeRegExp(myUsername)}(?![a-zA-Z0-9_-])`).test(m.content)
          return (
            <div key={m.id} className={`chat-msg ${mine ? 'mine' : ''} ${mentionsMe ? 'mentions-me' : ''}`}>
              <div className="chat-msg-head">
                <span className="chat-msg-name">
                  {m.username}
                  {m.isGuest && <span className="chat-guest-badge">{t('chat.guest')}</span>}
                  {mine && <span className="chat-mine-badge">{t('chat.you')}</span>}
                </span>
                <span className="chat-msg-ts">{formatTs(m.ts)}</span>
              </div>
              <div className="chat-msg-content">{renderContent(m)}</div>
            </div>
          )
        })}
      </div>

      {!atBottom && (
        <button
          type="button"
          className="chat-back-bottom"
          aria-label={t('chat.backToBottom')}
          onClick={() => {
            atBottomRef.current = true
            setAtBottom(true)
            listRef.current?.scrollTo({ top: listRef.current.scrollHeight, behavior: 'smooth' })
          }}
        >
          ↓
        </button>
      )}

      <div className="chat-input-bar">
        {sendError && <div className="chat-send-error" role="alert">{sendError}</div>}
        {mentionQuery !== null && mentionCandidates.length > 0 && (
          <div className="chat-mention-pop" role="listbox" aria-label={t('chat.mentionCandidates')}>
            {mentionCandidates.map((name, i) => (
              <button
                key={name}
                type="button"
                role="option"
                aria-selected={i === mentionIdx}
                className={`chat-mention-item ${i === mentionIdx ? 'active' : ''}`}
                onMouseDown={(e) => {
                  e.preventDefault() // 防止输入框失焦
                  applyMention(name)
                }}
                onMouseEnter={() => setMentionIdx(i)}
              >
                @{name}
              </button>
            ))}
          </div>
        )}
        {emojiOpen && (
          <div className="chat-emoji-panel" role="dialog" aria-label={t('chat.emoji')}>
            {EMOJI_GROUPS.map((g) => (
              <div key={g.label} className="chat-emoji-group">
                {g.items.map((e) => (
                  <button
                    key={e}
                    type="button"
                    className="chat-emoji-btn"
                    onClick={() => insertEmoji(e)}
                    aria-label={e}
                  >
                    {e}
                  </button>
                ))}
              </div>
            ))}
          </div>
        )}
        <div className="chat-input-row">
          <button
            type="button"
            className={`chat-icon-btn ${emojiOpen ? 'active' : ''}`}
            onClick={() => setEmojiOpen((v) => !v)}
            aria-label={t('chat.emoji')}
            aria-expanded={emojiOpen}
            title={t('chat.emoji')}
          >
            😊
          </button>
          <button
            type="button"
            className="chat-icon-btn"
            onClick={handlePickImage}
            disabled={uploadingImage || status !== 'connected'}
            aria-label={t('chat.sendImage')}
            title={t('chat.sendImage')}
          >
            {uploadingImage ? <span className="chat-icon-spinner" aria-hidden="true" /> : '🖼️'}
          </button>
          <input
            ref={imageInputRef}
            type="file"
            accept="image/jpeg,image/png,image/webp,image/gif"
            hidden
            onChange={(e) => void handleFileChange(e)}
          />
          <button
            type="button"
            className="chat-icon-btn"
            onClick={handlePickVideo}
            disabled={uploadingVideo || status !== 'connected'}
            aria-label={t('chat.sendVideo')}
            title={t('chat.sendVideo')}
          >
            {uploadingVideo ? <span className="chat-icon-spinner" aria-hidden="true" /> : '📹'}
          </button>
          <input
            ref={videoInputRef}
            type="file"
            accept="video/mp4,video/quicktime,video/webm,video/x-matroska,video/x-msvideo,.mp4,.m4v,.mov,.webm,.mkv,.avi"
            hidden
            onChange={(e) => void handleVideoChange(e)}
          />
          <input
            ref={inputRef}
            className="chat-input"
            value={input}
            maxLength={500}
            placeholder={t('chat.placeholder')}
            onChange={handleInputChange}
            onKeyDown={handleInputKeyDown}
            aria-label={t('chat.title')}
          />
          <button
            type="button"
            className="chat-send-btn"
            onClick={handleSend}
            disabled={!input.trim() || status !== 'connected'}
          >
            {t('chat.send')}
          </button>
        </div>
      </div>

      {lightbox && (
        <div
          ref={lightboxRef}
          className="chat-lightbox"
          role="dialog"
          aria-modal="true"
          aria-label={t('chat.imageAlt', { name: '' })}
          tabIndex={-1}
          onClick={() => setLightbox(null)}
        >
          <img src={lightbox} alt="" />
        </div>
      )}
    </div>
  )
}

export default React.memo(Chat)
