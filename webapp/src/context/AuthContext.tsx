import { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef, type ReactNode } from 'react'
import { getUserInfo, login as apiLogin, register as apiRegister, logout as apiLogout, enterGuestMode, setOnAuthRequired, AuthError, saveToken } from '../api'
import type { UserInfo } from '../api/types'
import { clearPlaybackHistoryCache } from '../api/playback'
import { clearGalleryCache } from '../api/galleryCache'
import { cacheClear } from '../api/client'
import i18n from '../i18n'

interface AuthContextType {
  user: UserInfo | null;
  loading: boolean;
  kickedMsg: string | null;
  clearKickedMsg: () => void;
  login: (username: string, password: string) => Promise<void>;
  loginWithToken: (token: string) => Promise<void>;
  register: (username: string, password: string) => Promise<string | null>;
  logout: () => Promise<boolean>;
  /** 进入访客模式：创建匿名会话并拉取访客用户信息，成功返回 true */
  enterGuest: () => Promise<boolean>;
  refreshUser: () => Promise<void>;
  setUser: (user: UserInfo | null) => void;
}

const AuthContext = createContext<AuthContextType | null>(null)

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) throw new Error('useAuth must be used within AuthProvider')
  return context
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUserState] = useState<UserInfo | null>(null)
  const [loading, setLoading] = useState(true)
  const [kickedMsg, setKickedMsg] = useState<string | null>(null)

  // 镜像 user，供全局 401 回调读取，避免闭包过期值
  const userRef = useRef<UserInfo | null>(null)
  // 会话代数：登录/登出/会话失效时递增，用于丢弃在途请求的过期结果
  const sessionRef = useRef(0)
  // 防止 401 复验请求再次触发回调造成死循环
  const revalidatingRef = useRef(false)

  const setUser = useCallback((u: UserInfo | null) => {
    userRef.current = u
    setUserState(u)
  }, [])

  const clearKickedMsg = useCallback(() => setKickedMsg(null), [])

  const refreshUser = useCallback(async () => {
    const session = sessionRef.current
    try {
      const info = await getUserInfo()
      if (session !== sessionRef.current) return // 已登出/重新登录，丢弃过期结果
      setUser(info)
      setKickedMsg(null)
    } catch (e) {
      if (session !== sessionRef.current) return
      if (e instanceof AuthError) {
        sessionRef.current += 1
        setUser(null)
      }
      // 网络/服务器错误保留现有登录态，避免离线被误登出
    }
  }, [setUser])

  useEffect(() => {
    // 首屏仅恢复登录态；访客模式由访客落地页/AuthDialog 的
    // "以访客身份进入"按钮显式触发（enterGuest）
    void refreshUser().finally(() => setLoading(false))

    setOnAuthRequired((msg?: string) => {
      // 已登出时忽略陈旧 401（如游客的埋点请求），避免误弹"被踢"提示
      if (!userRef.current || revalidatingRef.current) return
      // 401 先以 cookie 复验会话：登出前发出的旧请求返回的 401 不应误杀新会话
      revalidatingRef.current = true
      const session = sessionRef.current
      getUserInfo()
        .then((info) => {
          if (session === sessionRef.current) setUser(info)
        })
        .catch(() => {
          if (session !== sessionRef.current) return
          sessionRef.current += 1
          setUser(null)
          if (msg && msg !== '未登录') setKickedMsg(msg)
        })
        .finally(() => { revalidatingRef.current = false })
    })
    return () => setOnAuthRequired(() => {})
  }, [refreshUser, setUser])

  const login = useCallback(async (username: string, password: string) => {
    await apiLogin(username, password)
    sessionRef.current += 1
    await refreshUser()
  }, [refreshUser])

  // 网关 SSO：后端已换好 token，直接落盘并拉取用户信息
  const loginWithToken = useCallback(async (token: string) => {
    saveToken(token)
    sessionRef.current += 1
    await refreshUser()
  }, [refreshUser])

  // 访客模式：创建匿名影子账号会话（token 由 HttpOnly cookie 持有），
  // 然后拉取访客用户信息。访客与登录用户同权限层，但只能看到/播放
  // 自己上传的内容；之后注册或登录真实账号时自动合并访客内容。
  // 若浏览器已有有效的访客会话则直接复用，不重复创建影子账号。
  const enterGuest = useCallback(async (): Promise<boolean> => {
    sessionRef.current += 1
    const session = sessionRef.current
    try {
      const existing = await getUserInfo()
      if (existing.isGuest) {
        if (sessionRef.current === session) {
          setUser(existing)
          setKickedMsg(null)
          return true
        }
        return false
      }
      // 已是登录用户：不需要进入访客模式
      return false
    } catch {
      // 无有效会话 → 创建新访客
    }
    try {
      await enterGuestMode()
      if (sessionRef.current !== session) return false
      const info = await getUserInfo()
      if (sessionRef.current !== session) return false
      setUser(info)
      setKickedMsg(null)
      return true
    } catch {
      if (sessionRef.current === session) setUser(null)
      return false
    }
  }, [setUser])

  const register = useCallback(async (username: string, password: string): Promise<string | null> => {
    const res = await apiRegister(username, password)
    if (res.token) {
      sessionRef.current += 1
      await refreshUser()
      return null
    }
    return res.error || i18n.t('auth.registerPending')
  }, [refreshUser])

  const logout = useCallback(async () => {
    sessionRef.current += 1 // 先作废在途的 refreshUser 结果
    const serverOk = await apiLogout()
    // 模块级缓存按登录态隔离：登出时主动清空，避免切换账号后串读上一个用户的数据。
    // 服务端登出成功时 request() 已通过 invalidateCacheForPath('/auth/logout') 清空
    // LRU + react-query 两层缓存；失败路径（如 403）不会触发该清理，这里兜底。
    clearPlaybackHistoryCache()
    clearGalleryCache()
    cacheClear()
    setUser(null)
    setKickedMsg(null)
    return serverOk
  }, [setUser])

  const value = useMemo(() => ({
    user, loading, kickedMsg, clearKickedMsg, login, loginWithToken, register, logout, enterGuest, refreshUser, setUser
  }), [user, loading, kickedMsg, clearKickedMsg, login, loginWithToken, register, logout, enterGuest, refreshUser, setUser])

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  )
}
