import { useFeatureFlags } from '../../hooks/useFeatureFlags'
import './FeatureFlags.css'

export default function FeatureFlags() {
  const { flags, setFlag, resetFlag, resetAll } = useFeatureFlags()

  const handleToggle = (flag: string) => {
    const currentValue = flags[flag]
    if (typeof currentValue === 'boolean') {
      setFlag(flag, !currentValue)
    }
  }

  return (
    <div className="feature-flags" role="region" aria-label="功能开关">
      <div className="feature-flags-header">
        <h3>🚩 功能开关</h3>
        <button className="feature-flags-reset" onClick={resetAll}>
          重置全部
        </button>
      </div>

      <div className="feature-flags-list">
        {Object.entries(flags).map(([flag, value]) => (
          <div key={flag} className="feature-flag-item">
            <div className="feature-flag-info">
              <span className="feature-flag-name">{formatFlagName(flag)}</span>
              <span className="feature-flag-key">{flag}</span>
            </div>
            
            {typeof value === 'boolean' ? (
              <button
                className={`feature-flag-toggle ${value ? 'on' : 'off'}`}
                onClick={() => handleToggle(flag)}
                aria-label={`${value ? '禁用' : '启用'} ${formatFlagName(flag)}`}
              >
                <span className="toggle-thumb" />
              </button>
            ) : (
              <span className="feature-flag-value">
                {typeof value === 'number' ? value.toLocaleString() : String(value)}
              </span>
            )}
            
            <button
              className="feature-flag-reset-btn"
              onClick={() => resetFlag(flag)}
              title="重置为默认值"
            >
              ↺
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}

function formatFlagName(flag: string): string {
  // 将camelCase转换为可读文本
  return flag
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, str => str.toUpperCase())
    .replace(/enable /i, '')
}
