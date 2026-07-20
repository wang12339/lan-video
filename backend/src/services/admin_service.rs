use crate::repositories::user_repo::{UserRepository, UserWithStatus};
use crate::util::password;
use crate::util::response::ErrorResponse;
use axum::{http::StatusCode, Json};

const MIN_PASSWORD_LEN: usize = 10;
const MAX_PASSWORD_LEN: usize = 128;

#[derive(Clone)]
pub struct AdminService {
    user_repo: UserRepository,
}

pub enum AdminError {
    NotFound,
    SelfAction,
    InvalidPassword,
    HashFailed,
    Internal(String),
}

impl AdminError {
    pub fn into_response(self) -> (StatusCode, Json<ErrorResponse>) {
        match self {
            AdminError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse { error: "用户不存在".into() }),
            ),
            AdminError::SelfAction => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "不能对自己执行此操作".into() }),
            ),
            AdminError::InvalidPassword => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: "密码长度需在 10-128 个字符之间".into() }),
            ),
            AdminError::HashFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: "密码加密失败".into() }),
            ),
            AdminError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: msg }),
            ),
        }
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("admin service sqlx error: {}", e);
        AdminError::Internal("数据库错误".into())
    }
}

/// Result of an admin user-management action. `error_msg` is None on success.
pub struct ActionOutcome {
    pub ok: bool,
    pub error_msg: Option<String>,
    pub deleted_count: Option<i64>,
    pub new_role: Option<i16>,
}

impl AdminService {
    pub fn new(user_repo: UserRepository) -> Self {
        Self { user_repo }
    }

    pub async fn list_users(&self) -> Result<Vec<UserWithStatus>, AdminError> {
        let users = self.user_repo.list_users().await?;
        Ok(users)
    }

    pub async fn delete_user(
        &self,
        target_id: i64,
        actor_id: i64,
    ) -> Result<ActionOutcome, AdminError> {
        if target_id == actor_id {
            return Err(AdminError::SelfAction);
        }
        let deleted = self.user_repo.delete_user(target_id).await?;
        if deleted {
            Ok(ActionOutcome {
                ok: true,
                error_msg: None,
                deleted_count: None,
                new_role: None,
            })
        } else {
            Ok(ActionOutcome {
                ok: false,
                error_msg: Some("用户不存在".into()),
                deleted_count: None,
                new_role: None,
            })
        }
    }

    pub async fn reset_user_password(
        &self,
        target_id: i64,
        new_password: &str,
    ) -> Result<ActionOutcome, AdminError> {
        if new_password.len() < MIN_PASSWORD_LEN || new_password.len() > MAX_PASSWORD_LEN {
            return Err(AdminError::InvalidPassword);
        }
        let lower = new_password.to_ascii_lowercase();
        let weak_list = [
            "password", "qwerty", "12345678", "iloveyou", "admin1234", "welcome12",
            "11111111", "00000000", "dragon123", "monkey123",
        ];
        if weak_list.iter().any(|w| lower == *w) {
            return Err(AdminError::InvalidPassword);
        }
        let hash = password::hash(new_password).map_err(|_| AdminError::HashFailed)?;
        let ok = self.user_repo.update_password_hash(target_id, &hash).await?;
        // Invalidate all tokens for this user so they must re-login
        let _ = self.user_repo.delete_tokens_by_user_id(target_id).await;
        Ok(ActionOutcome {
            ok,
            error_msg: if !ok { Some("用户不存在".into()) } else { None },
            deleted_count: None,
            new_role: None,
        })
    }

    pub async fn toggle_user_admin(
        &self,
        target_id: i64,
        actor_id: i64,
    ) -> Result<ActionOutcome, AdminError> {
        if target_id == actor_id {
            return Err(AdminError::SelfAction);
        }
        let new_state = self.user_repo.toggle_admin(target_id).await?;
        let role: Option<i16> = if new_state {
            self.user_repo.get_user_role(target_id).await?
        } else {
            None
        };
        Ok(ActionOutcome {
            ok: new_state,
            error_msg: if !new_state { Some("用户不存在".into()) } else { None },
            deleted_count: None,
            new_role: role,
        })
    }

    pub async fn approve_user(
        &self,
        target_id: i64,
        approved: bool,
    ) -> Result<ActionOutcome, AdminError> {
        let ok = self.user_repo.approve_user(target_id, approved).await?;
        Ok(ActionOutcome {
            ok,
            error_msg: if !ok { Some("用户不存在".into()) } else { None },
            deleted_count: None,
            new_role: None,
        })
    }

    pub async fn kick_user(&self, target_id: i64) -> Result<i64, AdminError> {
        let deleted = self.user_repo.delete_tokens_by_user_id(target_id).await?;
        Ok(deleted as i64)
    }
}
