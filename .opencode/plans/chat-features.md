# 聊天室功能扩展（已确认方案）

状态：**待执行** —— 用户已批准，但当前会话处于 plan 模式（edit 权限被 deny），
需退出 plan 模式后按本计划实施。

## 需求（用户多选确认）
1. 表情 emoji 面板
2. 未读消息提醒（导航徽标）
3. @ 提及（补全 + 高亮）
4. 发送图片（用户自定补充）

## 实施步骤

### 1. 迁移 054（backend/migrations/054_add_chat_images.sql）
```sql
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS msg_type SMALLINT NOT NULL DEFAULT 0;
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS image_url VARCHAR(255);
```
msg_type: 0=文本、1=图片（content 为配文可为空，image_url=/media/chat/{file}）

### 2. 后端
- models/chat.rs：ChatMessageRow/ChatMessageItem/ChatEvent::Message 增加
  msg_type、image_url 字段（serde 命名 msgType/imageUrl，图片消息 skip 序列化 null）
- repositories/chat_repo.rs：insert 增加图片参数；list_paged 返回新列
- services/chat_service.rs：
  - send_message 增加 image_url 参数（msg_type 由调用方定）
  - admin_delete 时异步删除图片物理文件（safe_media_path 校验后 remove）
- handlers/chat.rs：
  - 新增 `POST /chat/image`（multipart）：≤10MB、magic bytes 校验
    jpg/png/webp/gif（复用 media_service::infer_image / validate），
    存 `media_root/chat/{uuid}.{ext}`，返回 `/media/chat/{file}`
  - ws handle_client_text 协议扩展：`{"type":"img","imageUrl":"..."}`，
    校验 image_url 必须以 `/media/chat/` 开头（防注入任意路径），
    发图计入发言限速
- app.rs：`POST /chat/image` 挂进 chat_routes（bearer+role1，30s 超时）
- middleware/auth/media.rs：`is_chat_public_path(path)` → `/media/chat/` 前缀
  已登录用户直接放行（在 Authorized 分支、owner 校验之前）；
  未登录仍 401（分享 token 不适用）
- openapi.rs + tests/openapi_route_tests.rs：
  - 新路由 POST /chat/messages/image 或 /chat/image（命名：/chat/image）
  - ChatMessageItem schema 加 msgType/imageUrl
  - /ws/chat 描述更新协议

### 3. 前端
- **api/chat.ts**：ChatMessage/ChatEvent 加 msgType/imageUrl；
  `uploadChatImage(file: File)`（multipart POST /chat/image）
- **新 ChatContext（全局单例）**：
  - 挂在 AuthProvider 内（依赖登录态），App 全程保持一条 WS 连接
  - 暴露：messages 状态？否——消息流仍归 Chat 页；context 只负责
    连接 + 事件分发订阅 + 未读计数
  - `useChatEvent(cb)` 订阅；未读逻辑：收到 message 事件且
    `location.pathname !== '/chat'` → unread+1；路由进入 /chat → 清零
- **Layout**：聊天室导航链接加未读徽标（红点+数字，桌面 + 移动抽屉）
- **Chat 页**：
  - 消费 ChatContext（不再自建连接）
  - emoji 面板：输入框左侧按钮，弹出 ~40 常用 emoji 分组，插入光标处
  - @ 补全：输入 @ 触发，浮层列在线名单（来自 online 事件），键盘/点击选择，
    选中后插入 `@用户名 `；渲染时 @用户名 高亮 span；含“我的用户名”的
    消息整条加左边框提醒
  - 图片：输入栏图片按钮 → `<input type=file accept=image/*>`（手机调相册）→
    uploadChatImage → 成功后 ws 发 `{"type":"img","imageUrl"}`；
    气泡内 img（max-height 200px，圆角），点击打开简单 lightbox（fixed 全屏遮罩）
- Chat.css 相应样式（面板、@高亮、图片气泡、lightboox、徽标）+ 移动端适配

### 4. 验证
- cargo fmt/clippy/test 全绿；integration_chat.rs 扩展：图片消息持久化、
  /chat/image 校验（超限/非图片拒绝）、media_auth /media/chat/ 分支单测
- 前端 build + vitest
- Playwright E2E：双端互发 emoji/@/图片；未读徽标计数（A 在首页收到 B 的
  消息 → 徽标 +1 → 进聊天室清零）；刷新后历史含图片消息

## 设计取舍（已确认）
- 图片 ≤10MB；聊天图片不进私人媒体库、不占配额、不出现在“我的作品”
- 未读计数会话内有效（刷新归零）
- /media/chat/* 登录即可见（含访客），未登录不可见
