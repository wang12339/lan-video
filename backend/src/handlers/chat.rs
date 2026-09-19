use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;

use crate::middleware::auth::AuthUser;
use crate::models::chat::{ChatEvent, ChatHistoryResponse};
use crate::services::chat_service::{
    ChatPayload, ChatService, CHAT_IMAGE_MAX_BYTES, CHAT_MEDIA_PREFIX, CHAT_VIDEO_MAX_BYTES,
    HISTORY_PAGE_LIMIT, MSG_TYPE_IMAGE, MSG_TYPE_VIDEO,
};
use crate::state::AppState;
use crate::util::response::{error_response, internal_error_log, ErrorResponse};

/// WS 心跳间隔：低于常见反代/LB 的 60s 空闲超时
const WS_PING_INTERVAL_SECS: u64 = 25;
/// 单帧上限（聊天消息 ≤500 字节，64KB 余量足够并防滥用帧）
const WS_MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
pub struct ChatHistoryQuery {
    pub before_id: Option<i64>,
    pub limit: Option<i64>,
}

/// GET /chat/messages — 历史分页（id 倒序游标，前端反转拼接）
pub async fn get_chat_history(
    State(state): State<Arc<AppState>>,
    Extension(_auth_user): Extension<AuthUser>,
    Query(q): Query<ChatHistoryQuery>,
) -> Result<Json<ChatHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = q.limit.unwrap_or(HISTORY_PAGE_LIMIT).clamp(1, 100);
    let resp = ChatService::from_state(&state)
        .history(q.before_id, limit)
        .await
        .map_err(|e| internal_error_log("chat_history", &e))?;
    Ok(Json(resp))
}

/// GET /ws/chat — 公共聊天室 WebSocket（cookie/bearer 均可，role ≥ 1）
///
/// 协议（JSON 文本帧）：
/// - 客户端 → 服务端：`{"type":"msg","content":"..."}`
/// - 服务端 → 客户端：`{"type":"message",...}` / `{"type":"online",...}` /
///   `{"type":"deleted",...}` / `{"type":"error","message":"..."}`
pub async fn ws_chat(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.max_frame_size(WS_MAX_FRAME_BYTES)
        .max_message_size(WS_MAX_FRAME_BYTES)
        .on_upgrade(move |socket| run_chat_socket(socket, state, auth_user))
}

