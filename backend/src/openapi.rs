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
                    "responses": { "200": { "description": "OK" } }
                }
            },
            "/server/info": {
                "get": {
                    "summary": "Server information",
                    "operationId": "serverInfo",
                    "responses": { "200": { "description": "Server version and status" } }
                }
            },
            "/auth/register": {
                "post": {
                    "summary": "Register a new user",
                    "operationId": "authRegister",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "username": { "type": "string" },
                                        "password": { "type": "string" }
                                    },
                                    "required": ["username", "password"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Registration result with auth token" }
                    }
                }
            },
            "/auth/login": {
                "post": {
                    "summary": "Login with username and password",
                    "operationId": "authLogin",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "username": { "type": "string" },
                                        "password": { "type": "string" }
                                    },
                                    "required": ["username", "password"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": { "description": "Login result with auth token" }
                    }
                }
            },
            "/auth/logout": {
                "post": {
                    "summary": "Logout and invalidate token",
                    "operationId": "authLogout",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Logged out" } }
                }
            },
            "/auth/user": {
                "get": {
                    "summary": "Get current user info",
                    "operationId": "authUserInfo",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "User details" } }
                }
            },
            "/auth/user/profile": {
                "get": {
                    "summary": "Get user profile with stats",
                    "operationId": "authUserProfile",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Profile with watch history" } }
                }
            },
            "/videos": {
                "get": {
                    "summary": "List videos (paginated)",
                    "operationId": "listVideos",
                    "parameters": [
                        { "name": "page", "in": "query", "schema": { "type": "integer", "default": 0 } },
                        { "name": "size", "in": "query", "schema": { "type": "integer", "default": 20 } },
                        { "name": "query", "in": "query", "schema": { "type": "string" } },
                        { "name": "type", "in": "query", "schema": { "type": "string" } },
                        { "name": "category", "in": "query", "schema": { "type": "string" } }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": {
                            "description": "Paginated video list",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "items": { "type": "array", "items": { "$ref": "#/components/schemas/VideoItem" } },
                                            "total": { "type": "integer" },
                                            "page": { "type": "integer" },
                                            "size": { "type": "integer" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/videos/{id}": {
                "get": {
                    "summary": "Get single video details",
                    "operationId": "getVideo",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": {
                        "200": { "description": "Video details" },
                        "404": { "description": "Video not found" }
                    }
                }
            },
            "/playback/history": {
                "get": {
                    "summary": "List playback history for current user",
                    "operationId": "listPlaybackHistory",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Watch history list" } }
                },
                "post": {
                    "summary": "Update playback position",
                    "operationId": "updatePlaybackHistory",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "video_id": { "type": "integer" },
                                        "position_ms": { "type": "integer" },
                                        "duration_ms": { "type": "integer" }
                                    },
                                    "required": ["video_id", "position_ms"]
                                }
                            }
                        }
                    },
                    "responses": { "204": { "description": "Updated" } }
                }
            },
            "/playback/history/{video_id}": {
                "get": {
                    "summary": "Get playback position for a video",
                    "operationId": "getPlaybackHistoryForVideo",
                    "parameters": [
                        { "name": "video_id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Playback position" } }
                }
            },
            "/admin/videos/upload": {
                "post": {
                    "summary": "Upload a video file (multipart)",
                    "operationId": "uploadVideo",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string", "format": "binary" },
                                        "category": { "type": "string" },
                                        "fileHash": { "type": "string" }
                                    },
                                    "required": ["file"]
                                }
                            }
                        }
                    },
                    "responses": { "201": { "description": "Uploaded video ID" } }
                }
            },
            "/admin/videos/batch": {
                "delete": {
                    "summary": "Batch delete videos",
                    "operationId": "deleteVideos",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": { "type": "integer" }
                                }
                            }
                        }
                    },
                    "responses": { "200": { "description": "Delete result" } }
                }
            },
            "/admin/videos/{id}": {
                "put": {
                    "summary": "Update video metadata",
                    "operationId": "updateVideo",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": { "200": { "description": "Update result" } }
                },
                "delete": {
                    "summary": "Delete a video",
                    "operationId": "deleteVideo",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "integer" } }
                    ],
                    "responses": { "200": { "description": "Delete result" } }
                }
            },
            "/admin/videos/external": {
                "post": {
                    "summary": "Add external video link",
                    "operationId": "addExternalVideo",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": { "201": { "description": "Created" } }
                }
            },
            "/admin/videos/scan": {
                "post": {
                    "summary": "Scan media directory for new files",
                    "operationId": "scanMedia",
                    "security": [{ "bearerAuth": [] }, { "adminAuth": [] }],
                    "responses": { "200": { "description": "Number of new files added" } }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer"
                },
                "adminAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Requires admin privileges"
                }
            },
            "schemas": {
                "VideoItem": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "sourceType": { "type": "string" },
                        "coverUrl": { "type": "string", "nullable": true },
                        "streamUrl": { "type": "string" },
                        "category": { "type": "string" },
                        "duration": { "type": "integer" },
                        "watchPosition": { "type": "integer" }
                    },
                    "required": ["id", "title", "streamUrl"]
                }
            }
        }
    })
}
