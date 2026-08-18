import { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef, type ReactNode } from 'react'
import { getUserInfo, login as apiLogin, register as apiRegister, logout as apiLogout, setOnAuthRequired, AuthError } from '../api'
import type { UserInfo } from '../api/types'
import i18n from '../i18n'

interface AuthContextType {
  user: UserInfo | null;
  loading: boolean;
  kickedMsg: string | null;
  clearKickedMsg: () => void;
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<string | null>;
  logout: () => Promise<void>;
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
    // 首屏用 httpOnly cookie 恢复登录态（内存无 token 时 cookie 可能仍有效）
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
    await apiLogout()
    setUser(null)
    setKickedMsg(null)
  }, [setUser])

  const value = useMemo(() => ({
    user, loading, kickedMsg, clearKickedMsg, login, register, logout, refreshUser, setUser
  }), [user, loading, kickedMsg, clearKickedMsg, login, register, logout, refreshUser, setUser])

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  )
}