async fn run_chat_socket(
    socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
    user: AuthUser,
) {
    let username = user.username.clone();
    let chat = ChatService::from_state(&state);

    let mut rx = state.chat_hub.join(user.id, &username);
    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(WS_PING_INTERVAL_SECS));
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            // ── 房间广播 → 客户端 ──
            ev = rx.recv() => match ev {
                Ok(event) => {
                    let Ok(text) = serde_json::to_string(&*event) else { continue };
                    if ws_tx
                        .send(axum::extract::ws::Message::Text(text.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                // 慢消费者：丢弃错过的实时事件（历史以 DB 为准），不断连
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },

            // ── 客户端 → 房间 ──
            frame = ws_rx.next() => match frame {
                Some(Ok(axum::extract::ws::Message::Text(text))) => {
                    let reply =
                        handle_client_text(&chat, &user, text.as_str()).await;
                    let Ok(reply_text) = serde_json::to_string(&reply) else { continue };
                    if ws_tx
                        .send(axum::extract::ws::Message::Text(reply_text.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Some(Ok(axum::extract::ws::Message::Ping(p))) => {
                    if ws_tx
                        .send(axum::extract::ws::Message::Pong(p))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Some(Ok(axum::extract::ws::Message::Close(_))) | None => break,
                // 二进制帧/其他：聊天协议只收文本，忽略
                _ => {}
            },

            // ── 保活 ──
            _ = ping.tick() => {
                if ws_tx
                    .send(axum::extract::ws::Message::Ping(Vec::new().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    state.chat_hub.leave(user.id);
}

/// 客户端 WS 文本帧协议
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientFrame {
    /// 文本消息
    Msg { content: String },
    /// 图片消息（先经 POST /chat/image 上传拿到 imageUrl）
    Img {
        #[serde(rename = "imageUrl")]
        image_url: String,
    },
    /// 视频消息（先经 POST /chat/video 上传拿到 videoUrl）
    Video {
        #[serde(rename = "videoUrl")]
        video_url: String,
    },
}

/// 处理一条客户端文本帧，返回回给**该客户端自己**的事件
/// （成功时房间广播已由 send_message 完成，这里只回错误提示）。
async fn handle_client_text(chat: &ChatService, user: &AuthUser, text: &str) -> ChatEvent {
    let frame: ClientFrame = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(_) => {
            return ChatEvent::Error {
                message: "请求格式错误".into(),
            }
        }
    };
    // 发图与发文本共用发言限速
    if let Err(e) = chat.check_rate_limit(user.id).await {
        return match e {
            crate::util::error::ServiceError::RateLimited => ChatEvent::Error {
                message: "发言太频繁，请稍后再试".into(),
            },
            _ => ChatEvent::Error {
                message: "发送失败，请重试".into(),
            },
        };
    }
    match frame {
        ClientFrame::Msg { content } => {
            let content = match ChatService::validate_content(&content) {
                Ok(c) => c,
                Err(e) => {
                    return ChatEvent::Error {
                        message: e.to_string(),
                    }
                }
            };
            match chat
                .send_message(
                    user.id,
                    &user.username,
                    user.is_guest,
                    ChatPayload {
                        content: &content,
                        msg_type: 0,
                        image_url: None,
                        video_url: None,
                    },
                )
                .await
            {
                Ok(event) => event,
                Err(_) => ChatEvent::Error {
                    message: "发送失败，请重试".into(),
                },
            }
        }
        ClientFrame::Img { image_url } => {
            let url = match ChatService::validate_media_url(&image_url) {
                Ok(u) => u,
                Err(e) => {
                    return ChatEvent::Error {
                        message: e.to_string(),
                    }
                }
            };
            match chat
                .send_message(
                    user.id,
                    &user.username,
                    user.is_guest,
                    ChatPayload {
                        content: "",
                        msg_type: MSG_TYPE_IMAGE,
                        image_url: Some(&url),
                        video_url: None,
                    },
                )
                .await
            {
                Ok(event) => event,
                Err(_) => ChatEvent::Error {
                    message: "发送失败，请重试".into(),
                },
            }
        }
        ClientFrame::Video { video_url } => {
            let url = match ChatService::validate_media_url(&video_url) {
                Ok(u) => u,
                Err(e) => {
                    return ChatEvent::Error {
                        message: e.to_string(),
                    }
                }
            };
            match chat
                .send_message(
                    user.id,
                    &user.username,
                    user.is_guest,
                    ChatPayload {
                        content: "",
                        msg_type: MSG_TYPE_VIDEO,
                        image_url: None,
                        video_url: Some(&url),
                    },
                )
                .await
            {
                Ok(event) => event,
                Err(_) => ChatEvent::Error {
                    message: "发送失败，请重试".into(),
                },
            }
        }
    }
}

/// POST /chat/image — 聊天图片上传
///
/// ≤10MB，magic bytes 校验（jpg/png/webp/gif），存 `media_root/chat/{uuid}.{ext}`，
/// 返回 `/media/chat/{file}`。计入发言限速（发图也占一条消息额度）。
pub async fn upload_chat_image(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // 发图计入限速，防刷图滥用
    let chat = ChatService::from_state(&state);
    if let Err(_e) = chat.check_rate_limit(auth_user.id).await {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "发言太频繁，请稍后再试",
        ));
    }

    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "无效的请求格式"))?
    {
        if field.name() == Some("file") {
            // 读取时带上限：超出即拒（+1 slack 让 over-size 报错而不是截断）
            let bytes = field
                .bytes()
                .await
                .map_err(|_| error_response(StatusCode::BAD_REQUEST, "读取文件失败"))?;
            if bytes.len() as u64 > CHAT_IMAGE_MAX_BYTES {
                return Err(error_response(StatusCode::BAD_REQUEST, "图片不能超过 10MB"));
            }
            data = Some(bytes.to_vec());
        }
    }
    let data = data.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "请选择要上传的图片"))?;

    // magic bytes 校验（不信任 Content-Type）
    let (ext, _mime) = crate::services::media_service::infer_image(&data).ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "不支持的图片格式，请上传 JPG/PNG/WebP/GIF",
        )
    })?;

    let chat_dir = state.config.media_root.join("chat");
    tokio::fs::create_dir_all(&chat_dir).await.map_err(|e| {
        tracing::error!("create chat dir failed: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
    })?;

    let file_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
    let path = chat_dir.join(&file_name);
    tokio::fs::write(&path, &data).await.map_err(|e| {
        tracing::error!("write chat image failed: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "保存文件失败")
    })?;

    let url = format!("{}{}", CHAT_MEDIA_PREFIX, file_name);
    Ok(Json(serde_json::json!({ "ok": true, "imageUrl": url })))
}

/// POST /chat/video — 聊天视频上传
///
/// ≤50MB，接受 MP4/M4V/MOV/WebM/MKV/AVI（magic bytes 校验），流式写盘。
/// 非 WebM 统一转为浏览器可播的 H.264/AAC MP4（H.264 源仅 remux，不重编码）；
/// 输出存 `media_root/chat/{uuid}.{mp4|webm}`，返回 `/media/chat/{file}`。
/// 计入发言限速（发视频也占一条消息额度）。
pub async fn upload_chat_video(
    State(state): State<Arc<AppState>>,
    Extension(auth_user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let chat = ChatService::from_state(&state);
    if let Err(_e) = chat.check_rate_limit(auth_user.id).await {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "发言太频繁，请稍后再试",
        ));
    }

    let chat_dir = state.config.media_root.join("chat");
    tokio::fs::create_dir_all(&chat_dir).await.map_err(|e| {
        tracing::error!("create chat dir failed: {}", e);
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
    })?;

    // 找到 file 字段：解析扩展名 → 流式写入临时文件（限制总大小）
    let mut uploaded: Option<(std::path::PathBuf, String)> = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "无效的请求格式"))?
    {
        if field.name() != Some("file") {
            continue;
        }
        // 扩展名白名单（真正格式再由 magic bytes 校验兜底）
        let raw_name = field.file_name().unwrap_or("video.mp4").to_string();
        let ext = std::path::Path::new(&raw_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !crate::services::chat_video::ALLOWED_INPUT_EXTS.contains(&ext.as_str()) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "不支持的视频格式，请上传 MP4/MOV/M4V/WebM/MKV/AVI",
            ));
        }

        let temp_path = chat_dir.join(format!(".upload_{}.{}", uuid::Uuid::new_v4().simple(), ext));
        let mut file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
            tracing::error!("create chat video temp failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
        })?;
        let mut written: u64 = 0;
        loop {
            // 每次最多读 1MB，边读边写，避免大文件整段进内存
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    written += chunk.len() as u64;
                    if written > CHAT_VIDEO_MAX_BYTES {
                        drop(file);
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        return Err(error_response(StatusCode::BAD_REQUEST, "视频不能超过 50MB"));
                    }
                    if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await {
                        tracing::error!("write chat video failed: {}", e);
                        drop(file);
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        return Err(error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "保存文件失败",
                        ));
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Err(error_response(StatusCode::BAD_REQUEST, "读取文件失败"));
                }
            }
        }
        if let Err(e) = tokio::io::AsyncWriteExt::flush(&mut file).await {
            tracing::error!("flush chat video failed: {}", e);
        }
        drop(file);

        if written == 0 {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "请选择要上传的视频",
            ));
        }

        // magic bytes 校验（不信任扩展名/Content-Type），spawn_blocking 避免阻塞
        let path_check = temp_path.clone();
        let ext_check = ext.clone();
        let valid = tokio::task::spawn_blocking(move || {
            crate::services::media_service::validate_file_type(&path_check, &ext_check)
        })
        .await
        .unwrap_or_else(|_| Err("校验任务失败".into()));
        if let Err(e) = valid {
            let _ = tokio::fs::remove_file(&temp_path).await;
            tracing::warn!("chat video validation rejected: {}", e);
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "视频格式无效或文件已损坏",
            ));
        }

        uploaded = Some((temp_path, ext));
        break;
    }

    let (temp_path, ext) =
        uploaded.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "请选择要上传的视频"))?;

    // 统一转成浏览器可播格式：WebM 原样保留，其余转 H.264/AAC MP4
    let ffmpeg = state.config.ffmpeg_path.clone();
    let ffprobe = state.config.ffprobe_path.clone();
    let input = temp_path.clone();
    let out_dir = chat_dir.clone();
    let converted =
        crate::services::chat_video::prepare_video(&input, &ext, &out_dir, &ffmpeg, &ffprobe).await;

    let file_name = match converted {
        // 无需转换（WebM）：把临时文件改名为正式文件
        Ok(None) => {
            let file_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), ext);
            let final_path = chat_dir.join(&file_name);
            if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
                tracing::error!("finalize chat webm failed: {}", e);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "保存文件失败",
                ));
            }
            file_name
        }
        // 已转换出 MP4：删除原始临时文件
        Ok(Some(name)) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            name
        }
        Err(reason) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(error_response(StatusCode::BAD_REQUEST, reason));
        }
    };

    let url = format!("{}{}", CHAT_MEDIA_PREFIX, file_name);
    Ok(Json(serde_json::json!({ "ok": true, "videoUrl": url })))
}

/// DELETE /admin/chat/messages/{id} — 管理员删言（并广播让在线客户端移除）
pub async fn admin_delete_chat_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if id <= 0 {
        return Err(error_response(StatusCode::BAD_REQUEST, "无效的消息ID"));
    }
    let deleted = ChatService::from_state(&state)
        .admin_delete(id)
        .await
        .map_err(|e| internal_error_log("admin_delete_chat_message", &e))?;
    if !deleted {
        return Err(error_response(StatusCode::NOT_FOUND, "消息不存在"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /admin/chat/stats — 聊天室消息统计（管理后台）
pub async fn admin_chat_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let count = ChatService::from_state(&state)
        .stats()
        .await
        .map_err(|e| internal_error_log("admin_chat_stats", &e))?;
    Ok(Json(serde_json::json!({ "ok": true, "count": count })))
}

/// DELETE /admin/chat/messages — 管理员清空聊天室（并广播 Cleared 让在线客户端清屏）
pub async fn admin_clear_chat_messages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let deleted = ChatService::from_state(&state)
        .admin_clear()
        .await
        .map_err(|e| internal_error_log("admin_clear_chat_messages", &e))?;
    Ok(Json(serde_json::json!({ "ok": true, "deleted": deleted })))
}
