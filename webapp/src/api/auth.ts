// 认证 API

import { request, saveToken, clearToken, APIError } from './client';
import type { AuthResponse, UserInfo, UserProfile } from './types';

/**
 * 登录/注册接口失败时返回 HTTP 200 + { ok: false, error }，
 * 统一在此转成 APIError，与 request() 的非 2xx 抛错保持类型一致。
 * @param res - 登录/注册接口返回的响应体
 * @throws {APIError} 当响应体中 ok 为 false 时抛出错误
 * @returns 原始响应体（ok 为 true 时）
 */
function handleAuthResponse(res: AuthResponse): AuthResponse {
  if (res && res.ok === false) throw new APIError(res.error || '操作失败', 200);
  return res;
}

/**
 * 登录/注册成功后持久化 token
 * @param res - 登录/注册接口返回的响应体
 * @returns 原始响应体
 */
function persistToken(res: AuthResponse): AuthResponse {
  if (res.token) saveToken(res.token);
  return res;
}

/**
 * 用户登录
 * @param username - 用户名（2-64 字符）
 * @param password - 密码（6-128 字符）
 * @returns 包含 token 和用户信息的认证响应
 * @throws {APIError} 用户名或密码错误时抛出
 */
export async function login(username: string, password: string): Promise<AuthResponse> {
  const res = handleAuthResponse(await request<AuthResponse>('/auth/login', {
    method: 'POST',
    body: { username, password },
    auth: false,
  }));
  return persistToken(res);
}

/**
 * 用户注册
 * @param username - 用户名（2-64 字符）
 * @param password - 密码（6-128 字符）
 * @returns 包含 token 和用户信息的认证响应
 * @throws {APIError} 用户名已存在或注册被禁用时抛出
 */
export async function register(username: string, password: string): Promise<AuthResponse> {
  const res = handleAuthResponse(await request<AuthResponse>('/auth/register', {
    method: 'POST',
    body: { username, password },
    auth: false,
  }));
  return persistToken(res);
}

/**
 * 用户登出
 * 清除本地 token 和会话缓存，即使服务端请求失败也会执行本地清理
 */
export async function logout(): Promise<void> {
  try { await request('/auth/logout', { method: 'POST' }); } catch { /* ignore */ }
  // 清除会话缓存，避免登出后 60 秒内 checkSession() 仍返回旧结果
  resetSessionCache();
  clearToken();
}

/**
 * 获取当前登录用户的基本信息
 * @returns 用户信息（用户名、邮箱、头像 URL 等）
 * @throws {APIError} 未登录或 token 过期时抛出 401
 */
export async function getUserInfo(): Promise<UserInfo> {
  return request<UserInfo>('/auth/user', { silent: true });
}

/**
 * 获取当前登录用户的完整个人资料
 * @returns 用户个人资料（含详细设置和统计信息）
 * @throws {APIError} 未登录或 token 过期时抛出 401
 */
export async function getUserProfile(): Promise<UserProfile> {
  return request<UserProfile>('/auth/user/profile');
}

/** 会话缓存：避免频繁请求 /auth/user 接口 */
let sessionCache: { valid: boolean; expires: number } | null = null;

/** 重置会话缓存（登出或 token 变更时调用） */
function resetSessionCache() {
  sessionCache = null;
}

/**
 * 检查当前会话是否有效
 * 内部使用缓存：成功结果缓存 60 秒，失败结果缓存 5 秒，避免频繁请求
 * @returns true 表示会话有效（已登录），false 表示会话无效（未登录或 token 过期）
 */
export async function checkSession(): Promise<boolean> {
  if (sessionCache && Date.now() < sessionCache.expires) return sessionCache.valid;
  try { await request('/auth/user', { auth: true }); sessionCache = { valid: true, expires: Date.now() + 60000 }; return true; }
  catch { sessionCache = { valid: false, expires: Date.now() + 5000 }; return false; }
}

/**
 * 上传用户头像
 * @param file - 头像图片文件（支持 JPEG、PNG、WebP 等格式）
 * @returns 头像的 URL 地址，上传失败时返回空字符串
 * @remarks 使用 silent 模式，错误由调用方（个人中心页面）自行处理，不弹全局 Toast
 */
export async function uploadAvatar(file: File): Promise<string> {
  const formData = new FormData();
  formData.append('file', file);
  // 统一走 request()（multipart）：超时、错误本地化、401 处理一致；
  // silent：错误由个人中心页面自行展示，不重复弹全局 Toast
  const data = await request<{ avatarUrl?: string }>('/auth/user/avatar', {
    method: 'POST',
    body: formData,
    silent: true,
  });
  return data.avatarUrl || '';
}

/**
 * 发送邮箱验证邮件
 * @returns 操作结果，包含是否成功和提示消息
 * @throws {APIError} 未登录或发送失败时抛出
 */
export async function sendVerificationEmail(): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/send-verification-email', {
    method: 'POST',
  });
}

/**
 * 更新用户邮箱地址
 * @param email - 新邮箱地址
 * @returns 操作结果，包含是否成功和提示消息
 * @throws {APIError} 邮箱格式无效或已被占用时抛出
 */
export async function updateEmail(email: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/user/email', {
    method: 'PUT',
    body: { email },
  });
}

/**
 * 发送密码重置邮件
 * @param email - 用户注册时使用的邮箱地址
 * @returns 操作结果，包含是否成功和提示消息
 * @remarks 无论邮箱是否存在，均返回成功（防止邮箱枚举攻击）
 */
export async function forgotPassword(email: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/forgot-password', {
    method: 'POST',
    body: { email },
    auth: false,
  });
}

/**
 * 重置密码
 * @param token - 密码重置令牌（从邮件链接中获取）
 * @param password - 新密码（6-128 字符）
 * @returns 操作结果，包含是否成功和提示消息
 * @throws {APIError} 令牌无效或过期时抛出
 */
export async function resetPassword(token: string, password: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/reset-password', {
    method: 'POST',
    body: { token, password },
    auth: false,
  });
}

/**
 * 验证邮箱地址
 * 提交邮箱验证令牌完成验证（邮件链接走 GET 渲染落地页，API 场景用 POST）
 * @param token - 邮箱验证令牌（从验证邮件链接中获取）
 * @returns 操作结果，包含是否成功和提示消息
 * @throws {APIError} 令牌无效或过期时抛出
 */
export async function verifyEmail(token: string): Promise<{ ok: boolean; message: string }> {
  return request<{ ok: boolean; message: string }>('/auth/verify-email', {
    method: 'POST',
    body: { token },
    auth: false,
  });
}


