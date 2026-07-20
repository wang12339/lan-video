import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { listReplies, createComment } from '../../api'
import type { Comment } from '../../api'
import { useAuth } from '../../context/AuthContext'
import { formatDate } from './utils'

interface Props {
  comment: Comment
  onDelete: (id: number) => void
  videoId: number
}

export default function CommentItem({ comment, onDelete, videoId }: Props) {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [replyTo, setReplyTo] = useState<number | null>(null)
  const [replyContent, setReplyContent] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [replies, setReplies] = useState<Comment[]>([])
  const [repliesLoaded, setRepliesLoaded] = useState(false)

  const handleReply = async (parentId: number) => {
    const trimmed = replyContent.trim()
    if (!trimmed || submitting) return
    setSubmitting(true)
    try {
      await createComment(videoId, trimmed, parentId)
      setReplyContent('')
      setReplyTo(null)
      const data = await listReplies(parentId)
      setReplies(data)
      setExpanded(true)
      setRepliesLoaded(true)
    } catch (e) {
      console.error('Failed to submit reply:', e)
    } finally {
      setSubmitting(false)
    }
  }

  const toggleReplies = async () => {
    if (!expanded && !repliesLoaded && comment.id != null) {
      try {
        const data = await listReplies(comment.id)
        setReplies(data)
        setRepliesLoaded(true)
      } catch (e) {
        console.error('Failed to load replies:', e)
      }
    }
    setExpanded(!expanded)
  }

  return (
    <div className="comment-item">
      <div className="comment-avatar">
        {comment.avatarUrl
          ? <img src={comment.avatarUrl} alt={comment.username} />
          : <div className="comment-avatar-placeholder">{comment.username[0]}</div>
        }
      </div>
      <div className="comment-body">
        <div className="comment-header">
          <span className="comment-user">{comment.username}</span>
          <span className="comment-time">{formatDate(comment.createdAt)}</span>
        </div>
        <p className="comment-text">{comment.content}</p>
        <div className="comment-actions">
          {user && (
            <button className="comment-action-btn" onClick={() => setReplyTo(replyTo === comment.id ? null : comment.id)}>
              {t('comments.reply')}
            </button>
          )}
          {(user?.id === comment.userId || user?.isAdmin) && (
            <button className="comment-action-btn danger" onClick={() => onDelete(comment.id)}>
              {t('comments.delete')}
            </button>
          )}
        </div>

        {replyTo === comment.id && (
          <form className="reply-form" onSubmit={(e) => { e.preventDefault(); handleReply(comment.id) }}>
            <textarea
              className="reply-input"
              value={replyContent}
              onChange={(e) => setReplyContent(e.target.value)}
              placeholder={`回复 @${comment.username}...`}
              rows={2}
              maxLength={2000}
            />
            <div className="reply-actions">
              <button className="comment-action-btn" type="button" onClick={() => { setReplyTo(null); setReplyContent('') }}>{t('common.cancel')}</button>
              <button
                className="comment-submit small"
                type="submit"
                disabled={!replyContent.trim() || submitting}
              >
                {t('comments.reply')}
              </button>
            </div>
          </form>
        )}

        {comment.id != null && (
          <button className="show-replies-btn" onClick={toggleReplies}>
            {expanded ? t('comments.hideReplies') : t('comments.showReplies')}
          </button>
        )}

        {expanded && replies.map(r => (
          <div key={r.id} className="reply-item">
            <div className="reply-avatar">
              {r.avatarUrl
                ? <img src={r.avatarUrl} alt={r.username} />
                : <div className="comment-avatar-placeholder small">{r.username[0]}</div>
              }
            </div>
            <div className="reply-body">
              <div className="comment-header">
                <span className="comment-user">{r.username}</span>
                <span className="comment-time">{formatDate(r.createdAt)}</span>
              </div>
              <p className="comment-text">{r.content}</p>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
