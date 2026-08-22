import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import AuthDialog from '../components/AuthDialog/AuthDialog'
import i18n from '../i18n'

// ── Mocks ──────────────────────────────────────────────────────────────────────

const mockLogin = vi.fn()
const mockRegister = vi.fn()
const mockClearKickedMsg = vi.fn()

vi.mock('../context/AuthContext', () => ({
  useAuth: () => ({
    user: null,
    loading: false,
    kickedMsg: null,
    clearKickedMsg: mockClearKickedMsg,
    login: mockLogin,
    register: mockRegister,
    logout: vi.fn(),
    refreshUser: vi.fn(),
    setUser: vi.fn(),
  }),
}))

vi.mock('../api', () => ({
  forgotPassword: vi.fn(),
  resetPassword: vi.fn(),
}))

vi.mock('../api/auth', () => ({
  verifyEmail: vi.fn(),
}))

// ── Helpers ────────────────────────────────────────────────────────────────────

const t = (key: string) => i18n.t(key)

function renderDialog(
  props: Partial<React.ComponentProps<typeof AuthDialog>> = {},
  initialEntries: string[] = ['/'],
) {
  const onClose = vi.fn()
  const utils = render(
    <MemoryRouter initialEntries={initialEntries}>
      <AuthDialog onClose={onClose} closable={true} {...props} />
    </MemoryRouter>,
  )
  return { ...utils, onClose }
}

function getInputByLabel(labelText: string): HTMLInputElement {
  const label = screen.getByText(labelText).closest('label')!
  return label.querySelector('input') as HTMLInputElement
}

/** 按 dialog 内的 h2 标题断言（避免与 tab/button 同名文本冲突） */
function getDialogTitle(): HTMLElement {
  return document.querySelector('#auth-dialog-title') as HTMLElement
}

// ── Tests ──────────────────────────────────────────────────────────────────────

