import { useEffect } from 'react'

/**
 * 弹窗/抽屉打开时锁定页面滚动（移动端防滚动穿透）。
 *
 * 同锁 body 与 documentElement：安卓 WebView/夸克等浏览器 touchmove
 * 会穿透到 documentElement/整页，只锁 body 不够（AuthDialog 同款做法）。
 * 关闭时恢复打开前的原始值。
 *
 * @param locked 是否锁定（通常传弹窗的 open 状态）
 */
export function useScrollLock(locked: boolean) {
  useEffect(() => {
    if (!locked) return
    const prevBody = document.body.style.overflow
    const prevHtml = document.documentElement.style.overflow
    document.body.style.overflow = 'hidden'
    document.documentElement.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = prevBody
      document.documentElement.style.overflow = prevHtml
    }
  }, [locked])
}
