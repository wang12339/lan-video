use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};

use crate::services::media_service::safe_media_path;
use crate::services::transcoder::Transcoder;

/// Worker re-checks the queue at most every second even without a
/// notification, so tasks re-queued for retry (which don't re-notify) are
/// still picked up promptly.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Total attempts per task (1 initial + 2 retries) before it is
/// dead-lettered (marked `failed` in `transcoding_jobs`).
const MAX_ATTEMPTS: u32 = 3;

/// Retry backoff: base * 2^(attempt-1), capped.
const RETRY_BASE_DELAY: Duration = Duration::from_secs(5);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(60);

/// Safety valve: if the retry bookkeeping ever grows this large something is
/// badly wrong (e.g. a flood of unprocessable tasks); drop the bookkeeping
/// rather than leak memory.
const MAX_RETRY_TRACKED: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeTask {
    pub id: i64,
    pub video_id: i64,
    pub input_path: String,
    pub resolutions: Vec<String>,
    pub status: TaskStatus,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

pub struct TaskQueue {
    queue: Arc<Mutex<VecDeque<TranscodeTask>>>,
    notify: Arc<Notify>,
    transcoder: Transcoder,
    pool: PgPool,
    media_root: PathBuf,
}

impl TaskQueue {
    pub fn new(transcoder: Transcoder, pool: PgPool, media_root: PathBuf) -> Self {
        TaskQueue {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
            transcoder,
            pool,
            media_root,
        }
    }

    /// Enqueue a task. Job rows are persisted by [`Self::add_video_transcode`]
    /// before the task reaches the queue, so the status endpoints and crash
    /// recovery see the work even if the process dies mid-flight.
    ///
    /// A task that is already in the queue (same id) is ignored to prevent
    /// duplicate execution.
    pub async fn add_task(&self, task: TranscodeTask) {
        if task.status != TaskStatus::Pending {
            tracing::warn!(
                "Ignoring non-pending transcode task: video_id={}, status={:?}",
                task.video_id,
                task.status
            );
            return;
        }
        {
            let mut queue = self.queue.lock().await;
            if queue.iter().any(|t| t.id == task.id) {
                tracing::debug!("Transcode task already queued, skipping: id={}", task.id);
                return;
            }
            queue.push_back(task.clone());
        }
        self.notify.notify_one();
    }

    /// Enqueue transcoding for a video from a local (uploaded) media file.
    ///
    /// Job rows are persisted to `transcoding_jobs` — idempotently thanks to
    /// `UNIQUE(video_id, resolution)` — and the task key is the smallest
    /// pending job id, so repeated submissions collapse onto one queue entry
    /// while crash recovery (which rebuilds tasks from the same rows) and
    /// admin cancel stay consistent.
    ///
    /// Semantics:
    /// - dead-lettered (`failed`) rows are revived — a re-submission is an
    ///   explicit retry request;
    /// - `completed` rows whose `video_variants` record is missing (e.g. the
    ///   variant was deleted) are revived, so a deleted variant can be
    ///   re-transcoded;
    /// - rows with a live variant are left alone — a duplicate request is a
    ///   no-op instead of redundant CPU work.
    ///
    /// Returns the job id used as the task key, or `None` when there was
    /// nothing left to do. The caller must pass an input path that already
    /// resolves inside the media root (see [`safe_media_path`]).
    pub async fn add_video_transcode(
        &self,
        video_id: i64,
        input_path: PathBuf,
        resolutions: Vec<String>,
    ) -> Result<Option<i64>, sqlx::Error> {
        // Revive rows that represent re-doable work (failed jobs, or
        // completed jobs whose variant record no longer exists).
        sqlx::query(
            "UPDATE transcoding_jobs SET status = 'pending', error_message = NULL, progress = 0 \
             WHERE video_id = $1 AND status IN ('failed', 'completed') \
               AND NOT EXISTS ( \
                   SELECT 1 FROM video_variants vv \
                   WHERE vv.video_id = transcoding_jobs.video_id \
                     AND vv.resolution = transcoding_jobs.resolution \
               )",
        )
        .bind(video_id)
        .execute(&self.pool)
        .await?;

        // Insert job rows for any resolution that has none yet (no-op for
        // pending/processing/completed rows with a live variant).
        sqlx::query(
            "INSERT INTO transcoding_jobs (video_id, status, resolution) \
             SELECT $1, 'pending', unnest($2::text[]) \
             ON CONFLICT (video_id, resolution) DO NOTHING",
        )
        .bind(video_id)
        .bind(&resolutions)
        .execute(&self.pool)
        .await?;

        let min_id: Option<i64> = sqlx::query_scalar(
            "SELECT MIN(id)::bigint FROM transcoding_jobs \
             WHERE video_id = $1 AND status = 'pending'",
        )
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;

        let Some(min_id) = min_id else {
            tracing::debug!("No pending transcode work for video {}", video_id);
            return Ok(None);
        };
        let id = min_id;

        let task = TranscodeTask {
            id,
            video_id,
            input_path: input_path.to_string_lossy().into_owned(),
            resolutions,
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now().naive_utc(),
        };
        self.add_task(task).await;
        Ok(Some(id))
    }

    pub async fn process_next(&self) -> Option<TranscodeTask> {
        let mut queue = self.queue.lock().await;
        queue.pop_front()
    }

    /// Spawn the background worker. Before the loop it recovers jobs that a
    /// previous process left `pending` or `processing` (crash recovery), so
    /// queued work survives a restart as long as the job rows do.
    pub async fn start_worker(&self) {
        let queue = self.queue.clone();
        let notify = self.notify.clone();
        let transcoder = self.transcoder.clone();
        let pool = self.pool.clone();
        let media_root = self.media_root.clone();

        tokio::spawn(async move {
            recover_stale_jobs(&queue, &notify, &pool, &media_root).await;

            // Attempt bookkeeping lives here, owned by the worker. `retries`
            // counts attempts per task id; `next_attempt_at` holds when a
            // retried task may run again (exponential backoff).
            let mut retries: HashMap<i64, u32> = HashMap::new();
            let mut next_attempt_at: HashMap<i64, Instant> = HashMap::new();

            // Cancellation-safe notify: `notified` is re-created from the
            // previous one, so a wake-up delivered while the queue was being
            // drained is never lost.
            let notified = notify.notified();
            tokio::pin!(notified);
            loop {
                tokio::select! {
                    _ = notified.as_mut() => {
                        notified.set(notify.notified());
                    }
                    _ = tokio::time::sleep(POLL_INTERVAL) => {}
                }

                loop {
                    let task = {
                        let mut q = queue.lock().await;
                        q.pop_front()
                    };
                    let Some(task) = task else { break };

                    // A retried task is not due yet: put it back at the front
                    // and stop draining, letting other tasks go first. The
                    // outer loop will come back within POLL_INTERVAL.
                    if let Some(until) = next_attempt_at.get(&task.id).copied() {
                        if Instant::now() < until {
                            queue.lock().await.push_front(task);
                            break;
                        }
                    }

                    run_task(
                        &pool,
                        &transcoder,
                        &queue,
                        &media_root,
                        task,
                        &mut retries,
                        &mut next_attempt_at,
                    )
                    .await;

                    if retries.len() > MAX_RETRY_TRACKED {
                        retries.clear();
                        next_attempt_at.clear();
                    }
                }
            }
        });
    }

    pub async fn get_queue_size(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        queue.clear();
    }
}

impl Clone for TaskQueue {
    fn clone(&self) -> Self {
        TaskQueue {
            queue: self.queue.clone(),
            notify: self.notify.clone(),
            transcoder: self.transcoder.clone(),
            pool: self.pool.clone(),
            media_root: self.media_root.clone(),
        }
    }
}

/// Run one task to completion (or until it is scheduled for retry).
///
/// The persisted job state is re-checked before doing work: an admin cancel
/// between enqueue and processing flips the rows to `failed`, and we must
/// not resurrect them. Only the still-`pending` resolutions are transcoded.
async fn run_task(
    pool: &PgPool,
    transcoder: &Transcoder,
    queue: &Arc<Mutex<VecDeque<TranscodeTask>>>,
    media_root: &Path,
    task: TranscodeTask,
    retries: &mut HashMap<i64, u32>,
    next_attempt_at: &mut HashMap<i64, Instant>,
) {
    tracing::info!(
        "Processing transcode task: video_id={}, resolutions={:?}",
        task.video_id,
        task.resolutions
    );

    let pending = match pending_resolutions(pool, task.video_id, &task.resolutions).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(
                "Failed to read transcode job state: video_id={}, error={}",
                task.video_id,
                e
            );
            schedule_retry(pool, queue, task, retries, next_attempt_at).await;
            return;
        }
    };

    if pending.is_empty() {
        tracing::warn!(
            "Skipping transcode task (no pending resolutions — cancelled or done): video_id={}",
            task.video_id
        );
        // No further attempts will be made; drop any stale bookkeeping so a
        // future job reusing this id is not delayed by a dead retry timer.
        retries.remove(&task.id);
        next_attempt_at.remove(&task.id);
        return;
    }

    if let Err(e) = mark_processing(pool, task.video_id, &pending).await {
        tracing::error!(
            "Failed to mark transcode job as processing: video_id={}, error={}",
            task.video_id,
            e
        );
        schedule_retry(pool, queue, task, retries, next_attempt_at).await;
        return;
    }

    let input_path = std::path::Path::new(&task.input_path);
    if !is_within_media_root(input_path, media_root) {
        tracing::error!(
            "Refusing to transcode input outside media root: video_id={}, path={:?}",
            task.video_id,
            input_path
        );
        reset_jobs_to_pending(pool, task.video_id, &pending).await;
        schedule_retry(pool, queue, task, retries, next_attempt_at).await;
        return;
    }
    match transcoder
        .transcode(task.video_id, input_path, pending)
        .await
    {
        Ok(variants) => {
            if let Err(e) = persist_variants(pool, task.video_id, &variants).await {
                // The output files exist but the records are missing; retry
                // the whole task (the transcoder overwrites its outputs, and
                // the job rows are reset to `pending` so the retry picks
                // them up again).
                tracing::error!(
                    "Transcode ok but variant persistence failed, will retry: video_id={}, error={}",
                    task.video_id,
                    e
                );
                reset_jobs_to_pending(pool, task.video_id, &task.resolutions).await;
                schedule_retry(pool, queue, task, retries, next_attempt_at).await;
                return;
            }
            tracing::info!(
                "Transcode completed and persisted: video_id={}, {} variants",
                task.video_id,
                variants.len()
            );
            retries.remove(&task.id);
            next_attempt_at.remove(&task.id);
        }
        Err(e) => {
            tracing::error!("Transcode failed: video_id={}, error={}", task.video_id, e);
            // Without this reset the rows would stay `processing` and the
            // retry would find no `pending` work, silently dropping the task
            // instead of retrying/dead-lettering it.
            reset_jobs_to_pending(pool, task.video_id, &task.resolutions).await;
            schedule_retry(pool, queue, task, retries, next_attempt_at).await;
        }
    }
}

