# API 文档

## 认证

### POST /api/auth/register
注册新用户

请求体：
```json
{
  "username": "string",
  "password": "string",
  "email": "string"
}
```

### POST /api/auth/login
用户登录

请求体：
```json
{
  "username": "string",
  "password": "string"
}
```

响应：
```json
{
  "ok": true,
  "token": "string"
}
```

## 视频

### GET /api/videos
获取视频列表

查询参数：
- page: 页码 (默认1)
- page_size: 每页数量 (默认20)
- category: 分类筛选
- search: 搜索关键词

### GET /api/videos/:id
获取视频详情

### POST /api/videos/upload
上传视频（需要认证）

## 播放列表

### GET /api/playlists
获取用户播放列表

### POST /api/playlists
创建播放列表

## 评论

### GET /api/videos/:id/comments
获取视频评论

### POST /api/videos/:id/comments
添加评论

## 错误响应格式

所有错误响应格式：
```json
{
  "error": "错误消息"
}
```

HTTP 状态码：
- 400: 请求参数错误
- 401: 未认证
- 403: 无权限
- 404: 资源不存在
- 409: 资源冲突
- 429: 请求过于频繁
- 500: 服务器内部错误
