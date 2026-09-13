/// <reference types="vite/client" />

// hls.js 的 light 构建无自带类型声明：类 API 与完整版一致
// （去掉了 EME/字幕/备用音轨等个人平台用不到的特性）。
declare module 'hls.js/light' {
  import Hls from 'hls.js'
  export default Hls
}
