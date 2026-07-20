// 用户偏好设置

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
  let prefs: Prefs = { ...defaultPrefs };
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (raw) {
      prefs = { ...defaultPrefs, ...JSON.parse(raw) };
    }
  } catch { /* ignore */ }
  cachedPrefs = prefs;
  return prefs;
}

export function getPref<K extends keyof Prefs>(key: K): Prefs[K] {
  return loadPrefs()[key];
}

export function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]) {
  const prefs = loadPrefs();
  prefs[key] = value;
  cachedPrefs = prefs;
  try { localStorage.setItem(PREFS_KEY, JSON.stringify(prefs)); } catch { /* noop */ }
}
