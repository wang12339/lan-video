use std::sync::Arc;

use crate::middleware::rate_limit::RateLimiter;
use crate::models::chat::{ChatEvent, ChatHistoryResponse, ChatMessageItem, NewChatMessage};
use crate::repositories::chat_repo::ChatRepository;
use crate::state::{AppState, ChatHub};
use crate::util::error::ServiceError;

/// 单条消息长度上限（字节，UTF-8）
pub const MAX_CONTENT_BYTES: usize = 500;
/// 聊天图片大小上限
pub const CHAT_IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;
/// 聊天视频大小上限（短片段）
pub const CHAT_VIDEO_MAX_BYTES: u64 = 50 * 1024 * 1024;
/// 图片消息 msg_type
pub const MSG_TYPE_IMAGE: i16 = 1;
/// 视频消息 msg_type
pub const MSG_TYPE_VIDEO: i16 = 2;
/// 聊天媒体（图片/视频）必须存放在此前缀下（防注入任意路径做删除/展示）
pub const CHAT_MEDIA_PREFIX: &str = "/media/chat/";
/// 发言速率限制：每用户每 10 秒最多 5 条（含图片/视频；访客与正式用户同限）。
/// 注意 RateLimiter 语义为"最多放行 max_attempts-1 次"，故传 6。
const RATE_LIMIT_MAX: u32 = 6;
const RATE_LIMIT_WINDOW_SECS: u64 = 10;
/// 单次历史拉取上限
pub const HISTORY_PAGE_LIMIT: i64 = 50;

/// 聊天室业务逻辑。轻量克隆（repo/limiter/hub 均为 Arc），
/// 由 handler 通过 [`ChatService::from_state`] 按需构造。
#[derive(Clone)]
pub struct ChatService {
    repo: ChatRepository,
    rate_limiter: RateLimiter,
    hub: Arc<ChatHub>,
    media_root: std::path::PathBuf,
}

impl ChatService {
    pub fn from_state(state: &Arc<AppState>) -> Self {
        Self {
            repo: state.repos.chat.clone(),
            rate_limiter: state.rate_limiter.clone(),
            hub: state.chat_hub.clone(),
            media_root: state.config.media_root.clone(),
        }
    }

    pub fn new(
        repo: ChatRepository,
        rate_limiter: RateLimiter,
        hub: Arc<ChatHub>,
        media_root: std::path::PathBuf,
    ) -> Self {
        Self {
            repo,
            rate_limiter,
            hub,
            media_root,
        }
    }

    /// 校验内容合法性（长度、空白）。返回 trim 后的内容。
    pub fn validate_content(raw: &str) -> Result<String, ServiceError> {
        let content = raw.trim();
        if content.is_empty() {
            return Err(ServiceError::BadRequest("消息内容不能为空".into()));
        }
        if content.len() > MAX_CONTENT_BYTES {
            return Err(ServiceError::BadRequest(format!(
                "消息不能超过 {} 个字符",
                MAX_CONTENT_BYTES
            )));
        }
        // 控制字符防日志注入（与用户名校验同策略）；保留 \n/\t 供多行输入
        if content
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(ServiceError::BadRequest("消息包含非法字符".into()));
        }
        Ok(content.to_string())
    }

    /// 发言速率限制（按 user_id）。
    pub async fn check_rate_limit(&self, user_id: i64) -> Result<(), ServiceError> {
        let key = format!("chat:user:{}", user_id);
        if self
            .rate_limiter
            .check_with(&key, RATE_LIMIT_MAX, RATE_LIMIT_WINDOW_SECS, 0)
            .await
            .is_err()
        {
            return Err(ServiceError::RateLimited);
        }
        Ok(())
    }
}

/// 一条待发送的消息载荷（文本/图片/视频）。
pub type ChatPayload<'a> = NewChatMessage<'a>;

