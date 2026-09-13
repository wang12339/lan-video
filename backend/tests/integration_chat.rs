#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! 公共聊天室集成测试：内容校验、限速、持久化/分页、管理员删言。
//! 需要 `DATABASE_URL`，否则整体跳过。

mod integration_test_helpers;

use atmos_video_backend::services::chat_service::{ChatPayload, ChatService};
use integration_test_helpers::*;

#[tokio::test]
async fn chat_validate_content_edges() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    // 空白
    assert!(ChatService::validate_content("   ").is_err());
    // 正常
    assert_eq!(
        ChatService::validate_content("  hello \n world ").unwrap(),
        "hello \n world"
    );
    // 换行/制表符保留
    assert!(ChatService::validate_content("a\nb\tc").is_ok());
    // 控制字符拒绝
    assert!(ChatService::validate_content("a\u{0}b").is_err());
    assert!(ChatService::validate_content("a\u{7}b").is_err());
    // 超长（>500 字节；中文 UTF-8 每字 3 字节，167 字 = 501 字节）
    let long = "好".repeat(167);
    assert!(ChatService::validate_content(&long).is_err());
    // 恰好 ≤500 字节（166 字 = 498 字节）
    let ok_len = "好".repeat(166);
    assert!(ChatService::validate_content(&ok_len).is_ok());
}