/// Flip a failed attempt's job rows back to `pending` so a scheduled retry
/// finds them. Must run before re-enqueueing after a failure that happened
/// once the rows were already marked `processing`.
async fn reset_jobs_to_pending(pool: &PgPool, video_id: i64, resolutions: &[String]) {
    if let Err(e) = sqlx::query(
        "UPDATE transcoding_jobs SET status = 'pending', started_at = NULL \
         WHERE video_id = $1 AND resolution = ANY($2) AND status = 'processing'",
    )
    .bind(video_id)
    .bind(resolutions)
    .execute(pool)
    .await
    {
        tracing::error!(
            "Failed to reset transcode jobs to pending: video_id={}, error={}",
            video_id,
            e
        );
    }
}

/// Belt-and-braces guard: the ffmpeg input must resolve inside the media
/// root. Both enqueue paths already enforce this via [`safe_media_path`], but
/// the worker re-checks so a future regression cannot hand ffmpeg an
/// arbitrary filesystem path (e.g. via a tampered stream_url).
fn is_within_media_root(path: &Path, media_root: &Path) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = media_root.canonicalize() else {
        return false;
    };
    canonical.starts_with(&root)
}

/// Record the failure, and either schedule the task for another attempt
/// (exponential backoff, pushed to the back of the queue so other tasks go
/// first) or dead-letter it after MAX_ATTEMPTS.
async fn schedule_retry(
    pool: &PgPool,
    queue: &Arc<Mutex<VecDeque<TranscodeTask>>>,
    task: TranscodeTask,
    retries: &mut HashMap<i64, u32>,
    next_attempt_at: &mut HashMap<i64, Instant>,
) {
    let attempt = retries.entry(task.id).or_insert(0);
    *attempt += 1;
    let attempt = *attempt;

    if attempt >= MAX_ATTEMPTS {
        retries.remove(&task.id);
        next_attempt_at.remove(&task.id);
        dead_letter(pool, &task).await;
        return;
    }

    let delay = retry_backoff(attempt);
    tracing::warn!(
        "Transcode attempt {}/{} failed, retrying in {:?}: video_id={}",
        attempt,
        MAX_ATTEMPTS,
        delay,
        task.video_id
    );
    next_attempt_at.insert(task.id, Instant::now() + delay);
    queue.lock().await.push_back(task);
}

