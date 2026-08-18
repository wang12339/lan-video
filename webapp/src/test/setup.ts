import '@testing-library/jest-dom'
import i18n from '../i18n'

// 测试环境固定为 zh-CN：组件内 t() 渲染中文，与各用例的中文断言一致
localStorage.removeItem('atmos.lang')
await i18n.changeLanguage('zh-CN')