impl ChatService {
    /// 校验媒体消息（图片/视频）的 URL：必须位于 /media/chat/ 前缀下
    /// 且不含路径穿越。
    pub fn validate_media_url(url: &str) -> Result<String, ServiceError> {
        let url = url.trim();
        let Some(rel) = url.strip_prefix(CHAT_MEDIA_PREFIX) else {
            return Err(ServiceError::BadRequest("媒体地址无效".into()));
        };
        // 仅允许安全文件名：字母数字/-_ 和扩展名，禁止穿越段
        if rel.is_empty()
            || rel.contains("..")
            || !rel
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
        {
            return Err(ServiceError::BadRequest("媒体地址无效".into()));
        }
        Ok(url.to_string())
    }

    /// 持久化并广播一条消息。返回广播用的事件。
    pub async fn send_message(
        &self,
        tenant_id: i64,
        user_id: i64,
        username: &str,
        is_guest: bool,
        payload: ChatPayload<'_>,
    ) -> Result<ChatEvent, ServiceError> {
        let row = self
            .repo
            .insert(tenant_id, user_id, username, payload)
            .await?;
        let event = ChatEvent::Message {
            id: row.id,
            user_id,
            username: username.to_string(),
            is_guest,
            content: row.content,
            msg_type: row.msg_type,
            image_url: row.image_url,
            video_url: row.video_url,
            ts: row.created_at.to_rfc3339(),
        };
        self.hub.broadcast(tenant_id, event.clone());
        Ok(event)
    }

    /// 历史分页（倒序游标）。
    pub async fn history(
        &self,
        tenant_id: i64,
        before_id: Option<i64>,
        limit: i64,
    ) -> Result<ChatHistoryResponse, ServiceError> {
        let rows = self.repo.list_paged(tenant_id, before_id, limit).await?;
        let has_more = rows.len() as i64 >= limit;
        let items = rows
            .into_iter()
            .map(|r| {
                let is_guest = r.username.starts_with("guest_");
                ChatMessageItem {
                    id: r.id,
                    user_id: r.user_id,
                    // 访客影子账号固定 guest_ 前缀（create_guest_user 生成），
                    // 普通用户注册校验不拦此前缀属已知可仿冒面，展示层风险可接受
                    is_guest,
                    username: r.username,
                    content: r.content,
                    msg_type: r.msg_type,
                    image_url: r.image_url,
                    video_url: r.video_url,
                    created_at: r.created_at.to_rfc3339(),
                }
            })
            .collect();
        Ok(ChatHistoryResponse { items, has_more })
    }

    /// 管理员删除消息，并广播删除事件让在线客户端移除。
    /// 图片/视频消息同步尽力而为地清理物理文件。
    pub async fn admin_delete(&self, tenant_id: i64, id: i64) -> Result<bool, ServiceError> {
        // 先取行（拿媒体 URL）再删除
        let row = self.repo.find_by_id(tenant_id, id).await?;
        let deleted = self.repo.delete(tenant_id, id).await?;
        if deleted {
            if let Some(row) = row {
                let media_urls = [row.image_url, row.video_url];
                let media_root = self.media_root.clone();
                tokio::task::spawn_blocking(move || {
                    for url in media_urls.into_iter().flatten() {
                        if let Some(path) =
                            crate::services::media_service::safe_media_path(&url, &media_root)
                        {
                            if path.starts_with(media_root.join("chat")) {
                                let _ = std::fs::remove_file(path);
                            }
                        }
                    }
                });
            }
            self.hub.broadcast(tenant_id, ChatEvent::Deleted { id });
        }
        Ok(deleted)
    }

    /// 本租户消息总数（管理后台展示用）。
    pub async fn stats(&self, tenant_id: i64) -> Result<i64, ServiceError> {
        Ok(self.repo.count(tenant_id).await?)
    }

    /// 管理员清空本租户聊天室。返回删除条数；有删除时广播 Cleared
    /// 让所有在线客户端立即清空消息列表。
    pub async fn admin_clear(&self, tenant_id: i64) -> Result<u64, ServiceError> {
        let deleted = self.repo.delete_all(tenant_id).await?;
        if deleted > 0 {
            self.hub.broadcast(tenant_id, ChatEvent::Cleared);
        }
        Ok(deleted)
    }
}
