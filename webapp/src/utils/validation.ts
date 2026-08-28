const EMAIL_REGEX = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const USERNAME_REGEX = /^[a-zA-Z0-9_\u4e00-\u9fff]{3,20}$/
const URL_REGEX = /^https?:\/\/[^\s/$.?#].[^\s]*$/i
const LOWER_A = 97
const LOWER_Z = 122
const UPPER_A = 65
const UPPER_Z = 90
const DIGIT_0 = 48
const DIGIT_9 = 57

export interface ValidationResult {
  valid: boolean
  error?: string
}

const OK: ValidationResult = { valid: true }

export function validateEmail(email: string): ValidationResult {
  const trimmed = email.trim()
  if (!trimmed) return { valid: false, error: '邮箱不能为空' }
  if (trimmed.length > 254) return { valid: false, error: '邮箱地址过长' }
  if (!EMAIL_REGEX.test(trimmed)) return { valid: false, error: '邮箱格式无效' }
  return OK
}

export function validateUsername(username: string): ValidationResult {
  const trimmed = username.trim()
  if (!trimmed) return { valid: false, error: '用户名不能为空' }
  const len = trimmed.length
  if (len < 3) return { valid: false, error: '用户名至少3个字符' }
  if (len > 20) return { valid: false, error: '用户名最多20个字符' }
  if (!USERNAME_REGEX.test(trimmed)) return { valid: false, error: '用户名只能包含字母、数字、下划线或中文' }
  return OK
}

export function validatePassword(password: string): ValidationResult {
  if (!password) return { valid: false, error: '密码不能为空' }
  const len = password.length
  if (len < 8) return { valid: false, error: '密码至少8个字符' }
  if (len > 128) return { valid: false, error: '密码最多128个字符' }
  return OK
}

export function validatePasswordStrength(password: string): ValidationResult {
  const base = validatePassword(password)
  if (!base.valid) return base

  let score = 0
  let hasSpecial = false
  const len = password.length
  for (let i = 0; i < len; i++) {
    const c = password.charCodeAt(i)
    if (c >= LOWER_A && c <= LOWER_Z) score |= 1
    else if (c >= UPPER_A && c <= UPPER_Z) score |= 2
    else if (c >= DIGIT_0 && c <= DIGIT_9) score |= 4
    else hasSpecial = true
    if (score === 7) break
  }
  if (hasSpecial) score |= 8

  if (score < 3) return { valid: false, error: '密码需包含字母、数字或特殊字符中的至少两种' }
  return OK
}

export function validateTitle(title: string): ValidationResult {
  const trimmed = title.trim()
  if (!trimmed) return { valid: false, error: '标题不能为空' }
  if (trimmed.length > 100) return { valid: false, error: '标题最多100个字符' }
  return OK
}

export function validateDescription(desc: string): ValidationResult {
  if (desc.length > 2000) return { valid: false, error: '描述最多2000个字符' }
  return OK
}

export function validateUrl(url: string): ValidationResult {
  const trimmed = url.trim()
  if (!trimmed) return { valid: false, error: 'URL不能为空' }
  if (!URL_REGEX.test(trimmed)) return { valid: false, error: 'URL格式无效' }
  return OK
}

export function validateFileSize(bytes: number, maxBytes: number): ValidationResult {
  if (bytes <= 0) return { valid: false, error: '文件大小无效' }
  if (bytes > maxBytes) return { valid: false, error: '文件超过大小限制' }
  return OK
}

export function validateVideoTitle(title: string): ValidationResult {
  return validateTitle(title)
}

export function validateTagName(tag: string): ValidationResult {
  const trimmed = tag.trim()
  if (!trimmed) return { valid: false, error: '标签不能为空' }
  if (trimmed.length > 20) return { valid: false, error: '标签最多20个字符' }
  return OK
}

export function sanitizeInput(input: string): string {
  const len = input.length
  let start = 0
  while (start < len && input.charCodeAt(start) <= 32) start++
  let end = len - 1
  while (end >= start && input.charCodeAt(end) <= 32) end--
  if (start > end) return ''
  const parts: string[] = []
  for (let i = start; i <= end; i++) {
    const c = input.charCodeAt(i)
    if (c === 60 || c === 62) continue
    const ch = input[i]
    if (ch !== undefined) parts.push(ch)
  }
  return parts.join('')
}

export function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str
  return str.slice(0, maxLen - 1) + '…'
}

export function isValidId(id: string | number): boolean {
  if (typeof id === 'number') return id > 0 && Number.isSafeInteger(id)
  if (id.length === 0) return false
  const num = Number(id)
  return Number.isSafeInteger(num) && num > 0
}
