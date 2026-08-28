import { memo } from 'react'
import { useTranslation } from 'react-i18next'

interface SearchBarProps {
  isPending: boolean
  emailVerified: boolean | null
}

function SearchBar({ isPending, emailVerified }: SearchBarProps) {
  const { t } = useTranslation()

  return (
    <>
      {isPending && <div className="home-progress" role="progressbar" aria-label={t('common.loading')} />}
      {emailVerified !== null && (
        <div className={`email-verify-banner ${emailVerified ? 'success' : 'error'}`}>
          {emailVerified ? t('home.emailVerified') : t('home.emailVerifyFailed')}
        </div>
      )}
    </>
  )
}

export default memo(SearchBar)
