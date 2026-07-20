use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

use crate::services::transcoder::Transcoder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeTask {
    pub id: i32,
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
}

impl TaskQueue {
    pub fn new(transcoder: Transcoder, pool: PgPool) -> Self {
        TaskQueue {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            notify: Arc::new(Notify::new()),
            transcoder,
            pool,
        }
    }

    pub async fn add_task(&self, task: TranscodeTask) {
        let mut queue = self.queue.lock().await;
        queue.push_back(task);
        self.notify.notify_one();
    }

    pub async fn process_next(&self) -> Option<TranscodeTask> {
        let mut queue = self.queue.lock().await;
        queue.pop_front()
    }

    pub async fn start_worker(&self) {
        let queue = self.queue.clone();
        let notify = self.notify.clone();
        let transcoder = self.transcoder.clone();
        let _pool = self.pool.clone();

        tokio::spawn(async move {
            loop {
                // Wait for notification or timeout
                tokio::select! {
                    _ = notify.notified() => {},
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
                }

                // Process tasks
                while let Some(task) = {
                    let mut q = queue.lock().await;
                    q.pop_front()
                } {
                    tracing::info!(
                        "Processing transcode task: video_id={}, resolutions={:?}",
                        task.video_id,
                        task.resolutions
                    );

                    let input_path = std::path::Path::new(&task.input_path);
                    match transcoder
                        .transcode(task.video_id, input_path, task.resolutions.clone())
                        .await
                    {
                        Ok(variants) => {
                            tracing::info!(
                                "Transcode completed: video_id={}, {} variants",
                                task.video_id,
                                variants.len()
                            );
                            // TODO: Save variants to database
                        }
                        Err(e) => {
                            tracing::error!(
                                "Transcode failed: video_id={}, error={}",
                                task.video_id,
                                e
                            );
                        }
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
