import { memo } from 'react'
import { useTranslation } from 'react-i18next'
import { LEVELS } from '../utils/logFormatter'

interface LogFiltersProps {
  level: string
  onLevelChange: (level: string) => void
  searchInput: string
  onSearchInputChange: (value: string) => void
  search: string
  onSearchChange: (value: string) => void
  timeFrom: string
  onTimeFromChange: (value: string) => void
  timeTo: string
  onTimeToChange: (value: string) => void
}

function LogFilters({
  level,
  onLevelChange,
  searchInput,
  onSearchInputChange,
  search,
  onSearchChange,
  timeFrom,
  onTimeFromChange,
  timeTo,
  onTimeToChange,
}: LogFiltersProps) {
  const { t } = useTranslation()

  return (
    <div className="a-filter-bar">
      <span className="a-filter-label">{t('admin.logs.level')}</span>
      <select
        className="a-filter"
        value={level}
        onChange={e => onLevelChange(e.target.value)}
        aria-label={t('admin.logs.levelAria')}
      >
        <option value="">{t('admin.logs.all')}</option>
        {LEVELS.map(l => <option key={l} value={l}>{l}</option>)}
      </select>
      <div className="a-search">
        <input
          type="search"
          value={searchInput}
          onChange={e => onSearchInputChange(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && onSearchChange(searchInput.trim())}
          placeholder={t('admin.logs.searchPlaceholder')}
          aria-label={t('admin.logs.searchAria')}
        />
        <button onClick={() => onSearchChange(searchInput.trim())}>{t('common.search')}</button>
        {search && (
          <button onClick={() => { onSearchChange(''); onSearchInputChange('') }} title={t('admin.media.clearSearch')}>
            ×
          </button>
        )}
      </div>
      <span className="a-filter-label">{t('admin.logs.from')}</span>
      <input
        type="datetime-local"
        className="a-filter"
        value={timeFrom}
        onChange={e => onTimeFromChange(e.target.value)}
        aria-label={t('admin.logs.startTimeAria')}
      />
      <span className="a-filter-label">{t('admin.logs.to')}</span>
      <input
        type="datetime-local"
        className="a-filter"
        value={timeTo}
        onChange={e => onTimeToChange(e.target.value)}
        aria-label={t('admin.logs.endTimeAria')}
      />
    </div>
  )
}

export default memo(LogFilters)
