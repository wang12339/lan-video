import { memo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { listReplies, createComment, deleteComment } from '../../api'
import type { Comment } from '../../api'
import { useAuth } from '../../context/AuthContext'
import { ConfirmDialog } from '../ui'
import { formatDate, getInitial, COMMENT_MAX_LENGTH } from './utils'

interface Props {
  comment: Comment
  onDelete: (comment: Comment) => void
  videoId: string
}

function CommentAvatar({ url, username, small = false }: { url: string | null; username: string; small?: boolean }) {
  if (url) {
    return <img src={url} alt={username} />
  }
  return <div className={`comment-avatar-placeholder${small ? ' small' : ''}`}>{getInitial(username)}</div>
}

export default memo(function CommentItem({ comment, onDelete, videoId }: Props) {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [replyTo, setReplyTo] = useState<string | null>(null)
  const [replyContent, setReplyContent] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [replyError, setReplyError] = useState('')
  const [expanded, setExpanded] = useState(false)
  const [replies, setReplies] = useState<Comment[]>([])
  const [repliesLoaded, setRepliesLoaded] = useState(false)
  const [repliesLoading, setRepliesLoading] = useState(false)
  const [repliesError, setRepliesError] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<Comment | null>(null)
  const [deleteError, setDeleteError] = useState('')

  const canDelete = (target: Comment) => user?.id === target.userId || user?.isAdmin

  const submitReply = async () => {
    const trimmed = replyContent.trim()
    if (!trimmed || submitting) return
    setSubmitting(true)
    setReplyError('')
    try {
      const created = await createComment(videoId, trimmed, comment.id)
      setReplyContent('')
      setReplyTo(null)
      setReplies(prev => [...prev, created])
      setRepliesLoaded(true)
      setExpanded(true)
    } catch {
      setReplyError(t('comments.submitFailed'))
    } finally {
      setSubmitting(false)
    }
  }

  const loadReplies = async () => {
    if (repliesLoaded || repliesLoading) return
    setRepliesLoading(true)
    setRepliesError('')
    try {
      const data = await listReplies(comment.id)
      setReplies(data)
      setRepliesLoaded(true)
    } catch {
      setRepliesError(t('comments.loadFailed'))
    } finally {
      setRepliesLoading(false)
    }
  }

  const toggleReplies = () => {
    if (expanded) {
      setExpanded(false)
      return
    }
    void loadReplies()
    setExpanded(true)
  }

  const handleDeleteReply = async () => {
    if (!deleteTarget) return
    setDeleteError('')
    try {
      await deleteComment(deleteTarget.id)
      setReplies(prev => prev.filter(r => r.id !== deleteTarget.id))
      setDeleteTarget(null)
    } catch {
      setDeleteError(t('errors.serverError'))
    }
  }

  const onReplyKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      void submitReply()
    }
  }

  const replyCountAtLimit = replyContent.length > COMMENT_MAX_LENGTH * 0.9

  return (
    <div className="comment-item">
      <div className="comment-avatar">
        <CommentAvatar url={comment.avatarUrl} username={comment.username} />
      </div>
      <div className="comment-body">
        <div className="comment-header">
          <span className="comment-user">{comment.username}</span>
          <span className="comment-time">{formatDate(comment.createdAt)}</span>
        </div>
        <p className="comment-text">{comment.content}</p>
        <div className="comment-actions">
          {user && (
            <button
              className="comment-action-btn"
              onClick={() => { setReplyTo(replyTo === comment.id ? null : comment.id); setReplyError('') }}
            >
              {t('comments.reply')}
            </button>
          )}
          {canDelete(comment) && (
            <button className="comment-action-btn danger" onClick={() => onDelete(comment)}>
              {t('comments.delete')}
            </button>
          )}
        </div>

        {replyTo === comment.id && (
          <form className="reply-form" onSubmit={(e) => { e.preventDefault(); void submitReply() }}>
            <textarea
              className="reply-input"
              value={replyContent}
              onChange={(e) => setReplyContent(e.target.value)}
              onKeyDown={onReplyKeyDown}
              placeholder={t('comments.replyPlaceholder', { name: comment.username })}
              rows={2}
              maxLength={COMMENT_MAX_LENGTH}
              aria-label={t('comments.placeholder')}
              autoFocus
            />
            <div className="reply-meta">
              <span className={`comment-count${replyCountAtLimit ? ' near-limit' : ''}`}>
                {replyContent.length}/{COMMENT_MAX_LENGTH}
              </span>
              <div className="reply-actions">
                <button className="comment-action-btn" type="button" onClick={() => { setReplyTo(null); setReplyContent(''); setReplyError('') }}>{t('common.cancel')}</button>
                <button
                  className="comment-submit small"
                  type="submit"
                  disabled={!replyContent.trim() || submitting}
                >
                  {submitting ? t('comments.submitting') : t('comments.reply')}
                </button>
              </div>
            </div>
            {replyError && <p className="comments-error compact" role="alert">{replyError}</p>}
          </form>
        )}

        <button className="show-replies-btn" onClick={toggleReplies}>
          {expanded ? t('comments.hideReplies') : t('comments.showReplies')}
        </button>

        {expanded && (
          <div className="reply-list">
            {repliesLoading && !repliesLoaded && <p className="comments-loading">{t('common.loading')}</p>}
            {repliesError && (
              <div className="reply-error-block">
                <p className="comments-error compact" role="alert">{repliesError}</p>
                <button className="comments-retry-btn small" onClick={() => void loadReplies()}>
                  {t('common.retry')}
                </button>
              </div>
            )}
            {replies.map(r => (
              <div key={r.id} className="reply-item">
                <div className="reply-avatar">
                  <CommentAvatar url={r.avatarUrl} username={r.username} small />
                </div>
                <div className="reply-body">
                  <div className="comment-header">
                    <span className="comment-user">{r.username}</span>
                    <span className="comment-time">{formatDate(r.createdAt)}</span>
                  </div>
                  <p className="comment-text">{r.content}</p>
                  {canDelete(r) && (
                    <div className="comment-actions">
                      <button className="comment-action-btn danger" onClick={() => setDeleteTarget(r)}>
                        {t('comments.delete')}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            ))}
            {deleteError && <p className="comments-error compact" role="alert">{deleteError}</p>}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title={t('comments.deleteConfirm')}
        message={t('comments.notRecoverable')}
        danger
        onConfirm={handleDeleteReply}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  )
})
