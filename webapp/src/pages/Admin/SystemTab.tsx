import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { health } from '../../api'
import { getSystemInfo, scanMedia, backfillThumbnails, getRegistrationEnabled, setRegistrationEnabled } from '../../api/admin'
import type { SystemInfo } from '../../api/admin'
import { AlertDialog } from '../../components/ui'

export default function SystemTab() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [scanning, setScanning] = useState(false)
  const [scanResult, setScanResult] = useState('')
  const [backfilling, setBackfilling] = useState(false)
  const [backfillResult, setBackfillResult] = useState('')
  const [alertMsg, setAlertMsg] = useState('')

  const { data: serverOk } = useQuery({
    queryKey: ['health'],
    queryFn: () => health(),
    refetchInterval: 30_000,
  })
  const { data: sys } = useQuery<SystemInfo>({
    queryKey: ['admin-system-info'],
    queryFn: getSystemInfo,
    refetchInterval: 30_000,
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
    try {
      const res = await scanMedia()
      setScanResult(t('admin.system.scanComplete', { added: res.added }))
      queryClient.invalidateQueries({ queryKey: ['admin-stats'] })
    } catch (e) {
      setScanResult(e instanceof Error ? e.message : t('admin.system.scanFailed'))
    } finally {
      setScanning(false)
    }
  }

  const handleBackfill = async () => {
    if (backfilling) return
    setBackfilling(true)
    setBackfillResult('')
    try {
      const res = await backfillThumbnails()
      setBackfillResult(res.ok ? t('admin.system.backfillComplete', { generated: res.generated }) : t('admin.system.backfillFailed'))
    } catch (e) {
      setBackfillResult(e instanceof Error ? e.message : t('admin.system.backfillFailed'))
    } finally {
      setBackfilling(false)
    }
  }

  return (
    <div className="admin-tab-content">
      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.system.status')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.serverStatus')}</span>
            <span className={`admin-info-value ${serverOk ? 'ok' : 'fail'}`}>
              {serverOk === undefined ? '...' : serverOk ? t('admin.system.serverOk') : t('admin.system.serverFail')}
            </span>
          </div>
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.dbConnections')}</span>
            <span className="admin-info-value">{sys?.dbConnections ?? '--'}</span>
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
            <button
              className={`admin-btn ${regEnabled ? 'admin-btn-active' : ''}`}
              disabled={regEnabled === undefined || toggleRegistrationMut.isPending}
              onClick={handleToggleRegistration}
            >
              {regEnabled ? t('admin.system.enabled') : t('admin.system.disabled')}
            </button>
          </div>
        </div>
      </div>

      <div className="admin-section">
        <h3 className="admin-section-title">{t('admin.system.actions')}</h3>
        <div className="admin-card">
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.scan')}</span>
            <button className="admin-btn" disabled={scanning} onClick={handleScan}>
              {scanning ? t('admin.system.scanning') : t('admin.system.scanStart')}
            </button>
          </div>
          {scanResult && <div className="admin-info-extra">{scanResult}</div>}
          <div className="admin-info-row">
            <span className="admin-info-label">{t('admin.system.backfill')}</span>
            <button className="admin-btn" disabled={backfilling} onClick={handleBackfill}>
              {backfilling ? t('admin.system.backfilling') : t('admin.system.backfillStart')}
            </button>
          </div>
          {backfillResult && <div className="admin-info-extra">{backfillResult}</div>}
        </div>
      </div>

      <AlertDialog open={!!alertMsg} message={alertMsg} onClose={() => setAlertMsg('')} />
    </div>
  )
}
