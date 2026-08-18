# Changelog

## [0.4.0] - 2026-08-13

### 新增
- 搜索建议前缀查询改用 `title ILIKE` + GIN trigram 索引，个性化推荐改为两段式查询（迁移 040）
- 静态资源缓存策略：`/webapp/assets/*` immutable 一年缓存，`index.html` 与 SPA 路由保持 no-cache
- 前端：Gallery 无限追加守卫与错误重试、灯箱无障碍、AuthDialog 焦点陷阱与滚动锁定、评论区 react-query 缓存 + 乐观更新 + 重试、LogsTab 轮询静默 + 页面隐藏暂停、viewport 允许缩放

### 安全
- 媒体访问授权链加固：`/media/variants/*` 需有效播放会话或分享令牌，`/media` 未登记路径一律 403，头像路径白名单放行
- 分享链接所有权校验：仅视频上传者或 admin 可创建分享，其余返回 403
- 转码管线路径逃逸修复（统一 `safe_media_path`）、ffmpeg 输出大小上限、stderr 截断，转码任务队列接入（重试/死信全链路生效）
- `X-Forwarded-Proto` 仅信任 Cloudflare 网段或 `TRUSTED_PROXY`；`HASHID_SALT` 未设置时启动告警
- 上传临时文件 24h TTL 清扫 + 上传锁清理 + fsync 原子落盘 + category 校验
- token 认证缓存（10s TTL，登出时精确失效）

### 性能
- 推荐查询两段式重构（EXPLAIN 13.4ms → 0.28ms）、搜索建议 trigram 索引（5.4ms → 0.21ms）（迁移 040）
- 静态资源 immutable 一年缓存，减少回源
- 播放器 4Hz 重渲染隔离到 controls 子树、`beforeunload` keepalive 进度上报、autoPlay 偏好接入

### 修复
- 标签重名冲突返回 409（替代静默失败）
- 播放列表删除条目后 position 空洞压缩
- 前端双缓存失效统一规则表（`client.ts` 失效入口）、埋点去重、prefs 异常防护

### 测试
- 后端新增：标签 409、播放列表 position 压缩、media 授权、分享非所有者 403、security 响应头、上传临时文件清扫、category 校验等
- 前端 Vitest 用例新增评论工具、确认弹窗等（累计 116 个）

## [0.3.0] - 2026-08-13

### 新增
- 多租户支持：`tenants` 表及 14 张业务表 `tenant_id` 列（迁移 033、034）
- 邮箱绑定与密码重置（迁移 035）
- 邮箱验证与验证令牌（迁移 036）
- 热门/趋势推荐：`trending_score` 预计算与索引（迁移 037）
- 邮箱唯一约束（迁移 038）
- 前端 i18n（zh-CN / en-US）与组件化重构

### 安全
- 修复关键认证绕过漏洞：`password::verify()` 恒返回成功导致任意密码可登录
- `/admin/track` 审计端点从仅管理员改为任意已认证用户可调用（缩小管理员路由暴露面）

### 性能
- 视频列表复合索引：`source_type`/`category` + `views` + `id`（迁移 032）
- 热门推荐改为按预计算的 `trending_score` 排序，消除运行时计算（迁移 037）
- 补充 `videos(created_at)` 索引支撑最新视频查询（迁移 039）
- 视频列表缓存（60 秒 TTL）与推荐缓存（120 秒 TTL）
- 前端重构：组件提取与渲染性能优化

### 修复
- 全文搜索词典不一致：触发器与查询端统一使用 `simple` 词典并回填 `search_vector`（迁移 039）
- 历史 `auth_tokens.tenant_id` 按所属用户回填（迁移 039）

### 测试
- 新增路由注册与 OpenAPI 文档一致性测试（`backend/tests/openapi_route_tests.rs`）
- 新增 service 单元测试与 HTTP 集成测试（`http_integration`、`integration_auth`、`integration_videos` 等）
- 前端引入 Vitest 单元测试（`webapp/src/test/`）

## [0.2.0] - 2026-07-20

### 新增
- 用户分享链接管理（列出/撤销）
- 用户头像上传
- 评论系统
- 播放列表
- 全文搜索
- 视频标签
- 管理员转码控制
- 用户存储配额
- 审核用户注册

### 安全
- 分享令牌使用 SHA-256 哈希存储（迁移 023）
- 分享令牌通过 HttpOnly Cookie 传递而非 URL
- 添加 token 撤销功能（迁移 024）
- 关闭用户名枚举时间侧信道
- 添加 Hotlink 防护中间件
- 添加上传带宽限制
- 添加分享端点的速率限制

### 修复
- 中文全文搜索使用正确字典（迁移 021）
- 认证令牌索引优化（迁移 022）
- 分享令牌哈希存储（迁移 026）

## [0.1.0] - 2026-06

### 初始版本
- 用户认证（登录/注册/登出）
- 视频上传和缩略图生成
- 视频播放（HTML5 + 自适应转码）
- 播放历史追踪
- 喜欢/收藏
- 管理后台（用户/视频/标签管理）
- 系统健康检查
- Prometheus 指标
- Sentry 错误追踪
