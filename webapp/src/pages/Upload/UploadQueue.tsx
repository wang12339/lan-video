import React, { type ReactNode } from 'react'
import type { Category } from '../../config/categories'
import type { UploadItem } from './hooks/useUploadManager'
import UploadItemRow from './UploadItem'

interface Props {
  files: UploadItem[]
  uploading: boolean
  categories: Category[]
  onCategoryChange: (item: UploadItem, category: string) => void
  onRetry: (idx: number) => void
  onRemove: (idx: number) => void
  actionButtons: ReactNode
}

function UploadQueue({
  files, uploading, categories,
  onCategoryChange, onRetry, onRemove, actionButtons,
}: Props) {
  return (
    <>
      <div className="upload-queue">
        {files.map((item, idx) => (
          <UploadItemRow
            key={item.file.name + item.file.size + item.file.lastModified}
            item={item}
            uploading={uploading}
            categories={categories}
            onCategoryChange={onCategoryChange}
            onRetry={() => onRetry(idx)}
            onRemove={() => onRemove(idx)}
          />
        ))}
      </div>
      <div className="upload-actions">
        {actionButtons}
      </div>
    </>
  )
}

export default React.memo(UploadQueue)
