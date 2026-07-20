import { useState, useEffect, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { listComments, createComment, deleteComment } from '../../api'
import type { Comment } from '../../api'
import { useAuth } from '../../context/AuthContext'
import CommentItem from './CommentItem'
import './Comments.css'

interface Props {
  videoId: number
}

export default function Comments({ videoId }: Props) {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [comments, setComments] = useState<Comment[]>([])
  const [total, setTotal] = useState(0)
  const pageRef = useRef(0)
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState('')
  const [content, setContent] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState('')

  const loadComments = useCallback(async (p: number, append = false) => {
    setLoading(true)
    setLoadError('')
    try {
      const res = await listComments(videoId, p)
      setTotal(res.total)
      setComments(prev => append ? [...prev, ...res.comments] : res.comments)
    } catch (e) {
      const err = e as { status?: number; message?: string }
      setLoadError(err.status ? `服务器错误 (${err.status})` : t('comments.loadFailed'))
    } finally {
      setLoading(false)
    }
  }, [videoId])

  useEffect(() => {
    loadComments(0)
  }, [loadComments])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const trimmed = content.trim()
    if (!trimmed || submitting) return
    setSubmitting(true)
    setSubmitError('')
    try {
      await createComment(videoId, trimmed)
      setContent('')
      loadComments(0)
    } catch {
      setSubmitError(t('comments.submitFailed'))
    } finally {
      setSubmitting(false)
    }
  }

  const handleDelete = async (commentId: number) => {
    try {
      await deleteComment(commentId)
      setComments(prev => prev.filter(c => c.id !== commentId))
      setTotal(prev => prev - 1)
    } catch (e) {
      console.error('Failed to delete comment:', e)
    }
  }

  return (
    <div className="comments-section">
      <h3 className="comments-title">{t('comments.title', { total })}</h3>

      {user && (
        <form className="comment-form" onSubmit={handleSubmit}>
          <textarea
            className="comment-input"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder={t('comments.placeholder')}
            rows={2}
            maxLength={2000}
          />
          <button
            className="comment-submit"
            type="submit"
            disabled={!content.trim() || submitting}
          >
            {submitting ? t('comments.submitting') : t('comments.submit')}
          </button>
          {submitError && <p className="comments-error" style={{ marginTop: 0 }}>{submitError}</p>}
        </form>
      )}

      <div className="comments-list">
        {comments.map(c => (
          <CommentItem key={c.id} comment={c} onDelete={handleDelete} videoId={videoId} />
        ))}

        {loadError && (
          <p className="comments-error">{loadError}</p>
        )}
        {comments.length === 0 && !loading && !loadError && (
          <p className="comments-empty">{t('comments.noComments')}</p>
        )}

        {comments.length < total && (
          <button className="load-more-btn" onClick={() => { pageRef.current += 1; loadComments(pageRef.current, true) }} disabled={loading}>
            {loading ? t('common.loading') : t('common.loadMore')}
          </button>
        )}
      </div>
    </div>
  )
}
