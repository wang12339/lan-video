import { useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useUploadManager, CATEGORIES } from './hooks/useUploadManager'
import type { UploadItem } from './hooks/useUploadManager'
import { useToast } from '../../components/Toast/Toast'
import CategorySelector from './CategorySelector'
import DropZone from './DropZone'
import UploadQueue from './UploadQueue'
import i18n from '../../i18n'
import './Upload.css'

export default function Upload() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const {
    files, setFiles, category, setCategory,
    burnAfterWatch, setBurnAfterWatch,
    dragOver, uploading, addFiles,
    startUpload, cancelUpload,
    handleDrop, handleDragEnter, handleDragLeave,
  } = useUploadManager()

  const removeFile = useCallback((idx: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== idx))
  }, [setFiles])

  const retryItem = useCallback((idx: number) => {
    setFiles((prev) =>
      prev.map((f, i) =>
        i === idx ? { ...f, status: 'pending' as const, progress: 0, errorMsg: undefined } : f
      )
    )
  }, [setFiles])

  const clearAll = useCallback(() => {
    setFiles([])
  }, [setFiles])

  const handleCategoryChange = useCallback((item: UploadItem, newCategory: string) => {
    setFiles((prev) =>
      prev.map((f) => (f === item ? { ...f, category: newCategory } : f))
    )
  }, [setFiles])

  const pendingCount = useMemo(() => files.filter((f) => f.status === 'pending' || f.status === 'error').length, [files])

  return (
    <div className="upload-page">
      <div className="upload-header">
        <h1>{t('upload.title')}</h1>
        <p>{i18n.t('upload.formatHint')}</p>
      </div>

      <DropZone
        dragOver={dragOver}
        hasFiles={files.length > 0}
        disabled={uploading}
        onDrop={handleDrop}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onFilesSelected={addFiles}
        disabledHint={() => toast(i18n.t('upload.uploadBusy'), 'info')}
      />

      {files.length > 0 && (
        <>
          <CategorySelector
            category={category}
            setCategory={setCategory}
            disabled={uploading}
            categories={CATEGORIES}
          />

          <label className="upload-burn-toggle">
            <input
              type="checkbox"
              checked={burnAfterWatch}
              onChange={(e) => setBurnAfterWatch(e.target.checked)}
              disabled={uploading}
            />
            <span className="upload-burn-toggle__mark">🔥</span>
            <span className="upload-burn-toggle__text">
              <strong>{t('upload.burnAfterWatch')}</strong>
              <small>{t('upload.burnAfterWatchHint')}</small>
            </span>
          </label>

          <UploadQueue
            files={files}
            uploading={uploading}
            categories={CATEGORIES}
            onCategoryChange={handleCategoryChange}
            onRetry={retryItem}
            onRemove={removeFile}
            actionButtons={
              <>
                {uploading ? (
                  <button className="upload-cancel-btn" onClick={cancelUpload}>
                    {t('common.cancel')}
                  </button>
                ) : (
                  <button
                    className="upload-start-btn"
                    onClick={startUpload}
                    disabled={pendingCount === 0}
                  >
                    {t('upload.pendingInfo', { pendingCount })}
                  </button>
                )}
                <button className="upload-clear-btn" onClick={clearAll} disabled={uploading}>
                  {t('upload.clearAll')}
                </button>
              </>
            }
          />
        </>
      )}
    </div>
  )
}
