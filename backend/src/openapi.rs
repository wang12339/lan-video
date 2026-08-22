use serde_json::json;

/// Build the OpenAPI 3.1 spec for the ATMOS API as a JSON value.
/// Generated manually to avoid invasive per-handler annotations.
pub fn spec() -> serde_json::Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "ATMOS API",
            "description": "ATMOS Video — REST API",
            "version": "0.1.0"
        },
        "servers": [
            { "url": "/", "description": "Same-origin (reverse proxy)" },
            { "url": "http://localhost:8082", "description": "Development server" }
        ],
        "paths": {
            "/health": {
                "get": {
                    "summary": "Comprehensive health check",
                    "operationId": "health",
                    "description": "Returns detailed health status including database connectivity, Redis connectivity (if configured), disk space usage, system information, and version information. Returns 200 if all checks pass, 503 if any critical check fails.",
                    "responses": {
                        "200": {
                            "description": "All health checks passed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        },
                        "503": {
                            "description": "One or more health checks failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/server/info": {
                "get": {
                    "summary": "Server information",
                    "operationId": "serverInfo",
                    "description": "Returns the server version",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Server version and status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ServerInfo" }
                                }
                            }
                        }
                    }
                }
            },
            "/auth/register": {
                "post": {
                    "summary": "Register a new user",
                    "operationId": "authRegister",
                    "description": "Create a new user account. Requires REGISTRATION_ENABLED=true. The first registered user becomes admin ONLY when ALLOW_FIRST_USER_ADMIN=true (default: false — new users start as viewers and need admin approval). Rate-limited per username.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/AuthRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Registration result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        },
                        "429": {
                            "description": "Too many attempts — rate limited",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/auth/login": {
                "post": {
                    "summary": "Login with username and password",
                    "operationId": "authLogin",
                    "description": "Authenticate and receive a bearer token. Rate-limited per username.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/AuthRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Login result with auth token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        },
                        "429": {
                            "description": "Too many attempts — rate limited",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/auth/logout": {
                "post": {
                    "summary": "Logout and invalidate token",
                    "operationId": "authLogout",
                    "description": "Invalidate the current auth token and clear the session cookie",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Logged out successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AuthResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/auth/forgot-password": {
                "post": {
                    "summary": "Request a password reset email",
                    "operationId": "forgotPassword",
                    "description": "发送密码重置邮件。无论邮箱是否注册都返回相同响应，避免邮箱枚举。IP 与邮箱均有速率限制。",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/EmailRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Reset request accepted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
                                }
                            }
                        },
                        "429": { "$ref": "#/components/responses/RateLimited" }
                    }
                }
            },
            "/auth/reset-password": {
                "get": {
                    "summary": "Password reset page (email link)",
                    "operationId": "resetPasswordGet",
                    "description": "处理邮件中的重置链接，携带 token 重定向到前端重置密码页面",
                    "parameters": [
                        { "name": "token", "in": "query", "required": true, "description": "密码重置令牌", "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "302": { "description": "重定向到前端重置密码页面" }
                    }
                },
                "post": {
                    "summary": "Reset password",
                    "operationId": "resetPassword",
                    "description": "使用邮件中的令牌设置新密码（8-128 字符，需包含大小写字母、数字、特殊字符中至少三种），成功后吊销该用户所有令牌",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ResetPasswordRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Password reset",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" }
                    }
                }
            },
            "/auth/verify-email": {
                "get": {
                    "summary": "Email verification page (email link)",
                    "operationId": "verifyEmailGet",
                    "description": "处理邮件中的验证链接，验证成功后直接返回成功/失败 HTML 页面",
                    "parameters": [
                        { "name": "token", "in": "query", "required": true, "description": "邮箱验证令牌", "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "验证结果 HTML 页面" }
                    }
                },
                "post": {
                    "summary": "Verify email with token",
                    "operationId": "verifyEmail",
                    "description": "使用令牌验证邮箱地址",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/VerifyEmailRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Email verified",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" }
                    }
                }
            },
            "/auth/user": {
                "get": {
                    "summary": "Get current user info",
                    "operationId": "authUserInfo",
                    "description": "Returns basic information about the authenticated user",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "User details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/UserInfoResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/auth/user/profile": {
                "get": {
                    "summary": "Get user profile with stats",
                    "operationId": "authUserProfile",
                    "description": "Returns user profile including watch history, total watch time, and videos watched count",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Profile with watch history",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/UserProfileResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/auth/user/email": {
                "put": {
                    "summary": "Update current user's email",
                    "operationId": "updateEmail",
                    "description": "更新当前用户邮箱（自动转小写），成功后重置邮箱验证状态",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/EmailRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Email updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "409": {
                            "description": "该邮箱已被其他账号绑定",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/auth/user/avatar": {
                "post": {
                    "summary": "Upload avatar image",
                    "operationId": "uploadAvatar",
                    "description": "通过 multipart 表单上传头像（JPG/PNG/WebP/GIF，最大 5MB），按 magic bytes 校验文件类型",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string", "format": "binary", "description": "头像图片文件" }
                                    },
                                    "required": ["file"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Avatar uploaded",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "ok": { "type": "boolean" },
                                            "avatarUrl": { "type": "string", "description": "头像地址" }
                                        }
                                    }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/auth/user/shares": {
                "get": {
                    "summary": "List current user's share links",
                    "operationId": "listMyShares",
                    "description": "列出当前用户创建的所有分享链接",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Share link list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/ShareListItem" }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/auth/user/shares/{share_id}": {
                "delete": {
                    "summary": "Revoke own share link",
                    "operationId": "revokeMyShare",
                    "description": "删除当前用户自己的分享链接",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "share_id", "in": "path", "required": true, "description": "分享链接 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Share link revoked",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/auth/send-verification-email": {
                "post": {
                    "summary": "Resend email verification link",
                    "operationId": "sendVerificationEmail",
                    "description": "重新发送邮箱验证邮件。每 5 分钟最多 2 次（用户级速率限制）；SMTP 未配置时直接标记已验证",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Verification email result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/admin/track": {
                "post": {
                    "summary": "Record user action (analytics)",
                    "operationId": "trackAction",
                    "description": "记录用户操作日志（页面、动作、目标），仅返回 204",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TrackRequest" }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Action recorded" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/videos": {
                "get": {
                    "summary": "List videos (paginated)",
                    "operationId": "listVideos",
                    "description": "Retrieve a paginated list of videos with optional filters. Results are cached for 10 seconds.",
                    "parameters": [
                        {
                            "name": "page",
                            "in": "query",
                            "description": "Page number (0-indexed)",
                            "schema": { "type": "integer", "default": 0, "minimum": 0 }
                        },
                        {
                            "name": "size",
                            "in": "query",
                            "description": "Page size (1-1000)",
                            "schema": { "type": "integer", "default": 20, "minimum": 1, "maximum": 1000 }
                        },
                        {
                            "name": "query",
                            "in": "query",
                            "description": "Search query — matches title and category (case-insensitive)",
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "type",
                            "in": "query",
                            "description": "Filter by source_type (prefix with ! to exclude, e.g. '!external')",
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "category",
                            "in": "query",
                            "description": "Filter by category name",
                            "schema": { "type": "string" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Paginated video list",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PagedVideoResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}": {
                "get": {
                    "summary": "Get single video details",
                    "operationId": "getVideo",
                    "description": "Retrieve details for a single video by ID",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Video details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/VideoItem" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/view": {
                "post": {
                    "summary": "Increment view count",
                    "operationId": "incrementViews",
                    "description": "Increment the view counter for a video. Rate-limited to 30 views per IP per 60 seconds per video.",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "View recorded",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
                                }
                            }
                        },
                        "429": { "$ref": "#/components/responses/RateLimited" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/like": {
                "get": {
                    "summary": "Get like status",
                    "operationId": "getLikeStatus",
                    "description": "Check if the current user has liked this video",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Like status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ToggleResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "post": {
                    "summary": "Toggle like",
                    "operationId": "toggleLike",
                    "description": "Toggle like status for the current user on this video",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "New like status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ToggleResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/favorite": {
                "get": {
                    "summary": "Get favorite status",
                    "operationId": "getFavoriteStatus",
                    "description": "Check if the current user has favorited this video",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Favorite status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ToggleResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "post": {
                    "summary": "Toggle favorite",
                    "operationId": "toggleFavorite",
                    "description": "Toggle favorite status for the current user on this video",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "New favorite status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ToggleResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/favorites": {
                "get": {
                    "summary": "List current user's favorites",
                    "operationId": "listFavorites",
                    "description": "返回当前用户收藏的视频列表",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Favorite video list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/RecentWatchItem" }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/variants": {
                "get": {
                    "summary": "List transcoded variants for a video",
                    "operationId": "getVideoVariants",
                    "description": "返回视频可用的转码分片（分辨率、播放地址、大小等）",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Variant list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/VideoVariantResponse" }
                                    }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/playlists": {
                "get": {
                    "summary": "List current user's playlists",
                    "operationId": "listMyPlaylists",
                    "description": "返回当前用户创建的所有播放列表（含封面与条目数）",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Playlist list",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PlaylistListResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                },
                "post": {
                    "summary": "Create a playlist",
                    "operationId": "createPlaylist",
                    "description": "创建播放列表，名称需为 1-200 字符",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreatePlaylistRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Playlist created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PlaylistResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/playlists/{id}": {
                "get": {
                    "summary": "Get a playlist",
                    "operationId": "getPlaylist",
                    "description": "获取播放列表详情（本人、管理员或公开列表）。非公开的他人列表返回 404 避免泄露存在性",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Playlist details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PlaylistResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "put": {
                    "summary": "Update a playlist",
                    "operationId": "updatePlaylist",
                    "description": "修改播放列表名称、描述或公开状态（仅所有者）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/UpdatePlaylistRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Playlist updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "delete": {
                    "summary": "Delete a playlist",
                    "operationId": "deletePlaylist",
                    "description": "删除播放列表及其全部条目（仅所有者）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Playlist deleted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/playlists/{id}/videos": {
                "get": {
                    "summary": "List videos in a playlist",
                    "operationId": "listPlaylistVideos",
                    "description": "按播放列表顺序返回其中的视频",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Playlist videos",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/PlaylistVideoItem" }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                },
                "post": {
                    "summary": "Add a video to a playlist",
                    "operationId": "addVideoToPlaylist",
                    "description": "向播放列表添加视频（仅所有者）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/AddVideoToPlaylistRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Video added",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/playlists/{id}/videos/{video_id}": {
                "delete": {
                    "summary": "Remove a video from a playlist",
                    "operationId": "removeVideoFromPlaylist",
                    "description": "从播放列表中移除指定视频（仅所有者）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "播放列表 ID", "schema": { "type": "integer" } },
                        { "name": "video_id", "in": "path", "required": true, "description": "视频 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Video removed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/videos/{id}/comments": {
                "get": {
                    "summary": "List comments for a video",
                    "operationId": "listComments",
                    "description": "分页返回视频的顶级评论",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        },
                        {
                            "name": "page",
                            "in": "query",
                            "description": "Page number (0-indexed)",
                            "schema": { "type": "integer", "default": 0 }
                        },
                        {
                            "name": "size",
                            "in": "query",
                            "description": "Page size (max 100)",
                            "schema": { "type": "integer", "default": 20, "maximum": 100 }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Comment list",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CommentListResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "post": {
                    "summary": "Create a comment",
                    "operationId": "createComment",
                    "description": "发表评论（可指定 parent_id 回复某条评论）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateCommentRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Comment created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CommentResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/comments/{id}/replies": {
                "get": {
                    "summary": "List replies to a comment",
                    "operationId": "listReplies",
                    "description": "返回某条评论的全部回复",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "评论 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Reply list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/CommentResponse" }
                                    }
                                }
                            }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/comments/{id}": {
                "delete": {
                    "summary": "Delete a comment",
                    "operationId": "deleteComment",
                    "description": "删除评论（作者本人或管理员）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "评论 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Comment deleted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/videos/{id}/share": {
                "post": {
                    "summary": "Create a share link for a video",
                    "operationId": "createShareLink",
                    "description": "为视频创建分享链接，返回一次性展示的分享令牌（token 仅创建时返回）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateShareRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Share link created",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CreateShareResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/share/{share_id}": {
                "delete": {
                    "summary": "Delete a share link",
                    "operationId": "deleteShareLink",
                    "description": "删除指定分享链接（创建者本人或管理员）",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        },
                        { "name": "share_id", "in": "path", "required": true, "description": "分享链接 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Share link deleted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" }
                    }
                }
            },
            "/playback/history": {
                "get": {
                    "summary": "List playback history for current user",
                    "operationId": "listPlaybackHistory",
                    "description": "Get the current user's playback history, ordered by most recent",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Watch history list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/RecentWatchItem" }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "post": {
                    "summary": "Update playback position",
                    "operationId": "updatePlaybackHistory",
                    "description": "Save or update the playback position for a video. Validates that values are non-negative and within reasonable bounds.",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/PlaybackHistoryRequest" }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Playback position updated" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/playback/history/{video_id}": {
                "get": {
                    "summary": "Get playback position for a video",
                    "operationId": "getPlaybackHistoryForVideo",
                    "description": "Get the saved playback position and duration for a specific video",
                    "parameters": [
                        {
                            "name": "video_id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Playback position",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PlaybackHistoryResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/playback/session/start": {
                "post": {
                    "summary": "Start a playback session",
                    "operationId": "startPlaybackSession",
                    "description": "记录一次播放会话的开始，用于在线用户统计",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/PlaybackSessionRequest" }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Session started" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/playback/session/heartbeat": {
                "post": {
                    "summary": "Refresh a playback session",
                    "operationId": "playbackSessionHeartbeat",
                    "description": "刷新播放会话的心跳，避免会话过期",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/PlaybackSessionRequest" }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Heartbeat recorded" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/playback/session/stop": {
                "post": {
                    "summary": "Stop a playback session",
                    "operationId": "stopPlaybackSession",
                    "description": "结束一次播放会话",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/PlaybackSessionRequest" }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Session stopped" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" }
                    }
                }
            },
            "/admin/videos/upload": {
                "post": {
                    "summary": "Upload a video file (multipart)",
                    "operationId": "uploadVideo",
                    "description": "Upload a video or image file via multipart form. The file is validated against its magic bytes. Duplicate files (by MD5 hash) are rejected. Thumbnails are generated automatically for video files.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string", "format": "binary", "description": "Video or image file" },
                                        "category": { "type": "string", "default": "local", "description": "Category for the uploaded file" },
                                        "fileHash": { "type": "string", "description": "Pre-computed MD5 hash for duplicate detection" }
                                    },
                                    "required": ["file"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Upload successful — returns the new video ID",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/IdResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "409": {
                            "description": "Duplicate file — video already exists",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/upload-resume": {
                "post": {
                    "summary": "Resume or append to an upload",
                    "operationId": "uploadResume",
                    "description": "Append data to a partial upload identified by hash. When the total received bytes equals x-upload-size, the upload is finalized. Returns 206 Partial Content if more data is needed.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "x-upload-hash", "in": "header", "required": true, "description": "Unique upload identifier (alphanumeric, dash, underscore, max 128 chars)", "schema": { "type": "string" } },
                        { "name": "x-upload-name", "in": "header", "description": "Original filename", "schema": { "type": "string", "default": "video.mp4" } },
                        { "name": "x-upload-size", "in": "header", "required": true, "description": "Total expected file size in bytes", "schema": { "type": "integer" } },
                        { "name": "x-upload-category", "in": "header", "description": "Category for the uploaded file", "schema": { "type": "string", "default": "local" } }
                    ],
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/octet-stream": {
                                "schema": { "type": "string", "format": "binary" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Upload status (empty body — just check received bytes)",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": { "received": { "type": "integer", "description": "Bytes received so far" } }
                                    }
                                }
                            }
                        },
                        "201": {
                            "description": "Upload finalized",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "integer" },
                                            "received": { "type": "integer" }
                                        }
                                    }
                                }
                            }
                        },
                        "206": {
                            "description": "Partial content — more data needed",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": { "received": { "type": "integer" } }
                                    }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/upload-status": {
                "get": {
                    "summary": "Check upload progress",
                    "operationId": "uploadStatus",
                    "description": "Check how many bytes have been received for a partial upload",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        {
                            "name": "hash",
                            "in": "query",
                            "required": true,
                            "description": "Upload hash identifier",
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Upload progress",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": { "received": { "type": "integer", "description": "Bytes received so far" } }
                                    }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/batch": {
                "delete": {
                    "summary": "Batch delete videos",
                    "operationId": "deleteVideos",
                    "description": "Delete multiple videos and their associated files, playback history, likes, and favorites in a single transaction. Max 500 IDs per request.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "type": "integer" },
                                    "maxItems": 500,
                                    "description": "Array of video IDs to delete"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Delete result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/{id}": {
                "put": {
                    "summary": "Update video metadata",
                    "operationId": "updateVideo",
                    "description": "Update title, description, and/or category for a video",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/VideoUpdateRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Update result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                },
                "delete": {
                    "summary": "Delete a video",
                    "operationId": "deleteVideo",
                    "description": "Delete a video and its associated physical files, playback history, likes, and favorites",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Delete result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/external": {
                "post": {
                    "summary": "Add external video link",
                    "operationId": "addExternalVideo",
                    "description": "Register an external video URL (http/https). Title must be 1-500 characters.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ExternalVideoRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Created — returns the new video ID",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/IdResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/check-hashes": {
                "post": {
                    "summary": "Check which file hashes already exist",
                    "operationId": "checkHashes",
                    "description": "Given a list of MD5 hashes, return which ones already exist in the database. Max 1000 hashes per request.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CheckHashesRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "List of existing hashes",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CheckHashesResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/check-files": {
                "post": {
                    "summary": "Check which files already exist by name and size",
                    "operationId": "checkFiles",
                    "description": "Given a list of (name, size) pairs, return which indices already exist in the database. Max 1000 files per request.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "$ref": "#/components/schemas/FileCheckItem" },
                                    "maxItems": 1000
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Indices of existing files",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/CheckFilesResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/scan": {
                "post": {
                    "summary": "Scan media directory for new files",
                    "operationId": "scanMedia",
                    "description": "Scan the configured media directory for video and image files not yet in the database. New files are added with MD5 hash and metadata. Max 5000 files per scan.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": false,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "category": { "type": "string", "default": "local", "description": "Category for discovered files" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Scan result",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "added": { "type": "integer", "description": "Number of new files added" }
                                        }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/backfill-thumbnails": {
                "post": {
                    "summary": "Generate missing thumbnails",
                    "operationId": "backfillThumbnails",
                    "description": "Scan all local videos without covers and generate thumbnails using ffmpeg. Runs in batches of 100 to avoid memory spikes.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Backfill result",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "ok": { "type": "boolean" },
                                            "generated": { "type": "integer", "description": "Number of thumbnails generated" },
                                            "errors": { "type": "array", "items": { "type": "string" }, "description": "Error messages for failed generations" }
                                        }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/{id}/cover": {
                "post": {
                    "summary": "Upload cover image for a video",
                    "operationId": "uploadCover",
                    "description": "Upload a cover image for a specific video via multipart form",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Video ID",
                            "schema": { "type": "integer" }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string", "format": "binary", "description": "Cover image file" }
                                    },
                                    "required": ["file"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "204": { "description": "Cover uploaded successfully" },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/users": {
                "get": {
                    "summary": "List users",
                    "operationId": "listUsers",
                    "description": "返回当前租户的用户列表（含审批状态、角色、活跃令牌等）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "User list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/UserWithStatus" }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/users/{id}": {
                "delete": {
                    "summary": "Delete a user",
                    "operationId": "deleteUser",
                    "description": "删除用户及其关联数据（不能删除自己）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "用户 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Delete result",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/users/{id}/password": {
                "put": {
                    "summary": "Reset a user's password",
                    "operationId": "resetUserPassword",
                    "description": "管理员重置指定用户密码，同时吊销该用户全部令牌",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "用户 ID", "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/AdminPasswordRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Password reset",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/users/{id}/admin": {
                "put": {
                    "summary": "Toggle a user's admin status",
                    "operationId": "toggleUserAdmin",
                    "description": "切换用户的管理员权限（不能操作自己）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "用户 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Admin status toggled",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/users/{id}/approve": {
                "put": {
                    "summary": "Approve or reject a user",
                    "operationId": "approveUser",
                    "description": "审批新用户（注册审批制下用户需审批后才能登录）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "用户 ID", "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "approved": { "type": "boolean", "description": "是否批准" }
                                    },
                                    "required": ["approved"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Approval updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/users/{id}/kick": {
                "post": {
                    "summary": "Kick a user offline",
                    "operationId": "kickUser",
                    "description": "强制用户下线：删除该用户全部认证令牌",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "description": "用户 ID", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "User kicked",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/videos/batch-category": {
                "put": {
                    "summary": "Batch update video categories",
                    "operationId": "batchUpdateCategory",
                    "description": "批量修改视频分类（最多 1000 个，分类名最多 100 字符）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/BatchCategoryRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Categories updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/stats": {
                "get": {
                    "summary": "Get admin dashboard stats",
                    "operationId": "getStats",
                    "description": "返回视频/图片/用户数量、总播放量与观看时长、类型与分类分布等统计数据",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Dashboard stats",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/AdminStatsResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/config/registration": {
                "get": {
                    "summary": "Get registration toggle state",
                    "operationId": "getRegistrationEnabled",
                    "description": "查询注册开关的当前状态",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Registration state",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "enabled": { "type": "boolean" }
                                        }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                },
                "put": {
                    "summary": "Set registration toggle",
                    "operationId": "setRegistrationEnabled",
                    "description": "开启/关闭公开注册（持久化到数据库）",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "enabled": { "type": "boolean" }
                                    },
                                    "required": ["enabled"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Registration toggle updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/system": {
                "get": {
                    "summary": "Get system monitoring info",
                    "operationId": "systemInfo",
                    "description": "返回媒体目录磁盘占用与数据库连接数等系统监控信息",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "System info",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "mediaSizeBytes": { "type": "integer", "description": "媒体目录磁盘占用（字节）" },
                                            "mediaSizeHuman": { "type": "string", "description": "媒体目录磁盘占用（人类可读）" },
                                            "dbConnections": { "type": "integer", "description": "数据库活跃连接数" }
                                        }
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" }
                    }
                }
            },
            "/admin/logs": {
                "get": {
                    "summary": "Read server logs",
                    "operationId": "getLogs",
                    "description": "从最新日志文件尾部读取日志条目，支持 level/search 过滤与分页",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "level", "in": "query", "description": "按日志级别过滤（INFO/WARN/ERROR…）", "schema": { "type": "string" } },
                        { "name": "search", "in": "query", "description": "按消息/路径/用户等关键字过滤", "schema": { "type": "string" } },
                        { "name": "limit", "in": "query", "description": "返回条数（默认 200，最大 1000）", "schema": { "type": "integer", "default": 200, "maximum": 1000 } },
                        { "name": "offset", "in": "query", "description": "跳过条数（从最新往旧）", "schema": { "type": "integer", "default": 0 } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Log entries",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/LogListResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "delete": {
                    "summary": "Clear server logs",
                    "operationId": "clearLogs",
                    "description": "清空当前日志文件内容",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Logs cleared",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/{id}/transcode": {
                "post": {
                    "summary": "Start video transcoding",
                    "operationId": "startTranscode",
                    "description": "Queue a video for background transcoding into multiple quality variants (2160p/1080p/720p/480p/360p)",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TranscodeRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Transcoding started",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/{id}/transcode/status": {
                "get": {
                    "summary": "Get transcode status",
                    "operationId": "getTranscodeStatus",
                    "description": "获取视频的转码状态与可用转码分片列表",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Transcode status",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TranscodeStatusResponse" } } }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/{id}/transcode/{resolution}": {
                "delete": {
                    "summary": "Delete a transcode variant",
                    "operationId": "deleteVariant",
                    "description": "Delete a specific transcoded variant and its physical file",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } },
                        { "name": "resolution", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "description": "Variant deleted", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } } },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/videos/{id}/transcode/cancel": {
                "post": {
                    "summary": "Cancel transcoding jobs",
                    "operationId": "cancelTranscode",
                    "description": "Cancel all pending/running transcoding jobs for a video",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "Jobs cancelled", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } } },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/tags": {
                "get": {
                    "summary": "List all tags",
                    "operationId": "listTags",
                    "description": "Get all available tags with usage count",
                    "responses": {
                        "200": {
                            "description": "Tag list",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagListResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/tags/popular": {
                "get": {
                    "summary": "Get popular tags",
                    "operationId": "getPopularTags",
                    "description": "Get most frequently used tags (limit 20)",
                    "responses": {
                        "200": {
                            "description": "Popular tags",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagListResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/tags/{id}": {
                "get": {
                    "summary": "Get tag by ID",
                    "operationId": "getTag",
                    "description": "Get a single tag with its usage count",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Tag details",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagResponse" } } }
                        },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tags": {
                "post": {
                    "summary": "Create a tag",
                    "operationId": "createTag",
                    "description": "Create a new tag. Name must be unique and 1-50 characters.",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagCreateRequest" } } }
                    },
                    "responses": {
                        "201": {
                            "description": "Tag created",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagResponse" } } }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tags/{id}": {
                "put": {
                    "summary": "Update a tag",
                    "operationId": "updateTag",
                    "description": "Update tag name and/or color",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagUpdateRequest" } } }
                    },
                    "responses": {
                        "200": { "description": "Tag updated", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagResponse" } } } },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "delete": {
                    "summary": "Delete a tag",
                    "operationId": "deleteTag",
                    "description": "Delete a tag and all its video associations",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "Tag deleted", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } } },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tenants": {
                "get": {
                    "summary": "List all tenants",
                    "operationId": "listTenants",
                    "description": "获取所有租户列表",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Tenant list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "tenants": {
                                                "type": "array",
                                                "items": { "$ref": "#/components/schemas/TenantConfig" }
                                            }
                                        },
                                        "required": ["tenants"]
                                    }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tenants/{id}": {
                "get": {
                    "summary": "Get tenant details",
                    "operationId": "getTenant",
                    "description": "获取单个租户详情",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Tenant details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TenantConfig" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "put": {
                    "summary": "Update tenant settings",
                    "operationId": "updateTenant",
                    "description": "更新租户配置",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TenantSettings" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Tenant settings updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tenants/{id}/stats": {
                "get": {
                    "summary": "Get tenant usage statistics",
                    "operationId": "getTenantStats",
                    "description": "获取租户使用统计",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Tenant statistics",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TenantStats" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/admin/tenants/{id}/toggle": {
                "post": {
                    "summary": "Enable or disable tenant",
                    "operationId": "toggleTenant",
                    "description": "禁用/启用租户",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "status": { "type": "string", "enum": ["active", "disabled", "maintenance"], "description": "Tenant status" }
                                    },
                                    "required": ["status"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Tenant status updated",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/OkResponse" }
                                }
                            }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/tags": {
                "get": {
                    "summary": "Get tags for a video",
                    "operationId": "getVideoTags",
                    "description": "Get all tags assigned to a video",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Video tags",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/TagListResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "post": {
                    "summary": "Add tags to a video",
                    "operationId": "addVideoTags",
                    "description": "Add one or more tags to a video by tag IDs",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/VideoTagRequest" } } }
                    },
                    "responses": {
                        "200": { "description": "Tags added", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } } },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                },
                "delete": {
                    "summary": "Remove tags from a video",
                    "operationId": "removeVideoTags",
                    "description": "按标签 ID 数组从视频上移除标签",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "type": "integer" },
                                    "description": "要移除的标签 ID 数组"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Tags removed",
                            "content": { "application/json": { "schema": { "type": "object", "properties": { "success": { "type": "boolean" }, "message": { "type": "string" } } } } }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/{id}/tags/{tag_id}": {
                "delete": {
                    "summary": "Remove a tag from a video",
                    "operationId": "removeVideoTag",
                    "description": "Remove a specific tag from a video",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } },
                        { "name": "tag_id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "Tag removed", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/OkResponse" } } } },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "403": { "$ref": "#/components/responses/Forbidden" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/search": {
                "get": {
                    "summary": "Full-text search videos",
                    "operationId": "searchVideos",
                    "description": "Search videos using PostgreSQL full-text search with ranking. Supports Chinese tokenization.",
                    "parameters": [
                        { "name": "q", "in": "query", "required": true, "schema": { "type": "string" }, "description": "Search query" },
                        { "name": "page", "in": "query", "schema": { "type": "integer", "default": 0, "minimum": 0 }, "description": "Page number (0-indexed)" },
                        { "name": "size", "in": "query", "schema": { "type": "integer", "default": 20, "maximum": 100 }, "description": "Results per page" }
                    ],
                    "responses": {
                        "200": {
                            "description": "Search results",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/SearchResponse" } } }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/videos/search/suggest": {
                "get": {
                    "summary": "Search suggestions",
                    "operationId": "searchSuggest",
                    "description": "Get search suggestions based on partial query",
                    "parameters": [
                        { "name": "q", "in": "query", "required": true, "schema": { "type": "string" }, "description": "Partial search query" },
                        { "name": "page", "in": "query", "schema": { "type": "integer", "default": 0 }, "description": "Page number" },
                        { "name": "size", "in": "query", "schema": { "type": "integer", "default": 10, "maximum": 20 }, "description": "Max suggestions" }
                    ],
                    "responses": {
                        "200": {
                            "description": "Search suggestions",
                            "content": { "application/json": { "schema": { "type": "array", "items": { "type": "string" } } } }
                        },
                        "400": { "$ref": "#/components/responses/BadRequest" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/recommendations": {
                "get": {
                    "summary": "Get personalized recommendations",
                    "operationId": "getRecommendations",
                    "description": "Get video recommendations based on the user's viewing history and category preferences",
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Recommendations",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RecommendationResponse" } } }
                        },
                        "401": { "$ref": "#/components/responses/Unauthorized" },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/recommendations/trending": {
                "get": {
                    "summary": "Get trending videos",
                    "operationId": "getTrendingVideos",
                    "description": "Get popular videos ranked by views and engagement",
                    "responses": {
                        "200": {
                            "description": "Trending videos",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RecommendationResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/recommendations/recent": {
                "get": {
                    "summary": "Get recent videos",
                    "operationId": "getRecentVideos",
                    "description": "Get most recently uploaded videos",
                    "responses": {
                        "200": {
                            "description": "Recent videos",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RecommendationResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/recommendations/similar/{video_id}": {
                "get": {
                    "summary": "Get similar videos",
                    "operationId": "getSimilarVideos",
                    "description": "Get videos similar to a specific video based on category matching",
                    "parameters": [
                        { "name": "video_id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Similar videos",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RecommendationResponse" } } }
                        },
                        "500": { "$ref": "#/components/responses/InternalError" }
                    }
                }
            },
            "/metrics": {
                "get": {
                    "summary": "Server metrics (JSON)",
                    "operationId": "metrics",
                    "description": "Returns server metrics in JSON format",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Server metrics",
                            "content": { "application/json": { "schema": { "type": "object" } } }
                        }
                    }
                }
            },
            "/metrics/prometheus": {
                "get": {
                    "summary": "Prometheus metrics",
                    "operationId": "metricsPrometheus",
                    "description": "Returns server metrics in Prometheus text exposition format",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Prometheus metrics text",
                            "content": { "text/plain": { "schema": { "type": "string" } } }
                        }
                    }
                }
            },
            "/share/{token}": {
                "get": {
                    "summary": "Resolve a shared video",
                    "operationId": "getShareVideo",
                    "description": "Resolve a share token to the shared video. Public — no auth required, but rate-limited per IP to prevent token enumeration. On success a share_token HttpOnly cookie is set so media requests authenticate.",
                    "parameters": [
                        { "name": "token", "in": "path", "required": true, "description": "Un-guessable share token", "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Shared video details",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/VideoItem" } } }
                        },
                        "404": { "$ref": "#/components/responses/NotFound" },
                        "429": { "$ref": "#/components/responses/RateLimited" }
                    }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Opaque 256-bit alphanumeric token returned by /auth/login or /auth/register"
                },
                "adminAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Requires admin privileges (role >= 3). First-user auto-admin only with ALLOW_FIRST_USER_ADMIN=true"
                }
            },
            "responses": {
                "BadRequest": {
                    "description": "Bad request — invalid parameters or request body",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "Unauthorized": {
                    "description": "Authentication required — missing or invalid token",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "Forbidden": {
                    "description": "Admin privileges required",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "NotFound": {
                    "description": "Resource not found",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "RateLimited": {
                    "description": "Too many requests — rate limit exceeded",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                },
                "InternalError": {
                    "description": "Internal server error",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }
            },
            "schemas": {
                "ErrorResponse": {
                    "type": "object",
                    "properties": {
                        "error": { "type": "string", "description": "Human-readable error message" }
                    },
                    "required": ["error"]
                },
                "HealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string", "enum": ["healthy", "unhealthy"], "description": "Overall health status" },
                        "version": { "type": "string", "description": "Server version from Cargo.toml" },
                        "timestamp": { "type": "string", "format": "date-time", "description": "Response timestamp in RFC3339 format" },
                        "checks": {
                            "type": "object",
                            "description": "Individual health check results",
                            "additionalProperties": {
                                "$ref": "#/components/schemas/CheckStatus"
                            }
                        },
                        "system_info": { "$ref": "#/components/schemas/SystemInfo" }
                    },
                    "required": ["status", "version", "timestamp", "checks", "system_info"]
                },
                "CheckStatus": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string", "enum": ["healthy", "unhealthy", "warning"], "description": "Check result status" },
                        "message": { "type": "string", "nullable": true, "description": "Optional error message" },
                        "response_time_ms": { "type": "integer", "nullable": true, "description": "Response time in milliseconds" }
                    },
                    "required": ["status"]
                },
                "SystemInfo": {
                    "type": "object",
                    "properties": {
                        "uptime_secs": { "type": "integer", "description": "Server uptime in seconds" },
                        "disk_usage": { "$ref": "#/components/schemas/DiskUsage" },
                        "memory_usage": { "$ref": "#/components/schemas/MemoryUsage", "nullable": true }
                    },
                    "required": ["uptime_secs", "disk_usage"]
                },
                "DiskUsage": {
                    "type": "object",
                    "properties": {
                        "total_bytes": { "type": "integer", "description": "Total disk space in bytes" },
                        "used_bytes": { "type": "integer", "description": "Used disk space in bytes" },
                        "available_bytes": { "type": "integer", "description": "Available disk space in bytes" },
                        "usage_percent": { "type": "number", "format": "double", "description": "Disk usage percentage (0-100)" }
                    },
                    "required": ["total_bytes", "used_bytes", "available_bytes", "usage_percent"]
                },
                "MemoryUsage": {
                    "type": "object",
                    "properties": {
                        "total_bytes": { "type": "integer", "description": "Total memory in bytes" },
                        "used_bytes": { "type": "integer", "description": "Used memory in bytes" },
                        "available_bytes": { "type": "integer", "description": "Available memory in bytes" },
                        "usage_percent": { "type": "number", "format": "double", "description": "Memory usage percentage (0-100)" }
                    },
                    "required": ["total_bytes", "used_bytes", "available_bytes", "usage_percent"]
                },
                "ServerInfo": {
                    "type": "object",
                    "properties": {
                        "version": { "type": "string", "description": "Server version from Cargo.toml" }
                    },
                    "required": ["version"]
                },
                "AuthRequest": {
                    "type": "object",
                    "properties": {
                        "username": { "type": "string", "minLength": 2, "maxLength": 64, "description": "Username (2-64 characters)" },
                        "password": { "type": "string", "minLength": 6, "maxLength": 128, "description": "Password (6-128 characters)" }
                    },
                    "required": ["username", "password"]
                },
                "AuthResponse": {
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean", "description": "Whether the operation succeeded" },
                        "token": { "type": "string", "nullable": true, "description": "Auth token (present on success)" },
                        "error": { "type": "string", "nullable": true, "description": "Error message (present on failure)" }
                    },
                    "required": ["ok"]
                },
                "UserInfoResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "username": { "type": "string" },
                        "isAdmin": { "type": "boolean" },
                        "createdAt": { "type": "string", "description": "Account creation timestamp (YYYY-MM-DD HH:MM:SS)" },
                        "email": { "type": "string", "nullable": true, "description": "用户邮箱" },
                        "emailVerified": { "type": "boolean", "description": "邮箱是否已验证" }
                    },
                    "required": ["id", "username", "isAdmin", "createdAt", "emailVerified"]
                },
                "UserProfileResponse": {
                    "type": "object",
                    "properties": {
                        "username": { "type": "string" },
                        "isAdmin": { "type": "boolean" },
                        "createdAt": { "type": "string" },
                        "totalVideosWatched": { "type": "integer" },
                        "totalWatchTimeMs": { "type": "integer", "description": "Total watch time in milliseconds" },
                        "recentHistory": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/RecentWatchItem" }
                        }
                    },
                    "required": ["username", "isAdmin", "createdAt", "totalVideosWatched", "totalWatchTimeMs", "recentHistory"]
                },
                "VideoItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "sourceType": { "type": "string", "description": "e.g. 'local_video', 'local_image', 'external'" },
                        "coverUrl": { "type": "string", "nullable": true, "description": "Cover image URL" },
                        "streamUrl": { "type": "string", "description": "Video/image stream URL" },
                        "thumbUrl": { "type": "string", "nullable": true, "description": "Thumbnail URL" },
                        "category": { "type": "string" },
                        "views": { "type": "integer" },
                        "duration": { "type": "integer", "description": "Duration in seconds" },
                        "watchPosition": { "type": "integer", "nullable": true, "description": "Current user's watch position in ms" },
                        "createdAt": { "type": "string", "description": "Creation time (UTC, format %Y-%m-%d %H:%M:%S)" }
                    },
                    "required": ["id", "title", "streamUrl"]
                },
                "PagedVideoResponse": {
                    "type": "object",
                    "properties": {
                        "items": { "type": "array", "items": { "$ref": "#/components/schemas/VideoItem" } },
                        "total": { "type": "integer", "description": "Total number of matching videos" },
                        "page": { "type": "integer", "description": "Current page number (0-indexed)" },
                        "size": { "type": "integer", "description": "Page size" }
                    },
                    "required": ["items", "total", "page", "size"]
                },
                "PlaybackHistoryRequest": {
                    "type": "object",
                    "properties": {
                        "video_id": { "type": "integer", "description": "Video ID" },
                        "position_ms": { "type": "integer", "minimum": 0, "description": "Current playback position in milliseconds" },
                        "duration_ms": { "type": "integer", "minimum": 0, "description": "Total video duration in milliseconds (max 7 days)" }
                    },
                    "required": ["video_id", "position_ms", "duration_ms"]
                },
                "PlaybackHistoryResponse": {
                    "type": "object",
                    "properties": {
                        "videoId": { "type": "integer" },
                        "positionMs": { "type": "integer", "description": "Saved playback position in ms" },
                        "durationMs": { "type": "integer", "description": "Saved video duration in ms" }
                    },
                    "required": ["videoId", "positionMs", "durationMs"]
                },
                "RecentWatchItem": {
                    "type": "object",
                    "properties": {
                        "videoId": { "type": "integer" },
                        "title": { "type": "string" },
                        "coverUrl": { "type": "string", "nullable": true },
                        "streamUrl": { "type": "string" },
                        "sourceType": { "type": "string" },
                        "category": { "type": "string" },
                        "positionMs": { "type": "integer" },
                        "durationMs": { "type": "integer" },
                        "updatedAt": { "type": "string", "description": "Last watch timestamp (YYYY-MM-DD HH:MM:SS)" }
                    },
                    "required": ["videoId", "title", "streamUrl", "sourceType", "category", "positionMs", "durationMs", "updatedAt"]
                },
                "ExternalVideoRequest": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "minLength": 1, "maxLength": 500, "description": "Video title" },
                        "description": { "type": "string", "nullable": true, "description": "Video description" },
                        "category": { "type": "string", "nullable": true, "default": "general", "description": "Video category" },
                        "stream_url": { "type": "string", "description": "External video URL (must start with http:// or https://)" },
                        "cover_url": { "type": "string", "nullable": true, "description": "Cover image URL (must start with http:// or https://)" }
                    },
                    "required": ["title", "stream_url"]
                },
                "VideoUpdateRequest": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "nullable": true, "description": "New title" },
                        "description": { "type": "string", "nullable": true, "description": "New description" },
                        "category": { "type": "string", "nullable": true, "description": "New category" }
                    }
                },
                "IdResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer", "description": "Created resource ID" }
                    },
                    "required": ["id"]
                },
                "OkResponse": {
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "error": { "type": "string", "nullable": true },
                        "deleted": { "type": "integer", "nullable": true, "description": "Number of deleted items (batch delete only)" }
                    },
                    "required": ["ok"]
                },
                "ToggleResponse": {
                    "type": "object",
                    "properties": {
                        "liked": { "type": "boolean", "description": "New like/favorite status" }
                    }
                },
                "TenantConfig": {
                    "type": "object",
                    "properties": {
                        "tenant_id": { "type": "integer", "description": "租户 ID" },
                        "slug": { "type": "string", "description": "租户标识符" },
                        "name": { "type": "string", "description": "租户名称" },
                        "host": { "type": "string", "description": "租户域名" },
                        "settings": { "$ref": "#/components/schemas/TenantSettings" }
                    },
                    "required": ["tenant_id", "slug", "name", "host", "settings"]
                },
                "TenantSettings": {
                    "type": "object",
                    "properties": {
                        "max_upload_size_mb": { "type": "integer", "description": "上传文件大小限制（MB）" },
                        "max_videos_per_user": { "type": "integer", "description": "用户视频数量上限" },
                        "registration_enabled": { "type": "boolean", "description": "是否允许注册" },
                        "custom_theme": { "type": "string", "nullable": true, "description": "自定义主题标识" },
                        "storage_quota_gb": { "type": "integer", "description": "存储配额（GB）" }
                    },
                    "required": ["max_upload_size_mb", "max_videos_per_user", "registration_enabled", "storage_quota_gb"]
                },
                "TenantStats": {
                    "type": "object",
                    "properties": {
                        "tenant_id": { "type": "integer", "description": "租户 ID" },
                        "slug": { "type": "string", "description": "租户标识符" },
                        "name": { "type": "string", "description": "租户名称" },
                        "user_count": { "type": "integer", "description": "用户总数" },
                        "video_count": { "type": "integer", "description": "视频总数" },
                        "storage_used_bytes": { "type": "integer", "description": "已用存储（字节）" },
                        "storage_limit_bytes": { "type": "integer", "description": "存储上限（字节）" }
                    },
                    "required": ["tenant_id", "slug", "name", "user_count", "video_count", "storage_used_bytes", "storage_limit_bytes"]
                },
                "CheckHashesRequest": {
                    "type": "object",
                    "properties": {
                        "hashes": {
                            "type": "array",
                            "items": { "type": "string" },
                            "maxItems": 1000,
                            "description": "List of MD5 hashes to check"
                        }
                    },
                    "required": ["hashes"]
                },
                "CheckHashesResponse": {
                    "type": "object",
                    "properties": {
                        "existing": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Hashes that already exist in the database"
                        }
                    },
                    "required": ["existing"]
                },
                "FileCheckItem": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Original filename" },
                        "size": { "type": "integer", "description": "File size in bytes" }
                    },
                    "required": ["name", "size"]
                },
                "CheckFilesResponse": {
                    "type": "object",
                    "properties": {
                        "existing_indices": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "Indices of files that already exist (by name + size match)"
                        }
                    },
                    "required": ["existing_indices"]
                },
                "TranscodeRequest": {
                    "type": "object",
                    "properties": {
                        "resolutions": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Target resolutions (e.g. [\"1080p\", \"720p\", \"480p\"])"
                        }
                    },
                    "required": ["resolutions"]
                },
                "TranscodeStatusResponse": {
                    "type": "object",
                    "properties": {
                        "videoId": { "type": "integer" },
                        "variants": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/VariantInfo" }
                        },
                        "pendingJobs": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/JobInfo" }
                        }
                    },
                    "required": ["videoId", "variants", "pendingJobs"]
                },
                "VariantInfo": {
                    "type": "object",
                    "properties": {
                        "resolution": { "type": "string", "description": "Resolution label (e.g. 1080p)" },
                        "filePath": { "type": "string", "description": "File path" },
                        "fileSize": { "type": "integer", "description": "File size in bytes" },
                        "bitrate": { "type": "integer", "nullable": true, "description": "Bitrate in bps" }
                    },
                    "required": ["resolution", "filePath", "fileSize"]
                },
                "JobInfo": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "resolution": { "type": "string", "description": "Target resolution" },
                        "status": { "type": "string", "enum": ["pending", "running", "completed", "failed"], "description": "Job status" },
                        "progress": { "type": "integer", "description": "Progress percentage 0-100" }
                    },
                    "required": ["id", "resolution", "status", "progress"]
                },
                "TagResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "color": { "type": "string", "nullable": true, "description": "Hex color code (e.g. #3b82f6)" },
                        "usageCount": { "type": "integer" }
                    },
                    "required": ["id", "name", "usageCount"]
                },
                "TagListResponse": {
                    "type": "object",
                    "properties": {
                        "tags": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/TagResponse" }
                        }
                    },
                    "required": ["tags"]
                },
                "TagCreateRequest": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "maxLength": 50, "description": "Tag name (unique, 1-50 characters)" },
                        "color": { "type": "string", "nullable": true, "description": "Optional hex color code" }
                    },
                    "required": ["name"]
                },
                "TagUpdateRequest": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "nullable": true, "description": "New tag name" },
                        "color": { "type": "string", "nullable": true, "description": "New hex color code" }
                    }
                },
                "VideoTagRequest": {
                    "type": "object",
                    "properties": {
                        "tagIds": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "Array of tag IDs to assign"
                        }
                    },
                    "required": ["tagIds"]
                },
                "SearchResponse": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/SearchResultItem" }
                        },
                        "total": { "type": "integer" },
                        "page": { "type": "integer" },
                        "size": { "type": "integer" }
                    },
                    "required": ["items", "total", "page", "size"]
                },
                "SearchResultItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "title": { "type": "string" },
                        "description": { "type": "string", "nullable": true },
                        "category": { "type": "string", "nullable": true },
                        "rank": { "type": "number", "description": "Search relevance rank score" },
                        "headline": { "type": "string", "nullable": true, "description": "Highlighted search result snippet" }
                    },
                    "required": ["id", "title", "rank"]
                },
                "RecommendationResponse": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/RecommendationItem" }
                        },
                        "total": { "type": "integer" }
                    },
                    "required": ["items", "total"]
                },
                "RecommendationItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "title": { "type": "string" },
                        "category": { "type": "string", "nullable": true },
                        "thumbUrl": { "type": "string", "nullable": true },
                        "score": { "type": "number", "description": "Recommendation relevance score" },
                        "reason": { "type": "string", "description": "Human-readable reason for recommendation" }
                    },
                    "required": ["id", "title", "score", "reason"]
                },
                "EmailRequest": {
                    "type": "object",
                    "properties": {
                        "email": { "type": "string", "description": "邮箱地址" }
                    },
                    "required": ["email"]
                },
                "MessageResponse": {
                    "type": "object",
                    "properties": {
                        "ok": { "type": "boolean" },
                        "message": { "type": "string" }
                    },
                    "required": ["ok", "message"]
                },
                "ResetPasswordRequest": {
                    "type": "object",
                    "properties": {
                        "token": { "type": "string", "description": "重置令牌（邮件中的链接参数）" },
                        "password": { "type": "string", "minLength": 8, "maxLength": 128, "description": "新密码" }
                    },
                    "required": ["token", "password"]
                },
                "VerifyEmailRequest": {
                    "type": "object",
                    "properties": {
                        "token": { "type": "string", "description": "邮箱验证令牌" }
                    },
                    "required": ["token"]
                },
                "TrackRequest": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "description": "用户操作名称" },
                        "target": { "type": "string", "nullable": true, "description": "操作目标" },
                        "page": { "type": "string", "nullable": true, "description": "来源页面" }
                    },
                    "required": ["action"]
                },
                "ShareListItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "expiresAt": { "type": "string", "nullable": true, "description": "过期时间 (YYYY-MM-DD HH:MM:SS)，null 表示永不过期" },
                        "createdAt": { "type": "string", "description": "创建时间 (YYYY-MM-DD HH:MM:SS)" },
                        "active": { "type": "boolean", "description": "是否仍有效" }
                    },
                    "required": ["id", "createdAt", "active"]
                },
                "PlaybackSessionRequest": {
                    "type": "object",
                    "properties": {
                        "video_id": { "type": "integer", "description": "视频 ID" }
                    },
                    "required": ["video_id"]
                },
                "VideoVariantResponse": {
                    "type": "object",
                    "properties": {
                        "resolution": { "type": "string", "description": "分辨率标签（如 1080p）" },
                        "url": { "type": "string", "description": "分片播放地址" },
                        "fileSize": { "type": "integer", "description": "文件大小（字节）" },
                        "bitrate": { "type": "integer", "nullable": true, "description": "码率（bps）" },
                        "codec": { "type": "string", "nullable": true, "description": "视频编码" }
                    },
                    "required": ["resolution", "url", "fileSize"]
                },
                "PlaylistResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "description": { "type": "string", "nullable": true },
                        "isPublic": { "type": "boolean" },
                        "coverUrl": { "type": "string", "nullable": true },
                        "itemCount": { "type": "integer", "description": "条目数量" },
                        "createdAt": { "type": "string", "description": "创建时间 (YYYY-MM-DD HH:MM:SS)" },
                        "updatedAt": { "type": "string", "description": "更新时间 (YYYY-MM-DD HH:MM:SS)" }
                    },
                    "required": ["id", "name", "isPublic", "itemCount", "createdAt", "updatedAt"]
                },
                "PlaylistListResponse": {
                    "type": "object",
                    "properties": {
                        "playlists": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/PlaylistResponse" }
                        }
                    },
                    "required": ["playlists"]
                },
                "CreatePlaylistRequest": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "minLength": 1, "maxLength": 200, "description": "播放列表名称" },
                        "description": { "type": "string", "nullable": true, "description": "描述" },
                        "isPublic": { "type": "boolean", "nullable": true, "default": false, "description": "是否公开" }
                    },
                    "required": ["name"]
                },
                "UpdatePlaylistRequest": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "nullable": true, "description": "新名称" },
                        "description": { "type": "string", "nullable": true, "description": "新描述" },
                        "isPublic": { "type": "boolean", "nullable": true, "description": "是否公开" }
                    }
                },
                "AddVideoToPlaylistRequest": {
                    "type": "object",
                    "properties": {
                        "video_id": { "type": "integer", "description": "要添加的视频 ID" }
                    },
                    "required": ["video_id"]
                },
                "PlaylistVideoItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "sourceType": { "type": "string" },
                        "coverUrl": { "type": "string", "nullable": true },
                        "streamUrl": { "type": "string" },
                        "category": { "type": "string" },
                        "views": { "type": "integer" },
                        "duration": { "type": "integer", "description": "时长（秒）" }
                    },
                    "required": ["id", "title", "sourceType", "streamUrl", "category", "views", "duration"]
                },
                "CommentResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "videoId": { "type": "integer" },
                        "userId": { "type": "integer" },
                        "username": { "type": "string" },
                        "avatarUrl": { "type": "string", "nullable": true },
                        "content": { "type": "string" },
                        "parentId": { "type": "integer", "nullable": true, "description": "父评论 ID（回复时存在）" },
                        "createdAt": { "type": "string", "description": "创建时间 (YYYY-MM-DD HH:MM:SS)" }
                    },
                    "required": ["id", "videoId", "userId", "username", "content", "createdAt"]
                },
                "CommentListResponse": {
                    "type": "object",
                    "properties": {
                        "comments": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/CommentResponse" }
                        },
                        "total": { "type": "integer" }
                    },
                    "required": ["comments", "total"]
                },
                "CreateCommentRequest": {
                    "type": "object",
                    "properties": {
                        "content": { "type": "string", "description": "评论内容" },
                        "parent_id": { "type": "integer", "nullable": true, "description": "父评论 ID（回复）" }
                    },
                    "required": ["content"]
                },
                "CreateShareRequest": {
                    "type": "object",
                    "properties": {
                        "expires_in_days": { "type": "integer", "nullable": true, "description": "有效天数（缺省为永不过期）" }
                    }
                },
                "CreateShareResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "videoId": { "type": "integer" },
                        "token": { "type": "string", "description": "分享令牌（仅创建时返回一次）" },
                        "shareUrl": { "type": "string", "description": "分享页面地址" },
                        "expiresAt": { "type": "string", "nullable": true },
                        "createdAt": { "type": "string" }
                    },
                    "required": ["id", "videoId", "token", "shareUrl", "createdAt"]
                },
                "UserWithStatus": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "username": { "type": "string" },
                        "approved": { "type": "boolean", "description": "是否已通过审批" },
                        "isAdmin": { "type": "boolean" },
                        "role": { "type": "integer", "description": "角色等级（1 viewer, 3 admin）" },
                        "avatarUrl": { "type": "string", "nullable": true },
                        "createdAt": { "type": "string", "description": "创建时间 (ISO 8601)" },
                        "hasActiveToken": { "type": "boolean", "description": "是否存在未过期的有效令牌" }
                    },
                    "required": ["id", "username", "approved", "isAdmin", "role", "createdAt", "hasActiveToken"]
                },
                "AdminPasswordRequest": {
                    "type": "object",
                    "properties": {
                        "password": { "type": "string", "minLength": 8, "maxLength": 128, "description": "新密码" }
                    },
                    "required": ["password"]
                },
                "BatchCategoryRequest": {
                    "type": "object",
                    "properties": {
                        "ids": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "maxItems": 1000,
                            "description": "视频 ID 列表"
                        },
                        "category": { "type": "string", "maxLength": 100, "description": "新的分类名称" }
                    },
                    "required": ["ids", "category"]
                },
                "AdminStatsResponse": {
                    "type": "object",
                    "properties": {
                        "totalVideos": { "type": "integer" },
                        "videoCount": { "type": "integer" },
                        "imageCount": { "type": "integer" },
                        "userCount": { "type": "integer" },
                        "pendingCount": { "type": "integer", "description": "待审批用户数" },
                        "totalViews": { "type": "integer" },
                        "totalDurationSecs": { "type": "integer" },
                        "byType": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": { "type": "string" },
                                    "count": { "type": "integer" }
                                }
                            }
                        },
                        "byCategory": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "category": { "type": "string" },
                                    "count": { "type": "integer" }
                                }
                            }
                        }
                    },
                    "required": ["totalVideos", "videoCount", "imageCount", "userCount", "pendingCount", "totalViews", "totalDurationSecs", "byType", "byCategory"]
                },
                "LogListResponse": {
                    "type": "object",
                    "properties": {
                        "entries": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/LogEntry" }
                        },
                        "total": { "type": "integer", "description": "本次读取的总条数（分页前）" },
                        "file": { "type": "string", "description": "日志文件名" }
                    },
                    "required": ["entries", "total", "file"]
                },
                "LogEntry": {
                    "type": "object",
                    "properties": {
                        "timestamp": { "type": "string" },
                        "level": { "type": "string" },
                        "message": { "type": "string" },
                        "method": { "type": "string", "nullable": true },
                        "path": { "type": "string", "nullable": true },
                        "status": { "type": "integer", "nullable": true },
                        "durationMs": { "type": "integer", "nullable": true },
                        "requestId": { "type": "string", "nullable": true },
                        "user": { "type": "string", "nullable": true },
                        "videoId": { "type": "integer", "nullable": true },
                        "error": { "type": "string", "nullable": true },
                        "action": { "type": "string", "nullable": true },
                        "target": { "type": "string", "nullable": true },
                        "page": { "type": "string", "nullable": true }
                    },
                    "required": ["timestamp", "level", "message"]
                }
            }
        }
    })
}
