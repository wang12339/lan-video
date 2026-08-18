import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { health } from '../../api'
import { getSystemInfo, scanMedia, backfillThumbnails, getRegistrationEnabled, setRegistrationEnabled } from '../../api/admin'
import type { SystemInfo } from '../../api/admin'
import { AlertDialog, SkeletonLoader } from '../../components/ui'

const POLL_MS = 30_000

export default function SystemTab() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [scanResult, setScanResult] = useState('')
  const [scanError, setScanError] = useState(false)
  const [scanning, setScanning] = useState(false)
  const [backfillResult, setBackfillResult] = useState('')
  const [backfillError, setBackfillError] = useState(false)
  const [backfilling, setBackfilling] = useState(false)
  const [alertMsg, setAlertMsg] = useState('')

  const { data: serverOk, isLoading: healthLoading } = useQuery({
    queryKey: ['health'],
    queryFn: () => health(),
    refetchInterval: POLL_MS,
  })
  const { data: sys, isLoading: sysLoading } = useQuery<SystemInfo>({
    queryKey: ['admin-system-info'],
    queryFn: getSystemInfo,
    refetchInterval: POLL_MS,
  })
  const { data: regData } = useQuery({
    queryKey: ['admin-registration-enabled'],
    queryFn: getRegistrationEnabled,
  })

  const toggleRegistrationMut = useMutation({
    mutationFn: setRegistrationEnabled,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-registration-enabled'] })
    },
    onError: () => setAlertMsg(t('admin.system.operationFailed')),
  })

  const regEnabled = regData?.enabled

  const handleToggleRegistration = () => {
    if (regEnabled === undefined || toggleRegistrationMut.isPending) return
    toggleRegistrationMut.mutate(!regEnabled)
  }

  const handleScan = async () => {
    if (scanning) return
    setScanning(true)
    setScanResult('')
    setScanError(false)
    try {
      const res = await scanMedia()
      setScanResult(t('admin.system.scanComplete', { added: res.added }))
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] })
    } catch (e) {
      setScanError(true)
      setScanResult(e instanceof Error ? e.message : t('admin.system.scanFailed'))
    } finally {
      setScanning(false)
    }
  }

  const handleBackfill = async () => {
    if (backfilling) return
    setBackfilling(true)
    setBackfillResult('')
    setBackfillError(false)
    try {
      const res = await backfillThumbnails()
      if (res.ok) {
        setBackfillResult(t('admin.system.backfillComplete', { generated: res.generated }))
      } else {
        setBackfillError(true)
        setBackfillResult(t('admin.system.backfillFailed'))
      }
    } catch (e) {
      setBackfillError(true)
      setBackfillResult(e instanceof Error ? e.message : t('admin.system.backfillFailed'))
    } finally {
      setBackfilling(false)
    }
  }

  if (sysLoading) return <SkeletonLoader type="card" lines={4} />

  return (
    <div className="admin-tab-content">
      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.system.status')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.serverStatus')}</span>
            <span className={`admin-info-value ${serverOk ? 'ok' : 'fail'}`}>
              {healthLoading && serverOk === undefined ? '...' : serverOk ? t('admin.system.serverOk') : t('admin.system.serverFail')}
            </span>
          </div>
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.dbConnections')}</span>
            <span className="admin-info-value">{sys?.dbConnections ?? '--'}</span>
          </div>
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.stats.storage')}</span>
            <span className="admin-info-value">{sys?.mediaSizeHuman || '--'}</span>
          </div>
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.mediaRoot')}</span>
            <span className="admin-info-value">{sys?.mediaRoot || '--'}</span>
          </div>
        </div>
      </div>

      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.system.registration')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.registrationToggle')}</span>
            <span className="admin-toggle-wrap">
              <button
                type="button"
                role="switch"
                aria-checked={regEnabled === true}
                aria-label={t('admin.system.registrationToggle')}
                className={`admin-toggle ${regEnabled ? 'on' : ''}`}
                disabled={regEnabled === undefined || toggleRegistrationMut.isPending}
                onClick={handleToggleRegistration}
              >
                <span className="admin-toggle-thumb" />
              </button>
              <span className="admin-toggle-text">
                {toggleRegistrationMut.isPending ? '...' : regEnabled === undefined ? '--' : regEnabled ? t('admin.system.enabled') : t('admin.system.disabled')}
              </span>
            </span>
          </div>
        </div>
      </div>

      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.system.actions')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.scan')}</span>
            <button type="button" className="admin-btn" disabled={scanning} onClick={() => void handleScan()}>
              {scanning ? t('admin.system.scanning') : t('admin.system.scanStart')}
            </button>
          </div>
          {scanResult && <div className={`admin-info-extra ${scanError ? 'admin-info-extra-error' : ''}`}>{scanResult}</div>}
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.backfill')}</span>
            <button type="button" className="admin-btn" disabled={backfilling} onClick={() => void handleBackfill()}>
              {backfilling ? t('admin.system.backfilling') : t('admin.system.backfillStart')}
            </button>
          </div>
          {backfillResult && <div className={`admin-info-extra ${backfillError ? 'admin-info-extra-error' : ''}`}>{backfillResult}</div>}
        </div>
      </div>

      <AlertDialog open={!!alertMsg} message={alertMsg} onClose={() => setAlertMsg('')} />
    </div>
  )
}
