use serde::Serialize;
use utoipa::ToSchema;

use crate::util::hashid_serde::{serialize_id, serialize_option_id};

/// 单条聊天消息（DB 行 + 展示字段）
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ChatMessageRow {
    pub id: i64,
    pub user_id: Option<i64>,
    pub username: String,
    pub content: String,
    /// 0=文本、1=图片、2=视频（迁移 054/055）
    pub msg_type: i16,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 待持久化的一条消息（文本/图片/视频）。
#[derive(Debug, Clone, Copy)]
pub struct NewChatMessage<'a> {
    /// 文本内容（图片/视频消息为配文，可为空）
    pub content: &'a str,
    /// 0=文本、1=图片、2=视频
    pub msg_type: i16,
    pub image_url: Option<&'a str>,
    pub video_url: Option<&'a str>,
}

/// 历史分页响应（id 游标，倒序返回，前端自行反转拼接）
#[derive(Debug, Serialize, ToSchema)]
pub struct ChatHistoryResponse {
    pub items: Vec<ChatMessageItem>,
    /// 是否还有更早的消息
    pub has_more: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatMessageItem {
    pub id: i64,
    /// hashid 序列化（与 /auth/user 等一致，前端用于"我的消息"比对）
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_option_id")]
    #[serde(rename = "userId")]
    #[schema(value_type = Option<String>, example = "k1a2b3c4")]
    pub user_id: Option<i64>,
    pub username: String,
    /// 发言者是否为访客影子账号（前端显示"访客"徽标）
    #[serde(rename = "isGuest")]
    pub is_guest: bool,
    pub content: String,
    /// 0=文本、1=图片、2=视频
    #[serde(rename = "msgType")]
    pub msg_type: i16,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "videoUrl")]
    pub video_url: Option<String>,
    #[serde(rename = "ts")]
    pub created_at: String,
}

/// WS 下行事件（serde_json 序列化为文本帧）
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// 新消息（含发送者信息；userId 为 hashid，与 /auth/user 一致）
    Message {
        id: i64,
        #[serde(rename = "userId")]
        #[serde(serialize_with = "serialize_id")]
        user_id: i64,
        username: String,
        #[serde(rename = "isGuest")]
        is_guest: bool,
        content: String,
        #[serde(rename = "msgType")]
        msg_type: i16,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "imageUrl")]
        image_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "videoUrl")]
        video_url: Option<String>,
        ts: String,
    },
    /// 在线成员变化
    Online {
        count: usize,
        /// 成员名列表（上限 50，防止刷屏）
        names: Vec<String>,
    },
    /// 管理员删除了某条消息
    Deleted { id: i64 },
    /// 管理员清空了整个聊天室
    Cleared,
    /// 服务端错误提示（限速/被删言等）
    Error { message: String },
}