/// Mark a task's job rows `failed` — the dead letter. The error message is
/// visible to admins via the status endpoints; a dead letter is otherwise
/// only distinguishable from a cancel by its message.
async fn dead_letter(pool: &PgPool, task: &TranscodeTask) {
    let result = sqlx::query(
        "UPDATE transcoding_jobs SET status = 'failed', error_message = $1, completed_at = NOW() \
         WHERE video_id = $2 AND status IN ('pending', 'processing')",
    )
    .bind("Transcode failed after all retry attempts")
    .bind(task.video_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => tracing::error!(
            "Transcode task permanently failed (dead-lettered): video_id={}",
            task.video_id
        ),
        Err(e) => tracing::error!(
            "Transcode task failed and could not be dead-lettered: video_id={}, error={}",
            task.video_id,
            e
        ),
    }
}

/// Exponential backoff for the nth failed attempt (1-based): 5s, 10s, 20s…
/// capped at RETRY_MAX_DELAY.
fn retry_backoff(attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(4);
    let secs = (RETRY_BASE_DELAY.as_secs() << shift).min(RETRY_MAX_DELAY.as_secs());
    Duration::from_secs(secs)
}

/// Resolutions of `resolutions` that are still `pending` in transcoding_jobs.
async fn pending_resolutions(
    pool: &PgPool,
    video_id: i64,
    resolutions: &[String],
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT resolution FROM transcoding_jobs \
         WHERE video_id = $1 AND resolution = ANY($2) AND status = 'pending'",
    )
    .bind(video_id)
    .bind(resolutions)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(r,)| r).collect())
}

