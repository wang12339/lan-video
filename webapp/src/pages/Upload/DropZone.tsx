import { memo, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import i18n from '../../i18n'

interface Props {
  dragOver: boolean
  hasFiles: boolean
  disabled: boolean
  onDrop: (e: React.DragEvent) => void
  onDragEnter: (e: React.DragEvent) => void
  onDragLeave: (e: React.DragEvent) => void
  onFilesSelected: (files: File[]) => void
  disabledHint: () => void
}

function DropZone({
  dragOver, hasFiles, disabled,
  onDrop, onDragEnter, onDragLeave, onFilesSelected, disabledHint,
}: Props) {
  const { t } = useTranslation()
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleFileInput = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      onFilesSelected(Array.from(e.target.files))
    }
    e.target.value = ''
  }, [onFilesSelected])

  const openFilePicker = useCallback(() => {
    if (disabled) {
      disabledHint()
      return
    }
    fileInputRef.current?.click()
  }, [disabled, disabledHint])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      openFilePicker()
    }
  }, [openFilePicker])

  const handleDragOver = useCallback((e: React.DragEvent) => e.preventDefault(), [])

  return (
    <>
      <div
        className={`upload-dropzone ${dragOver ? 'drag-over' : ''} ${hasFiles ? 'has-files' : ''} ${disabled ? 'disabled' : ''}`}
        role="button"
        tabIndex={0}
        aria-label={i18n.t('upload.selectFilesAria')}
        aria-disabled={disabled}
        onClick={openFilePicker}
        onKeyDown={handleKeyDown}
        onDragEnter={onDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
      >
        <div className="dropzone-icon" aria-hidden="true">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
        </div>
        <p className="dropzone-text">{t('upload.orDrag')}</p>
        <p className="dropzone-sub">{i18n.t('upload.orClickBelow')}</p>
        <input
          ref={fileInputRef}
          type="file"
          accept="video/*,image/*"
          hidden
          multiple
          onChange={handleFileInput}
        />
      </div>
      <div className="upload-select-btns">
        <button className="upload-select-btn" onClick={openFilePicker} disabled={disabled}>
          {t('upload.select')}
        </button>
      </div>
    </>
  )
}

export default memo(DropZone)
