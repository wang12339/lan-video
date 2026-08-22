import { useState, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useInfiniteQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import type { InfiniteData } from '@tanstack/react-query'
import { request } from '../../api/client'
import { createComment, deleteComment } from '../../api'
import type { Comment, CommentListResponse } from '../../api'
import { useAuth } from '../../context/AuthContext'
import { ConfirmDialog } from '../ui'
import CommentItem from './CommentItem'
import { COMMENT_MAX_LENGTH } from './utils'
import './Comments.css'

const COMMENT_PAGE_SIZE = 20

interface Props {
  videoId: string
}

export default function Comments({ videoId }: Props) {
  const { t } = useTranslation()
  const { user } = useAuth()
  const queryClient = useQueryClient()
  const [content, setContent] = useState('')
  const [submitError, setSubmitError] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<Comment | null>(null)
  const [deleteError, setDeleteError] = useState('')
  const [loadMoreFailed, setLoadMoreFailed] = useState(false)
  const newCommentRef = useRef<HTMLDivElement>(null)
  const commentsListRef = useRef<HTMLDivElement>(null)

  // 评论列表由 react-query 缓存（staleTime 30s，组件卸载后 gcTime 5min 内命中）；
  // queryFn 走 request 但 skipCache=true，绕过 client 层 LRU，避免双重缓存失效不一致
  const queryKey = ['comments', videoId] as const
  const {
    data,
    isPending,
    isError,
    error,
    hasNextPage,
    isFetching,
    isFetchingNextPage,
    fetchNextPage,
    refetch,
  } = useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam }) =>
      request<CommentListResponse>(
        `/videos/${videoId}/comments?page=${pageParam}&size=${COMMENT_PAGE_SIZE}`,
        { skipCache: true },
      ),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      const loaded = allPages.reduce((n, p) => n + p.comments.length, 0)
      return loaded < lastPage.total ? allPages.length : undefined
    },
    enabled: !!videoId && !!user,
    staleTime: 30_000,
  })

  const comments = data?.pages.flatMap((p) => p.comments) ?? []
  const total = data?.pages[data.pages.length - 1]?.total ?? 0

  const createMutation = useMutation({
    mutationFn: (text: string) => createComment(videoId, text),
    onMutate: async (text) => {
      await queryClient.cancelQueries({ queryKey })
      const prev = queryClient.getQueryData<InfiniteData<CommentListResponse>>(queryKey)
      const tempId = `temp-${Date.now()}`
      const optimistic: Comment = {
        id: tempId,
        videoId,
        userId: user?.id ?? 'me',
        username: user?.username ?? t('common.you'),
        avatarUrl: user?.avatarUrl ?? null,
        content: text,
        parentId: null,
        createdAt: new Date().toISOString(),
      }
      queryClient.setQueryData<InfiniteData<CommentListResponse>>(queryKey, (old) => {
        if (!old) return old
        return {
          ...old,
          pages: old.pages.map((p, i) => (i === 0
            ? { ...p, comments: [optimistic, ...p.comments], total: p.total + 1 }
            : p)),
        }
      })
      setContent('')
      setSubmitError('')
      return { prev, tempId }
    },
    onError: (_err, _text, ctx) => {
      if (ctx?.prev) queryClient.setQueryData(queryKey, ctx.prev)
      setSubmitError(t('comments.submitFailed'))
      // 失败时回滚并恢复输入（若需要可把 text 设回）
    },
    onSuccess: (created, _text, ctx) => {
      // 用真实 id 替换临时 id
      queryClient.setQueryData<InfiniteData<CommentListResponse>>(queryKey, (old) => {
        if (!old) return old
        return {
          ...old,
          pages: old.pages.map((p, i) => (i === 0
            ? { ...p, comments: p.comments.map(c => c.id === ctx?.tempId ? created : c) }
            : p)),
        }
      })
      setTimeout(() => {
        newCommentRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' })
      }, 100)
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteComment(id),
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey })
      const prev = queryClient.getQueryData<InfiniteData<CommentListResponse>>(queryKey)
      queryClient.setQueryData<InfiniteData<CommentListResponse>>(queryKey, (old) => {
        if (!old) return old
        return {
          ...old,
          pages: old.pages.map((p) => ({
            ...p,
            comments: p.comments.filter((c) => c.id !== id),
            total: Math.max(0, p.total - 1),
          })),
        }
      })
      setDeleteTarget(null)
      return { prev }
    },
    onError: (_err, _id, ctx) => {
      if (ctx?.prev) queryClient.setQueryData(queryKey, ctx.prev)
      setDeleteError(t('errors.serverError'))
    },
    onSettled: () => {
      // 确保最终一致性
      queryClient.invalidateQueries({ queryKey })
    },
  })

  const submit = () => {
    const trimmed = content.trim()
    if (!trimmed || createMutation.isPending) return
    createMutation.mutate(trimmed)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    submit()
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault()
      submit()
    }
  }

  const handleLoadMore = () => {
    if (isFetchingNextPage || !hasNextPage) return
    setLoadMoreFailed(false)
    fetchNextPage().catch(() => setLoadMoreFailed(true))
  }

  const handleDelete = () => {
    if (!deleteTarget) return
    setDeleteError('')
    deleteMutation.mutate(deleteTarget.id)
  }

  const countAtLimit = content.length > COMMENT_MAX_LENGTH * 0.9
  const errorMessage = error?.message || t('comments.loadFailed')

  return (
    <div className="comments-section">
      <h3 className="comments-title">{t('comments.title', { total })}</h3>

      {user && (
        <form className="comment-form" onSubmit={handleSubmit}>
          <textarea
            className="comment-input"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('comments.placeholder')}
            rows={2}
            maxLength={COMMENT_MAX_LENGTH}
            aria-label={t('comments.placeholder')}
          />
          <div className="comment-form-side">
            <span className={`comment-count${countAtLimit ? ' near-limit' : ''}`}>
              {content.length}/{COMMENT_MAX_LENGTH}
            </span>
            <button
              className="comment-submit"
              type="submit"
              disabled={!content.trim() || createMutation.isPending}
            >
              {createMutation.isPending ? t('comments.submitting') : t('comments.submit')}
            </button>
            {submitError && <p className="comments-error compact" role="alert">{submitError}</p>}
          </div>
        </form>
      )}

      <div className="comments-list" ref={commentsListRef}>
        {comments.map((c, index) => (
          <div key={c.id} ref={index === 0 ? newCommentRef : undefined}>
            <CommentItem comment={c} onDelete={setDeleteTarget} videoId={videoId} />
          </div>
        ))}

        {isPending && comments.length === 0 && (
          <p className="comments-loading">{t('common.loading')}</p>
        )}
        {isError && comments.length === 0 && (
          <div className="comments-error-block">
            <p className="comments-error" role="alert">{errorMessage}</p>
            <button className="retry-btn" onClick={() => void refetch()} disabled={isFetching}>
              {t('common.retry')}
            </button>
          </div>
        )}
        {deleteError && (
          <p className="comments-error" role="alert">{deleteError}</p>
        )}
        {comments.length === 0 && !isPending && !isError && (
          <p className="comments-empty">{t('comments.noComments')}</p>
        )}

        {loadMoreFailed && (
          <>
            <p className="comments-error compact" role="alert">{t('comments.loadFailed')}</p>
            <button className="load-more-btn" onClick={handleLoadMore}>
              {t('common.retry')}
            </button>
          </>
        )}
        {!loadMoreFailed && comments.length < total && (
          <button className="load-more-btn" onClick={handleLoadMore} disabled={isFetchingNextPage}>
            {isFetchingNextPage ? t('common.loading') : t('common.loadMore')}
          </button>
        )}
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        title={t('comments.deleteConfirm')}
        message={t('comments.notRecoverable')}
        danger
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  )
}