async fn mark_processing(
    pool: &PgPool,
    video_id: i64,
    resolutions: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE transcoding_jobs SET status = 'processing', started_at = NOW() \
         WHERE video_id = $1 AND resolution = ANY($2) AND status = 'pending'",
    )
    .bind(video_id)
    .bind(resolutions)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Persist transcoded variants: upsert video_variants rows, flip the job rows
/// to `completed`, and flag the video as having variants.
async fn persist_variants(
    pool: &PgPool,
    video_id: i64,
    variants: &[crate::services::transcoder::VideoVariant],
) -> Result<(), sqlx::Error> {
    for v in variants {
        sqlx::query(
            "INSERT INTO video_variants (video_id, resolution, file_path, file_size, bitrate, codec) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (video_id, resolution) DO UPDATE SET \
                 file_path = EXCLUDED.file_path, \
                 file_size = EXCLUDED.file_size, \
                 bitrate = EXCLUDED.bitrate, \
                 codec = EXCLUDED.codec",
        )
        .bind(video_id)
        .bind(&v.resolution)
        .bind(&v.file_path)
        .bind(v.file_size)
        .bind(v.bitrate)
        .bind(&v.codec)
        .execute(pool)
        .await?;
    }

    sqlx::query("UPDATE videos SET has_variants = true WHERE id = $1")
        .bind(video_id)
        .execute(pool)
        .await?;

    let resolutions: Vec<&String> = variants.iter().map(|v| &v.resolution).collect();
    sqlx::query(
        "UPDATE transcoding_jobs SET status = 'completed', completed_at = NOW() \
         WHERE video_id = $1 AND resolution = ANY($2)",
    )
    .bind(video_id)
    .bind(&resolutions)
    .execute(pool)
    .await?;

    Ok(())
}

