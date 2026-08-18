// 用户偏好设置
//
// 读写全程 try/catch：隐私模式、存储配额满（QuotaExceededError）等
// localStorage 异常一律降级为内存态，绝不让偏好读写导致页面崩溃。
// 写盘失败时保留内存值（本会话生效），刷新后回退到磁盘旧值。

const PREFS_KEY = 'atmos_prefs';

interface Prefs {
  autoPlay: boolean;
  speedMem: boolean;
}

const defaultPrefs: Prefs = { autoPlay: true, speedMem: false };

let cachedPrefs: Prefs | null = null;
window.addEventListener('storage', (e) => {
  if (e.key === PREFS_KEY) cachedPrefs = null;
});

export function loadPrefs(): Prefs {
  if (cachedPrefs) return cachedPrefs;
  const prefs: Prefs = { ...defaultPrefs };
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as unknown;
      // 只接受已知键且类型为 boolean 的值：localStorage 可能被旧版本或外部写入污染，
      // 直接透传会让 getPref/setPref 的运行时值偏离类型声明。
      if (parsed && typeof parsed === 'object') {
        const obj = parsed as Record<string, unknown>;
        for (const key of Object.keys(defaultPrefs) as (keyof Prefs)[]) {
          const v = obj[key];
          if (typeof v === 'boolean') prefs[key] = v;
        }
      }
    }
  } catch { /* localStorage 不可用（隐私模式/沙箱 iframe）：使用默认值 */ }
  cachedPrefs = prefs;
  return prefs;
}

export function getPref<K extends keyof Prefs>(key: K): Prefs[K] {
  return loadPrefs()[key];
}

export function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]) {
  // 值未变化不触发写盘，减少隐私模式下的配额压力
  if (getPref(key) === value) return;
  const prefs = loadPrefs();
  prefs[key] = value;
  cachedPrefs = prefs;
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // 写盘失败（隐私模式/配额满）：内存值本会话生效，刷新后回退磁盘旧值
    console.warn('[prefs] localStorage 写入失败，偏好仅本次会话生效');
  }
}
