import { createContext, useContext, useState, useEffect, useCallback, useMemo, type ReactNode } from 'react'
import { getUserInfo, login as apiLogin, register as apiRegister, logout as apiLogout, setOnAuthRequired } from '../api'
import type { UserInfo } from '../api/types'

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
  const [user, setUser] = useState<UserInfo | null>(null)
  const [loading, setLoading] = useState(true)
  const [kickedMsg, setKickedMsg] = useState<string | null>(null)

  const clearKickedMsg = useCallback(() => setKickedMsg(null), [])

  const refreshUser = useCallback(async () => {
    try {
      const info = await getUserInfo()
      setUser(info)
      setKickedMsg(null)
    } catch {
      setUser(null)
    }
  }, [])

  useEffect(() => {
    // Try to restore session from httpOnly cookie (sent automatically)
    // If no token in memory, the cookie may still be valid
    refreshUser()
      .catch(() => { /* not authenticated — expected */ })
      .finally(() => setLoading(false))

    setOnAuthRequired((msg?: string) => {
      setUser(null)
      if (msg) setKickedMsg(msg)
    })
    return () => setOnAuthRequired(() => {})
  }, [refreshUser])

  const login = useCallback(async (username: string, password: string) => {
    await apiLogin(username, password)
    await refreshUser()
  }, [refreshUser])

  const register = useCallback(async (username: string, password: string): Promise<string | null> => {
    const res = await apiRegister(username, password)
    if (res.token) {
      await refreshUser()
      return null
    }
    return res.error || '注册成功，请等待管理员审批'
  }, [refreshUser])

  const logout = useCallback(async () => {
    await apiLogout()
    setUser(null)
    setKickedMsg('')
  }, [])

  const value = useMemo(() => ({
    user, loading, kickedMsg, clearKickedMsg, login, register, logout, refreshUser, setUser
  }), [user, loading, kickedMsg, clearKickedMsg, login, register, logout, refreshUser])

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  )
}