/// Startup crash recovery:
/// 1. Rows left `processing` died mid-flight — reset them to `pending`.
/// 2. Rebuild in-memory tasks from all `pending` rows (grouped per video)
///    whose variant does not already exist on disk, and enqueue them.
async fn recover_stale_jobs(
    queue: &Arc<Mutex<VecDeque<TranscodeTask>>>,
    notify: &Notify,
    pool: &PgPool,
    media_root: &Path,
) {
    if let Err(e) = sqlx::query(
        "UPDATE transcoding_jobs SET status = 'pending', started_at = NULL WHERE status = 'processing'",
    )
    .execute(pool)
    .await
    {
        tracing::error!("Failed to reset stale processing jobs: {}", e);
    }

    #[derive(sqlx::FromRow)]
    struct PendingJob {
        video_id: i64,
        min_id: i64,
        resolutions: Vec<String>,
        stream_url: String,
    }

    let jobs: Vec<PendingJob> = match sqlx::query_as::<_, PendingJob>(
        "SELECT j.video_id::bigint AS video_id, \
                MIN(j.id)::bigint AS min_id, \
                array_agg(j.resolution ORDER BY j.resolution) AS resolutions, \
                v.stream_url \
         FROM transcoding_jobs j \
         JOIN videos v ON v.id = j.video_id \
         WHERE j.status = 'pending' AND v.source_type LIKE 'local%' \
           AND NOT EXISTS ( \
               SELECT 1 FROM video_variants vv \
               WHERE vv.video_id = j.video_id AND vv.resolution = j.resolution \
           ) \
         GROUP BY j.video_id, v.stream_url",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to load pending transcode jobs for recovery: {}", e);
            return;
        }
    };

    if jobs.is_empty() {
        return;
    }

    let mut enqueued = 0usize;
    for job in jobs {
        // Resolve the input through the canonicalized-root check used
        // everywhere else in the codebase: a bare `Path::join` would treat a
        // stream_url starting with `/media/` as absolute, replace the media
        // root, and escape into the filesystem root. Non-existent inputs
        // (canonicalize fails) are skipped just like before.
        let input_path = match safe_media_path(&job.stream_url, media_root) {
            Some(p) if p.is_file() => p,
            _ => {
                tracing::warn!(
                    "Skipping recovered transcode job, input missing or unsafe: video_id={}",
                    job.video_id
                );
                continue;
            }
        };
        let task = TranscodeTask {
            id: job.min_id,
            video_id: job.video_id,
            input_path: input_path.to_string_lossy().to_string(),
            resolutions: job.resolutions,
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now().naive_utc(),
        };
        queue.lock().await.push_back(task);
        enqueued += 1;
    }
    if enqueued > 0 {
        tracing::info!("Recovered {} transcode job(s) from database", enqueued);
        notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_backoff_grows_and_caps() {
        assert_eq!(retry_backoff(1), Duration::from_secs(5));
        assert_eq!(retry_backoff(2), Duration::from_secs(10));
        assert_eq!(retry_backoff(3), Duration::from_secs(20));
        assert_eq!(retry_backoff(4), Duration::from_secs(40));
        // Cap at the maximum delay, not overflow
        assert_eq!(retry_backoff(100), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_task_queue() {
        // This test would need mock objects for Transcoder and PgPool
        // For now, just test the queue logic
        let queue = Arc::new(Mutex::new(VecDeque::new()));

        let task = TranscodeTask {
            id: 1,
            video_id: 1,
            input_path: "/tmp/test.mp4".to_string(),
            resolutions: vec!["720p".to_string()],
            status: TaskStatus::Pending,
            created_at: chrono::Utc::now().naive_utc(),
        };

        {
            let mut q = queue.lock().await;
            q.push_back(task.clone());
        }

        {
            let mut q = queue.lock().await;
            let popped = q.pop_front();
            assert!(popped.is_some());
            assert_eq!(popped.unwrap().video_id, 1);
        }
    }
}