describe('AuthDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    i18n.changeLanguage('zh-CN')
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  // =========================================================================
  // 1) 登录表单
  // =========================================================================
  describe('登录表单', () => {
    it('默认渲染登录标题和表单字段', () => {
      renderDialog()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
      expect(getDialogTitle()).toHaveTextContent(t('auth.login'))
      expect(screen.getByLabelText(t('auth.username'))).toBeInTheDocument()
      expect(screen.getByLabelText(t('auth.password'))).toBeInTheDocument()
    })

    it('提交时调用 login 并关闭对话框', async () => {
      mockLogin.mockResolvedValue(undefined)
      const { onClose } = renderDialog()

      const usernameInput = getInputByLabel(t('auth.username'))
      const passwordInput = getInputByLabel(t('auth.password'))

      fireEvent.change(usernameInput, { target: { value: 'testuser' } })
      fireEvent.change(passwordInput, { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(mockLogin).toHaveBeenCalledWith('testuser', 'Test@1234')
      })
      expect(onClose).toHaveBeenCalled()
    })

    it('登录失败时显示错误信息', async () => {
      mockLogin.mockRejectedValue(new Error('用户名或密码错误'))
      renderDialog()

      const usernameInput = getInputByLabel(t('auth.username'))
      const passwordInput = getInputByLabel(t('auth.password'))

      fireEvent.change(usernameInput, { target: { value: 'testuser' } })
      fireEvent.change(passwordInput, { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByText('用户名或密码错误')).toBeInTheDocument()
      })
    })

    it('加载中时显示"处理中..."并禁用提交按钮', async () => {
      let resolveLogin!: () => void
      mockLogin.mockImplementation(() => new Promise<void>(r => { resolveLogin = r }))
      renderDialog()

      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'testuser' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByRole('button', { name: t('auth.submitting') })).toBeDisabled()
      })

      await act(async () => { resolveLogin() })
    })

    it('登录模式下显示"忘记密码"链接', () => {
      renderDialog()
      expect(screen.getByText(t('auth.forgotLink'))).toBeInTheDocument()
    })

    it('点击密码切换按钮可显示/隐藏密码', () => {
      renderDialog()
      const toggleBtn = screen.getByRole('button', { name: t('auth.showPassword') })
      const passwordInput = getInputByLabel(t('auth.password'))

      expect(passwordInput).toHaveAttribute('type', 'password')

      fireEvent.click(toggleBtn)
      expect(passwordInput).toHaveAttribute('type', 'text')
      expect(screen.getByRole('button', { name: t('auth.hidePassword') })).toBeInTheDocument()

      fireEvent.click(screen.getByRole('button', { name: t('auth.hidePassword') }))
      expect(passwordInput).toHaveAttribute('type', 'password')
    })
  })

  // =========================================================================
  // 2) 注册表单
  // =========================================================================
  describe('注册表单', () => {
    function switchToRegister() {
      fireEvent.click(screen.getByRole('tab', { name: t('auth.register') }))
    }

    it('切换到注册模式显示注册标题和表单字段', () => {
      renderDialog()
      switchToRegister()

      expect(getDialogTitle()).toHaveTextContent(t('auth.register'))
      expect(screen.getByLabelText(t('auth.username'))).toBeInTheDocument()
      expect(screen.getByLabelText(t('auth.password'))).toBeInTheDocument()
    })

    it('提交时调用 register 并关闭对话框', async () => {
      mockRegister.mockResolvedValue(null) // null = 成功直接登录
      const { onClose } = renderDialog()
      switchToRegister()

      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'newuser' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'NewPass@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(mockRegister).toHaveBeenCalledWith('newuser', 'NewPass@1234')
      })
      expect(onClose).toHaveBeenCalled()
    })

    it('注册成功但需要审批时显示提示消息', async () => {
      mockRegister.mockResolvedValue('注册成功，请等待管理员审批')
      renderDialog()
      switchToRegister()

      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'newuser' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'NewPass@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByText('注册成功，请等待管理员审批')).toBeInTheDocument()
      })
    })

    it('注册模式下没有"忘记密码"链接', () => {
      renderDialog()
      switchToRegister()
      expect(screen.queryByText(t('auth.forgotLink'))).not.toBeInTheDocument()
    })

    it('注册模式下密码输入框使用 autoComplete=new-password', () => {
      renderDialog()
      switchToRegister()
      const passwordInput = getInputByLabel(t('auth.password'))
      expect(passwordInput).toHaveAttribute('autocomplete', 'new-password')
    })
  })

  // =========================================================================
  // 3) 表单验证
  // =========================================================================
  describe('表单验证', () => {
    describe('登录验证', () => {
      it('用户名为空时显示必填提示', async () => {
        renderDialog()
        fireEvent.blur(getInputByLabel(t('auth.username')))
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.usernameRequired'))).toBeInTheDocument()
        })
      })

      it('密码为空时显示必填提示', async () => {
        renderDialog()
        fireEvent.blur(getInputByLabel(t('auth.password')))
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.passwordRequired'))).toBeInTheDocument()
        })
      })
    })

    describe('注册验证', () => {
      function switchToRegister() {
        fireEvent.click(screen.getByRole('tab', { name: t('auth.register') }))
      }

      it('用户名太短时显示长度提示', async () => {
        renderDialog()
        switchToRegister()
        fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'a' } })
        fireEvent.blur(getInputByLabel(t('auth.username')))
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.usernameLength'))).toBeInTheDocument()
        })
      })

      it('用户名包含控制字符时显示非法字符提示', async () => {
        renderDialog()
        switchToRegister()
        const input = getInputByLabel(t('auth.username'))
        // 使用 null 控制字符（\0），在 jsdom 中可保留在 value 里
        const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype, 'value'
        )!.set!
        nativeInputValueSetter.call(input, 'user\x00name')
        fireEvent.input(input)
        fireEvent.blur(input)
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.usernameIllegal'))).toBeInTheDocument()
        })
      })

      it('密码太短时显示长度提示', async () => {
        renderDialog()
        switchToRegister()
        fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Ab1!' } })
        fireEvent.blur(getInputByLabel(t('auth.password')))
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.passwordLength'))).toBeInTheDocument()
        })
      })

      it('密码强度不足时显示强度提示', async () => {
        renderDialog()
        switchToRegister()
        // 8 位但只包含两种字符类别（小写+数字）
        fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'abcdefgh1' } })
        fireEvent.blur(getInputByLabel(t('auth.password')))
        await waitFor(() => {
          expect(screen.getByText(t('auth.validation.passwordStrength'))).toBeInTheDocument()
        })
      })

      it('强密码通过验证（<12 位需至少 3 类字符）', async () => {
        renderDialog()
        switchToRegister()
        fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'validuser' } })
        fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Abc123!@' } })
        fireEvent.blur(getInputByLabel(t('auth.password')))
        await waitFor(() => {
          expect(screen.queryByText(t('auth.validation.passwordStrength'))).not.toBeInTheDocument()
        })
      })

      it('>=12 位密码只需 2 类字符即可通过', async () => {
        renderDialog()
        switchToRegister()
        fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'validuser' } })
        fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'abcdefghij12' } })
        fireEvent.blur(getInputByLabel(t('auth.password')))
        await waitFor(() => {
          expect(screen.queryByText(t('auth.validation.passwordStrength'))).not.toBeInTheDocument()
        })
      })
    })
  })

  // =========================================================================
  // 4) 错误提示
  // =========================================================================
  describe('错误提示', () => {
    it('API 返回错误时显示全局错误信息', async () => {
      mockLogin.mockRejectedValue(new Error('账号已被禁用'))
      renderDialog()

      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'banned' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByRole('alert')).toHaveTextContent('账号已被禁用')
      })
    })

    it('非 Error 类型异常时显示默认错误信息', async () => {
      mockLogin.mockRejectedValue('unexpected error')
      renderDialog()

      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'test' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByText(t('auth.error'))).toBeInTheDocument()
      })
    })

    it('表单验证失败时聚焦第一个错误字段', async () => {
      renderDialog()
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))

      await waitFor(() => {
        expect(screen.getByText(t('auth.validation.usernameRequired'))).toBeInTheDocument()
      })
      // 第一个错误字段应被聚焦
      expect(getInputByLabel(t('auth.username'))).toHaveFocus()
    })

    it('字段错误使用 role="alert" 以支持屏幕阅读器', async () => {
      renderDialog()
      fireEvent.blur(getInputByLabel(t('auth.username')))
      await waitFor(() => {
        const errorEl = screen.getByText(t('auth.validation.usernameRequired'))
        expect(errorEl).toHaveAttribute('role', 'alert')
      })
    })
  })

  // =========================================================================
  // 5) 切换模式
  // =========================================================================
  describe('切换模式', () => {
    it('点击登录/注册标签切换表单', () => {
      renderDialog()

      // 默认登录模式
      expect(screen.getByRole('tab', { name: t('auth.login') })).toHaveAttribute('aria-selected', 'true')
      expect(screen.getByRole('tab', { name: t('auth.register') })).toHaveAttribute('aria-selected', 'false')
      expect(getDialogTitle()).toHaveTextContent(t('auth.login'))

      // 切换到注册
      fireEvent.click(screen.getByRole('tab', { name: t('auth.register') }))
      expect(screen.getByRole('tab', { name: t('auth.login') })).toHaveAttribute('aria-selected', 'false')
      expect(screen.getByRole('tab', { name: t('auth.register') })).toHaveAttribute('aria-selected', 'true')
      expect(getDialogTitle()).toHaveTextContent(t('auth.register'))

      // 切换回登录
      fireEvent.click(screen.getByRole('tab', { name: t('auth.login') }))
      expect(screen.getByRole('tab', { name: t('auth.login') })).toHaveAttribute('aria-selected', 'true')
      expect(getDialogTitle()).toHaveTextContent(t('auth.login'))
    })

    it('切换模式时清除错误和成功消息', async () => {
      mockLogin.mockRejectedValue(new Error('登录失败'))
      renderDialog()

      // 触发登录错误
      fireEvent.change(getInputByLabel(t('auth.username')), { target: { value: 'test' } })
      fireEvent.change(getInputByLabel(t('auth.password')), { target: { value: 'Test@1234' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.submit') }))
      await waitFor(() => {
        expect(screen.getByText('登录失败')).toBeInTheDocument()
      })

      // 切换到注册模式后错误应消失
      fireEvent.click(screen.getByRole('tab', { name: t('auth.register') }))
      expect(screen.queryByText('登录失败')).not.toBeInTheDocument()
    })

    it('切换模式时清除已触摸的字段状态', async () => {
      renderDialog()

      // 触发登录验证错误
      fireEvent.blur(getInputByLabel(t('auth.username')))
      await waitFor(() => {
        expect(screen.getByText(t('auth.validation.usernameRequired'))).toBeInTheDocument()
      })

      // 切换到注册模式后验证错误应消失
      fireEvent.click(screen.getByRole('tab', { name: t('auth.register') }))
      expect(screen.queryByText(t('auth.validation.usernameRequired'))).not.toBeInTheDocument()
    })

    it('从登录页点击"忘记密码"切换到 forgot 模式', () => {
      renderDialog()
      fireEvent.click(screen.getByText(t('auth.forgotLink')))

      expect(getDialogTitle()).toHaveTextContent(t('auth.forgotTitle'))
      expect(screen.getByLabelText(t('auth.email'))).toBeInTheDocument()
      expect(screen.queryByLabelText(t('auth.username'))).not.toBeInTheDocument()
    })

    it('forgot 模式下点击"返回登录"切回 login', () => {
      renderDialog()
      fireEvent.click(screen.getByText(t('auth.forgotLink')))
      fireEvent.click(screen.getByText(t('auth.backToLogin')))

      expect(getDialogTitle()).toHaveTextContent(t('auth.login'))
      expect(screen.getByLabelText(t('auth.username'))).toBeInTheDocument()
    })

    it('reset 模式下点击"返回登录"切回 login 并清空密码', () => {
      renderDialog({}, ['/?reset_token=abc123'])
      // 默认进入 reset 模式
      expect(getDialogTitle()).toHaveTextContent(t('auth.resetTitle'))

      fireEvent.change(getInputByLabel(t('auth.resetNewPassword')), { target: { value: 'OldPass@123' } })
      fireEvent.click(screen.getByText(t('auth.backToLogin')))

      expect(getDialogTitle()).toHaveTextContent(t('auth.login'))
      // 密码应被清空
      const loginPasswordInput = getInputByLabel(t('auth.password'))
      expect(loginPasswordInput).toHaveValue('')
    })

    it('点击 overlay 背景关闭对话框（closable=true）', () => {
      const { onClose, container } = renderDialog()
      const overlay = container.querySelector('.auth-overlay')!
      fireEvent.click(overlay)
      expect(onClose).toHaveBeenCalled()
    })

    it('closable=false 时不显示关闭按钮', () => {
      render(
        <MemoryRouter>
          <AuthDialog closable={false} />
        </MemoryRouter>,
      )
      expect(screen.queryByRole('button', { name: t('auth.closeDialog') })).not.toBeInTheDocument()
    })

    it('按 Escape 键关闭对话框', () => {
      const { onClose } = renderDialog()
      // 组件在 document 上监听 keydown
      fireEvent.keyDown(document, { key: 'Escape' })
      expect(onClose).toHaveBeenCalled()
    })

    it('dialog 具有正确的 ARIA 属性', () => {
      renderDialog()
      const dialog = screen.getByRole('dialog')
      expect(dialog).toHaveAttribute('aria-modal', 'true')
      expect(dialog).toHaveAttribute('aria-labelledby', 'auth-dialog-title')
    })

    it('forgot 模式下提交邮箱验证', async () => {
      const { forgotPassword } = await import('../api')
      vi.mocked(forgotPassword).mockResolvedValue({ ok: true, message: '重置邮件已发送' })

      renderDialog()
      fireEvent.click(screen.getByText(t('auth.forgotLink')))

      fireEvent.change(getInputByLabel(t('auth.email')), { target: { value: 'user@example.com' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.forgotSubmit') }))

      await waitFor(() => {
        expect(forgotPassword).toHaveBeenCalledWith('user@example.com')
      })
      await waitFor(() => {
        expect(screen.getByText('重置邮件已发送')).toBeInTheDocument()
      })
    })

    it('reset 模式下提交重置密码', async () => {
      const { resetPassword } = await import('../api')
      vi.mocked(resetPassword).mockResolvedValue({ ok: true, message: '密码已重置' })

      renderDialog({}, ['/?reset_token=validtoken123'])
      expect(getDialogTitle()).toHaveTextContent(t('auth.resetTitle'))

      fireEvent.change(getInputByLabel(t('auth.resetNewPassword')), { target: { value: 'NewPass@123' } })
      fireEvent.click(screen.getByRole('button', { name: t('auth.resetSubmit') }))

      await waitFor(() => {
        expect(resetPassword).toHaveBeenCalledWith('validtoken123', 'NewPass@123')
      })
      await waitFor(() => {
        expect(screen.getByText('密码已重置')).toBeInTheDocument()
      })
    })
  })
})
