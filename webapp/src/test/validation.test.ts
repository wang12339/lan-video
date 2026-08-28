import { describe, it, expect } from 'vitest'
import {
  validateEmail,
  validateUsername,
  validatePassword,
  validatePasswordStrength,
  validateTitle,
  validateDescription,
  validateUrl,
  validateFileSize,
  validateVideoTitle,
  validateTagName,
  sanitizeInput,
  truncate,
  isValidId,
} from '../utils/validation'

describe('validateEmail', () => {
  it('accepts valid email', () => {
    expect(validateEmail('user@example.com').valid).toBe(true)
    expect(validateEmail('test.name+tag@domain.co').valid).toBe(true)
  })

  it('rejects empty email', () => {
    expect(validateEmail('').valid).toBe(false)
    expect(validateEmail('   ').valid).toBe(false)
  })

  it('rejects too long email', () => {
    expect(validateEmail('a'.repeat(255) + '@example.com').valid).toBe(false)
  })

  it('rejects malformed email', () => {
    expect(validateEmail('notanemail').valid).toBe(false)
    expect(validateEmail('@example.com').valid).toBe(false)
    expect(validateEmail('user@').valid).toBe(false)
    expect(validateEmail('user@com').valid).toBe(false)
  })

  it('trims whitespace', () => {
    expect(validateEmail('  user@example.com  ').valid).toBe(true)
  })
})

describe('validateUsername', () => {
  it('accepts valid username', () => {
    expect(validateUsername('abc').valid).toBe(true)
    expect(validateUsername('user_name').valid).toBe(true)
    expect(validateUsername('用户名').valid).toBe(true)
    expect(validateUsername('a1b2c3').valid).toBe(true)
  })

  it('rejects too short username', () => {
    expect(validateUsername('ab').valid).toBe(false)
    expect(validateUsername('a').valid).toBe(false)
  })

  it('rejects too long username', () => {
    expect(validateUsername('a'.repeat(21)).valid).toBe(false)
  })

  it('rejects empty username', () => {
    expect(validateUsername('').valid).toBe(false)
    expect(validateUsername('   ').valid).toBe(false)
  })

  it('rejects invalid characters', () => {
    expect(validateUsername('user name').valid).toBe(false)
    expect(validateUsername('user@name').valid).toBe(false)
    expect(validateUsername('user!name').valid).toBe(false)
  })

  it('accepts boundary lengths', () => {
    expect(validateUsername('abc').valid).toBe(true)
    expect(validateUsername('a'.repeat(20)).valid).toBe(true)
  })
})

describe('validatePassword', () => {
  it('accepts valid password', () => {
    expect(validatePassword('12345678').valid).toBe(true)
    expect(validatePassword('a'.repeat(128)).valid).toBe(true)
  })

  it('rejects empty password', () => {
    expect(validatePassword('').valid).toBe(false)
  })

  it('rejects too short password', () => {
    expect(validatePassword('1234567').valid).toBe(false)
  })

  it('rejects too long password', () => {
    expect(validatePassword('a'.repeat(129)).valid).toBe(false)
  })

  it('accepts boundary lengths', () => {
    expect(validatePassword('12345678').valid).toBe(true)
    expect(validatePassword('a'.repeat(128)).valid).toBe(true)
  })
})

describe('validatePasswordStrength', () => {
  it('accepts password with letters + digits', () => {
    expect(validatePasswordStrength('abc12345').valid).toBe(true)
  })

  it('accepts password with letters + special chars', () => {
    expect(validatePasswordStrength('abc!@#$%').valid).toBe(true)
  })

  it('accepts password with digits + special chars', () => {
    expect(validatePasswordStrength('123!@#$%').valid).toBe(true)
  })

  it('rejects password with only letters', () => {
    expect(validatePasswordStrength('abcdefgh').valid).toBe(false)
  })

  it('accepts password with only digits (score >= 3 threshold)', () => {
    expect(validatePasswordStrength('12345678').valid).toBe(true)
  })

  it('accepts password with only special chars (score >= 3 threshold)', () => {
    expect(validatePasswordStrength('!@#$%^&*').valid).toBe(true)
  })

  it('delegates to validatePassword for length check', () => {
    const result = validatePasswordStrength('short')
    expect(result.valid).toBe(false)
    expect(result.error).toContain('8')
  })

  it('accepts strong password with all categories', () => {
    expect(validatePasswordStrength('MyP@ssw0rd').valid).toBe(true)
  })
})

describe('validateTitle', () => {
  it('accepts valid title', () => {
    expect(validateTitle('My Video').valid).toBe(true)
  })

  it('rejects empty title', () => {
    expect(validateTitle('').valid).toBe(false)
    expect(validateTitle('   ').valid).toBe(false)
  })

  it('rejects too long title', () => {
    expect(validateTitle('a'.repeat(101)).valid).toBe(false)
  })

  it('accepts title at max length', () => {
    expect(validateTitle('a'.repeat(100)).valid).toBe(true)
  })
})

