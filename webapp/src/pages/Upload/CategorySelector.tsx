import { memo } from 'react'
import { useTranslation } from 'react-i18next'
import type { Category } from '../../config/categories'

interface Props {
  category: string
  setCategory: (key: string) => void
  disabled: boolean
  categories: Category[]
}

function CategorySelector({ category, setCategory, disabled, categories }: Props) {
  const { t } = useTranslation()

  return (
    <div className="upload-cats">
      <span className="upload-cats-label">{t('upload.category')}</span>
      {categories.map((cat) => (
        <button
          key={cat.key}
          className={`cat-dot ${category === cat.key ? 'active' : ''}`}
          onClick={() => setCategory(cat.key)}
          disabled={disabled}
        >
          <span className="dot" style={{ background: cat.color }} />
          {t(cat.i18nKey)}
        </button>
      ))}
    </div>
  )
}

export default memo(CategorySelector)
