# Atmos 局域网视频 - 用户行为追踪图

## 日志记录的用户行为

```
用户行为追踪
│
├─── 🔐 认证行为
│    │
│    ├─── 注册行为
│    │    ├─── 触发：POST /auth/register
│    │    ├─── 记录：username, timestamp
│    │    └─── 后续：等待审批/自动登录
│    │
│    ├─── 登录行为
│    │    ├─── 触发：POST /auth/login
│    │    ├─── 成功：记录 "user logged in"
│    │    ├─── 失败：记录 "failed login"
│    │    └─── 限流：触发 "rate limit" 警告
│    │
│    └─── 登出行为
│         └─── 触发：POST /auth/logout
│
├─── 🎬 视频浏览行为
│    │
│    ├─── 列表浏览
│    │    ├─── 触发：GET /videos
│    │    ├─── 记录：user, method, path, duration_ms
│    │    └─── 行为模式：
│    │         ├─── 频繁刷新（间隔<5秒）
│    │         ├─── 分类切换
│    │         └─── 搜索操作
│    │
│    ├─── 详情查看
│    │    ├─── 触发：GET /videos/{id}
│    │    └─── 记录：video_id, user
│    │
│    └─── 收藏列表
│         └─── 触发：GET /videos/favorites
│
├─── ▶️ 视频播放行为
│    │
│    ├─── 开始播放
│    │    ├─── 触发：POST /videos/{id}/play
│    │    ├─── 记录："开始播放视频" + video_id
│    │    └─── 创建播放会话
│    │
│    ├─── 播放中
│    │    ├─── 触发：POST /videos/{id}/heartbeat（每30秒）
│    │    ├─── 记录：心跳日志
│    │    └─── 会话保活
│    │
│    ├─── 停止播放
│    │    ├─── 触发：POST /videos/{id}/stop
│    │    ├─── 记录："停止播放视频" + video_id
│    │    └─── 更新播放历史
│    │
│    └─── 播放历史
│         └─── 触发：GET /playback/history
│
├─── ❤️ 互动行为
│    │
│    ├─── 点赞
│    │    ├─── 触发：POST /videos/{id}/like
│    │    ├─── 记录："toggle like" + liked状态
│    │    └─── 类型：like
│    │
│    └─── 收藏
│         ├─── 触发：POST /videos/{id}/favorite
│         ├─── 记录："toggle favorite" + favorited状态
│         └─── 类型：fav
│
└─── ⚙️ 管理员行为
     │
     ├─── 用户管理
     │    ├─── 查看列表：GET /admin/users
     │    ├─── 审批用户：PUT /admin/users/{id}/approve
     │    ├─── 重置密码：PUT /admin/users/{id}/password
     │    ├─── 切换管理员：PUT /admin/users/{id}/admin
     │    └─── 删除用户：DELETE /admin/users/{id}
     │
     ├─── 视频管理
     │    ├─── 编辑视频：PUT /admin/videos/{id}
     │    ├─── 删除视频：DELETE /admin/videos/{id}
     │    ├─── 批量删除：DELETE /admin/videos/batch
     │    ├─── 上传封面：POST /admin/videos/{id}/cover
     │    ├─── 添加外部：POST /admin/videos/external
     │    └─── 扫描媒体：POST /admin/videos/scan
     │
     ├─── 系统操作
     │    ├─── 查看统计：GET /admin/stats
     │    ├─── 查看系统：GET /admin/system
     │    ├─── 注册开关：PUT /admin/config/registration
     │    └─── 补全缩略图：POST /admin/videos/backfill-thumbnails
     │
     └─── 日志操作
          ├─── 查看日志：GET /admin/logs
          └─── 清空日志：DELETE /admin/logs
```

## 用户行为时间线示例

```
时间        用户        操作                    类型
─────────────────────────────────────────────────────
09:00:01   admin       登录成功                 login
09:00:15   admin       查看数据统计             系统
09:01:00   admin       查看用户列表             系统
09:01:30   admin       审批通过用户             成功
09:02:00   user1       注册新账号               成功
09:05:00   user1       登录成功                 login
09:05:30   user1       浏览视频列表             成功
09:06:00   user1       搜索 "教程"              成功
09:06:30   user1       查看视频详情             成功
09:07:00   user1       开始播放视频             播放
09:07:30   user1       播放心跳                 播放
09:08:00   user1       点赞视频                 like
09:08:30   user1       收藏视频                 fav
09:10:00   user1       停止播放                 播放
09:10:30   user1       查看播放历史             成功
09:15:00   user2       登录失败                 错误
09:15:30   user2       登录失败                 错误
09:16:00   user2       触发限流                 危险
```

## 行为模式分析

### 正常行为模式
```
登录 → 浏览列表 → 搜索/筛选 → 查看详情 → 播放 → 互动
```

### 异常行为检测
```
登录失败 × 3        → 账号暴力破解
频繁刷新（<2秒）    → 爬虫/机器人
路径遍历尝试        → 安全攻击
大量删除操作        → 恶意管理
```

### 用户活跃度指标
```
日活用户（DAU）     → 每日登录用户数
视频播放量         → 播放会话总数
互动率             → (点赞+收藏)/播放量
平均播放时长       → 播放会话平均时长
```

## 日志字段说明

| 字段 | 说明 | 示例 |
|------|------|------|
| timestamp | 操作时间 | 2024-01-15T09:00:01Z |
| level | 日志级别 | INFO/WARN/ERROR |
| user | 操作用户 | admin/user1 |
| method | HTTP方法 | GET/POST/PUT/DELETE |
| path | 请求路径 | /videos/1/like |
| status | 状态码 | 200/400/401/500 |
| duration_ms | 耗时(毫秒) | 15 |
| video_id | 视频ID | 1 |
| request_id | 请求追踪ID | abc-123-def |