describe('validateDescription', () => {
  it('accepts empty description', () => {
    expect(validateDescription('').valid).toBe(true)
  })

  it('accepts valid description', () => {
    expect(validateDescription('A description').valid).toBe(true)
  })

  it('rejects too long description', () => {
    expect(validateDescription('a'.repeat(2001)).valid).toBe(false)
  })

  it('accepts description at max length', () => {
    expect(validateDescription('a'.repeat(2000)).valid).toBe(true)
  })
})

describe('validateUrl', () => {
  it('accepts valid URLs', () => {
    expect(validateUrl('https://example.com').valid).toBe(true)
    expect(validateUrl('http://localhost:3000').valid).toBe(true)
    expect(validateUrl('https://example.com/path?q=1').valid).toBe(true)
  })

  it('rejects empty URL', () => {
    expect(validateUrl('').valid).toBe(false)
    expect(validateUrl('   ').valid).toBe(false)
  })

  it('rejects invalid URLs', () => {
    expect(validateUrl('not a url').valid).toBe(false)
    expect(validateUrl('ftp://example.com').valid).toBe(false)
    expect(validateUrl('example.com').valid).toBe(false)
  })

  it('trims whitespace', () => {
    expect(validateUrl('  https://example.com  ').valid).toBe(true)
  })
})

describe('validateFileSize', () => {
  it('accepts valid file size', () => {
    expect(validateFileSize(100, 1000).valid).toBe(true)
  })

  it('rejects zero size', () => {
    expect(validateFileSize(0, 1000).valid).toBe(false)
  })

  it('rejects negative size', () => {
    expect(validateFileSize(-1, 1000).valid).toBe(false)
  })

  it('rejects oversized file', () => {
    expect(validateFileSize(1001, 1000).valid).toBe(false)
  })

  it('accepts file at exact max size', () => {
    expect(validateFileSize(1000, 1000).valid).toBe(true)
  })
})

describe('validateVideoTitle', () => {
  it('delegates to validateTitle', () => {
    expect(validateVideoTitle('Valid').valid).toBe(true)
    expect(validateVideoTitle('').valid).toBe(false)
  })
})

describe('validateTagName', () => {
  it('accepts valid tag', () => {
    expect(validateTagName('rust').valid).toBe(true)
  })

  it('rejects empty tag', () => {
    expect(validateTagName('').valid).toBe(false)
    expect(validateTagName('   ').valid).toBe(false)
  })

  it('rejects too long tag', () => {
    expect(validateTagName('a'.repeat(21)).valid).toBe(false)
  })

  it('accepts tag at max length', () => {
    expect(validateTagName('a'.repeat(20)).valid).toBe(true)
  })
})

describe('sanitizeInput', () => {
  it('strips leading and trailing whitespace', () => {
    expect(sanitizeInput('  hello  ')).toBe('hello')
  })

  it('removes angle brackets', () => {
    expect(sanitizeInput('<script>alert(1)</script>')).toBe('scriptalert(1)/script')
  })

  it('returns empty string for whitespace-only input', () => {
    expect(sanitizeInput('   ')).toBe('')
  })

  it('handles empty string', () => {
    expect(sanitizeInput('')).toBe('')
  })

  it('preserves inner content', () => {
    expect(sanitizeInput('hello world')).toBe('hello world')
  })
})

describe('truncate', () => {
  it('returns original string if shorter than max', () => {
    expect(truncate('hello', 10)).toBe('hello')
  })

  it('truncates and adds ellipsis', () => {
    expect(truncate('hello world', 5)).toBe('hell…')
  })

  it('returns exact string at max length', () => {
    expect(truncate('hello', 5)).toBe('hello')
  })

  it('handles empty string', () => {
    expect(truncate('', 5)).toBe('')
  })
})

describe('isValidId', () => {
  it('accepts positive integers', () => {
    expect(isValidId(1)).toBe(true)
    expect(isValidId(42)).toBe(true)
    expect(isValidId(Number.MAX_SAFE_INTEGER)).toBe(true)
  })

  it('rejects zero', () => {
    expect(isValidId(0)).toBe(false)
  })

  it('rejects negative numbers', () => {
    expect(isValidId(-1)).toBe(false)
  })

  it('rejects non-safe integers', () => {
    expect(isValidId(Number.MAX_SAFE_INTEGER + 1)).toBe(false)
  })

  it('accepts positive string numbers', () => {
    expect(isValidId('42')).toBe(true)
    expect(isValidId('1')).toBe(true)
  })

  it('rejects zero string', () => {
    expect(isValidId('0')).toBe(false)
  })

  it('rejects negative string numbers', () => {
    expect(isValidId('-1')).toBe(false)
  })

  it('rejects non-numeric strings', () => {
    expect(isValidId('abc')).toBe(false)
    expect(isValidId('')).toBe(false)
    expect(isValidId('12.5')).toBe(false)
  })
})
