# Atmos Video 架构文档

## 系统架构

### 后端分层

```
main.rs
  └─ app.rs (路由定义)
       └─ middleware/ (中间件)
            └─ handlers/ (处理器)
                 └─ services/ (业务逻辑)
                      └─ repositories/ (数据访问)
                           └─ SQLx (数据库)
```

### 关键设计决策

1. **分层纪律**：handlers 不直接访问数据库
2. **统一错误处理**：ServiceError 枚举
3. **安全性优先**：时序侧信道防护、XSS 清洗
4. **异步处理**：spawn_blocking 处理 CPU 密集任务

### 数据库 Schema

主要表：
- users - 用户信息
- videos - 视频元数据
- playback_history - 播放历史
- user_likes/favorites - 点赞收藏
- comments - 评论
- playlists - 播放列表
- share_links - 分享链接

### 安全设计

1. 认证：Bearer Token (256位，7天过期)
2. 授权：RBAC 角色控制
3. 输入验证：白名单校验
4. 输出编码：XSS 防护
5. 限流：DashMap 原子操作

### 性能优化

1. 数据库：连接池、索引优化、查询缓存
2. 应用层：Moka 缓存、异步 I/O
3. 前端：懒加载、虚拟列表、HLS 自适应码率
