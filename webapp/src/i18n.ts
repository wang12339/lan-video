import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

import zhCN from './locales/zh-CN.json'
import enUS from './locales/en-US.json'

// localStorage 在隐私模式/沙箱 iframe 下可能直接抛错，
// 模块加载期异常会导致白屏，因此读取必须降级：异常时回退默认语言。
function getInitialLanguage(): string {
  try {
    return localStorage.getItem('atmos.lang') || 'zh-CN'
  } catch {
    return 'zh-CN'
  }
}

i18n
  .use(initReactI18next)
  .init({
    resources: {
      'zh-CN': { translation: zhCN },
      'en-US': { translation: enUS },
    },
    lng: getInitialLanguage(),
    fallbackLng: 'zh-CN',
    interpolation: { escapeValue: false },
  })

export default i18n