#[tokio::test]
async fn chat_send_history_delete() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let chat = ChatService::from_state(&state);

    // chat_messages.user_id 有 users 外键，必须用真实用户
    let (username, user_id) = create_test_user(&state, "chat_user").await;

    // 发三条消息
    let e1 = chat
        .send_message(
            1,
            user_id,
            &username,
            false,
            ChatPayload {
                content: "第一条",
                msg_type: 0,
                image_url: None,
                video_url: None,
            },
        )
        .await
        .unwrap();
    chat.send_message(
        1,
        user_id,
        &username,
        false,
        ChatPayload {
            content: "第二条",
            msg_type: 0,
            image_url: None,
            video_url: None,
        },
    )
    .await
    .unwrap();
    let e3 = chat
        .send_message(
            1,
            user_id,
            &username,
            false,
            ChatPayload {
                content: "第三条",
                msg_type: 0,
                image_url: None,
                video_url: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        e1,
        atmos_video_backend::models::chat::ChatEvent::Message { .. }
    ));

    // 历史分页：倒序、游标
    let page1 = chat.history(1, None, 2).await.unwrap();
    assert_eq!(page1.items.len(), 2);
    assert!(page1.has_more, "取 2 条应提示还有更多");
    assert!(page1.items[0].id > page1.items[1].id, "倒序");
    assert_eq!(page1.items[0].content, "第三条");

    let page2 = chat.history(1, Some(page1.items[1].id), 2).await.unwrap();
    assert!(page2.items.iter().any(|i| i.content == "第一条"));

    // 管理员删除 → 广播 Deleted + 再次分页不再出现
    let deleted_id = e3.message_id();
    let ok = chat.admin_delete(1, deleted_id).await.unwrap();
    assert!(ok, "删除应成功");
    let after = chat.history(1, None, 100).await.unwrap();
    assert!(
        !after.items.iter().any(|i| i.id == deleted_id),
        "删除后不应再出现在历史中"
    );
    // 幂等：重复删除返回 false
    assert!(!chat.admin_delete(1, deleted_id).await.unwrap());

    // 清理
    sqlx::query("DELETE FROM chat_messages WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .unwrap();
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

/// 给 ChatEvent 加个取消息 id 的小助手（测试用）
trait ChatEventExt {
    fn message_id(&self) -> i64;
}
impl ChatEventExt for atmos_video_backend::models::chat::ChatEvent {
    fn message_id(&self) -> i64 {
        match self {
            atmos_video_backend::models::chat::ChatEvent::Message { id, .. } => *id,
            _ => panic!("expected Message event"),
        }
    }
}

#[tokio::test]
async fn chat_image_message_persistence() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let chat = ChatService::from_state(&state);

    // validate_image_url：仅 /media/chat/ 前缀 + 安全文件名
    assert_eq!(
        ChatService::validate_media_url("/media/chat/abc-123.jpg").unwrap(),
        "/media/chat/abc-123.jpg"
    );
    for bad in [
        "/media/videos/x.jpg",
        "/media/avatars/x.png",
        "https://evil.com/x.png",
        "/media/chat/../secret.png",
        "/media/chat/",
        "",
    ] {
        assert!(
            ChatService::validate_media_url(bad).is_err(),
            "应拒绝非法图片地址: {bad}"
        );
    }

    let (username, user_id) = create_test_user(&state, "chatimg").await;

    // 图片消息：content 为空配文 + msgType=1 + imageUrl
    let e = chat
        .send_message(
            1,
            user_id,
            &username,
            false,
            ChatPayload {
                content: "",
                msg_type: 1,
                image_url: Some("/media/chat/abc-123.jpg"),
                video_url: None,
            },
        )
        .await
        .unwrap();
    let text = chat
        .send_message(
            1,
            user_id,
            &username,
            false,
            ChatPayload {
                content: "配文一条",
                msg_type: 0,
                image_url: None,
                video_url: None,
            },
        )
        .await
        .unwrap();

    let page = chat.history(1, None, 100).await.unwrap();
    let img = page
        .items
        .iter()
        .find(|i| i.id == e.message_id())
        .expect("图片消息应在历史中");
    assert_eq!(img.msg_type, 1);
    assert_eq!(img.image_url.as_deref(), Some("/media/chat/abc-123.jpg"));
    assert_eq!(img.content, "");
    let txt = page
        .items
        .iter()
        .find(|i| i.id == text.message_id())
        .expect("文本消息应在历史中");
    assert_eq!(txt.msg_type, 0);
    assert_eq!(txt.image_url, None);

    // 管理员删除图片消息（media_root/chat 下无实际文件也不应报错）
    let ok = chat.admin_delete(1, e.message_id()).await.unwrap();
    assert!(ok);

    // 清理
    sqlx::query("DELETE FROM chat_messages WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .unwrap();
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn chat_video_message_persistence() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let chat = ChatService::from_state(&state);

    // validate_media_url 同样用于视频（仅 /media/chat/ 前缀 + 安全文件名）
    assert_eq!(
        ChatService::validate_media_url("/media/chat/clip-ab12.webm").unwrap(),
        "/media/chat/clip-ab12.webm"
    );
    assert!(ChatService::validate_media_url("/media/chat/../x.mp4").is_err());
    assert!(ChatService::validate_media_url("/media/videos/x.mp4").is_err());

    let (username, user_id) = create_test_user(&state, "chatvid").await;
    let e = chat
        .send_message(
            1,
            user_id,
            &username,
            false,
            ChatPayload {
                content: "",
                msg_type: 2,
                image_url: None,
                video_url: Some("/media/chat/clip-ab12.webm"),
            },
        )
        .await
        .unwrap();

    let page = chat.history(1, None, 100).await.unwrap();
    let vid = page
        .items
        .iter()
        .find(|i| i.id == e.message_id())
        .expect("视频消息应在历史中");
    assert_eq!(vid.msg_type, 2);
    assert_eq!(vid.video_url.as_deref(), Some("/media/chat/clip-ab12.webm"));
    assert_eq!(vid.image_url, None);

    // 管理员删除（chat 目录下无实际文件也不应报错）
    assert!(chat.admin_delete(1, e.message_id()).await.unwrap());

    // 清理
    sqlx::query("DELETE FROM chat_messages WHERE user_id = $1")
        .bind(user_id)
        .execute(state.repos.video.pool())
        .await
        .unwrap();
    cleanup_test_user(state.repos.video.pool(), &username).await;
}

#[tokio::test]
async fn chat_admin_clear_all() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let chat = ChatService::from_state(&state);

    // 用专用租户，避免 admin_clear 清掉真实聊天数据
    let pool = state.repos.video.pool();
    let (tenant_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO tenants (name, slug) VALUES ('__test_chat_clear', '__test-chat-clear') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let (username, user_id) = create_test_user(&state, "chatclr").await;
    for c in ["甲", "乙", "丙"] {
        chat.send_message(
            tenant_id,
            user_id,
            &username,
            false,
            ChatPayload {
                content: c,
                msg_type: 0,
                image_url: None,
                video_url: None,
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(chat.stats(tenant_id).await.unwrap(), 3);

    // 清空 → 返回删除数 3，广播 Cleared，统计归零
    let deleted = chat.admin_clear(tenant_id).await.unwrap();
    assert_eq!(deleted, 3);
    assert_eq!(chat.stats(tenant_id).await.unwrap(), 0);
    assert_eq!(
        chat.history(tenant_id, None, 100)
            .await
            .unwrap()
            .items
            .len(),
        0
    );

    // 空聊天室重复清空：0 条、不报错
    assert_eq!(chat.admin_clear(tenant_id).await.unwrap(), 0);

    // 清理（chat_messages 随租户级联删除）
    sqlx::query("DELETE FROM tenants WHERE id = $1")
        .bind(tenant_id)
        .execute(pool)
        .await
        .unwrap();
    cleanup_test_user(pool, &username).await;
}

#[tokio::test]
async fn chat_rate_limit_per_user() {
    let Some(_) = database_url() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    let state = test_app_state().await;
    let chat = ChatService::from_state(&state);

    // 限速器独立于消息表：同一 user 连发 5 次后第 6 次被拒
    let uid = 987654;
    for i in 0..5 {
        assert!(
            chat.check_rate_limit(uid).await.is_ok(),
            "第 {} 次应放行",
            i + 1
        );
    }
    assert!(chat.check_rate_limit(uid).await.is_err(), "第 6 次应限速");
    // 其他用户不受影响
    assert!(chat.check_rate_limit(uid + 1).await.is_ok());
}
