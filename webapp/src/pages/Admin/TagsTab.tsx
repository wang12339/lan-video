import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listTags, createTag, updateTag, deleteTag, type Tag } from '../../api';
import { ConfirmDialog } from '../../components/ui';

const PRESET_COLORS = [
  '#3b82f6', '#ec4899', '#8b5cf6', '#10b981', '#f59e0b',
  '#ef4444', '#06b6d4', '#f97316', '#84cc16', '#6366f1',
];

export default function TagsTab() {
  const { t } = useTranslation()
  const [tags, setTags] = useState<Tag[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [showForm, setShowForm] = useState(false);
  const [editingTag, setEditingTag] = useState<Tag | null>(null);
  const [formName, setFormName] = useState('');
  const [formColor, setFormColor] = useState(PRESET_COLORS[0]);
  const [confirmDelete, setConfirmDelete] = useState<Tag | null>(null);

  const loadTags = useCallback(async () => {
    try {
      setLoading(true);
      setError('');
      const data = await listTags();
      setTags(data);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : t('admin.tags.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadTags(); }, [loadTags]);

  function openCreate() {
    setEditingTag(null);
    setFormName('');
    setFormColor(PRESET_COLORS[0]);
    setShowForm(true);
  }

  function openEdit(tag: Tag) {
    setEditingTag(tag);
    setFormName(tag.name);
    setFormColor(tag.color || PRESET_COLORS[0]);
    setShowForm(true);
  }

  async function handleSave() {
    const name = formName.trim();
    if (!name) return;
    try {
      if (editingTag) {
        await updateTag(editingTag.id, { name, color: formColor });
      } else {
        await createTag({ name, color: formColor });
      }
      setShowForm(false);
      loadTags();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : t('admin.tags.saveFailed'));
    }
  }

  async function handleDelete() {
    if (!confirmDelete) return;
    try {
      await deleteTag(confirmDelete.id);
      setConfirmDelete(null);
      loadTags();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : t('admin.tags.deleteFailed'));
    }
  }

  return (
    <div className="tags-tab">
      <div className="tags-header">
        <h2>{t('admin.tags.title')}</h2>
        <button className="admin-btn admin-btn-primary" onClick={openCreate}>{t('admin.tags.newTag')}</button>
      </div>

      {error && <div className="admin-error">{error}</div>}

      {loading ? (
        <div className="admin-loading">{t('common.loading')}</div>
      ) : tags.length === 0 ? (
        <div className="admin-empty">{t('admin.tags.empty')}</div>
      ) : (
        <div className="tags-grid">
          {tags.map(tag => (
            <div key={tag.id} className="tag-card">
              <div className="tag-card-header">
                <span className="tag-badge" style={{ background: tag.color || '#666' }}>{tag.name}</span>
                <span className="tag-count">{t('admin.tags.videos', { count: tag.usageCount })}</span>
              </div>
              <div className="tag-card-actions">
                <button className="admin-btn-sm" onClick={() => openEdit(tag)}>{t('admin.tags.edit')}</button>
                <button className="admin-btn-sm admin-btn-danger" onClick={() => setConfirmDelete(tag)}>{t('admin.tags.delete')}</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {showForm && (
        <div className="admin-modal-overlay" onClick={() => setShowForm(false)}>
          <div className="admin-modal" onClick={e => e.stopPropagation()}>
            <h3>{editingTag ? t('admin.tags.editTag') : t('admin.tags.createTag')}</h3>
            <div className="form-group">
              <label>{t('admin.tags.tagName')}</label>
              <input
                type="text"
                value={formName}
                onChange={e => setFormName(e.target.value)}
                placeholder={t('admin.tags.tagNamePlaceholder')}
                maxLength={50}
                autoFocus
              />
            </div>
            <div className="form-group">
              <label>{t('admin.tags.tagColor')}</label>
              <div className="color-picker">
                {PRESET_COLORS.map(c => (
                  <button
                    key={c}
                    className={`color-swatch ${formColor === c ? 'selected' : ''}`}
                    style={{ background: c }}
                    onClick={() => setFormColor(c)}
                  />
                ))}
              </div>
            </div>
            <div className="form-group">
              <label>{t('admin.tags.preview')}</label>
              <span className="tag-badge" style={{ background: formColor }}>{formName || t('admin.tags.preview')}</span>
            </div>
            <div className="admin-modal-actions">
              <button className="admin-btn" onClick={() => setShowForm(false)}>{t('common.cancel')}</button>
              <button className="admin-btn admin-btn-primary" onClick={handleSave} disabled={!formName.trim()}>
                {editingTag ? t('common.save') : t('common.create')}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmDelete && (
        <ConfirmDialog
          open={true}
          title={t('admin.tags.deleteTitle')}
          message={t('admin.tags.deleteConfirm', { name: confirmDelete.name })}
          danger
          onConfirm={handleDelete}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </div>
  );
}
