use serde_json::json;

/// Build the OpenAPI 3.1 spec for the ATMOS API as a JSON value.
/// Generated manually to avoid invasive per-handler annotations.
pub fn spec() -> serde_json::Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "ATMOS API",
            "description": "局域网视频播放平台 — REST API",
            "version": "0.1.0"
        },
        "servers": [
            { "url": "/", "description": "Same-origin (reverse proxy)" },
            { "url": "http://localhost:8082", "description": "Local dev server" }
        ],
        "paths": {
            "/health": {
                "get": {
                    "summary": "Health check",
                    "operationId": "health",
                    "description": "Liveness probe — returns 200 if the server is running",
                    "responses": {
                        "200": {
                            "description": "Server is healthy",
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
                    "description": "Create a new user account. The first user registered becomes admin. Rate-limited per username.",
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
                },
                "get": {
                    "summary": "Get transcode status",
                    "operationId": "getTranscodeStatus",
                    "description": "Get transcoding status and available variants for a video",
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
                    "responses": {
                        "200": {
                            "description": "Prometheus metrics text",
                            "content": { "text/plain": { "schema": { "type": "string" } } }
                        }
                    }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "JWT or opaque token returned by /auth/login or /auth/register"
                },
                "adminAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Requires admin privileges (first registered user is admin)"
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
                        "status": { "type": "string", "example": "ok" }
                    },
                    "required": ["status"]
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
                        "username": { "type": "string" },
                        "isAdmin": { "type": "boolean" },
                        "createdAt": { "type": "string", "description": "Account creation timestamp (YYYY-MM-DD HH:MM:SS)" }
                    },
                    "required": ["username", "isAdmin", "createdAt"]
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
                        "watchPosition": { "type": "integer", "nullable": true, "description": "Current user's watch position in ms" }
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
                }
            }
        }
    })
}
