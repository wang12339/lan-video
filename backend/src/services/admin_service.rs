use crate::repositories::user_repo::{UserRepository, UserWithStatus};
use crate::util::error::ServiceError;
use crate::util::password;

const MIN_PASSWORD_LEN: usize = 10;
const MAX_PASSWORD_LEN: usize = 128;

#[derive(Clone)]
pub struct AdminService {
    user_repo: UserRepository,
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

    pub async fn list_users(&self, tenant_id: i64) -> Result<Vec<UserWithStatus>, ServiceError> {
        let users = self.user_repo.list_users(tenant_id).await?;
        Ok(users)
    }

    pub async fn delete_user(
        &self,
        target_id: i64,
        actor_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if target_id == actor_id {
            return Err(ServiceError::bad_request("不能对自己执行此操作"));
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
    ) -> Result<ActionOutcome, ServiceError> {
        if new_password.chars().count() < MIN_PASSWORD_LEN
            || new_password.chars().count() > MAX_PASSWORD_LEN
        {
            return Err(ServiceError::bad_request("密码长度需在 10-128 个字符之间"));
        }
        let lower = new_password.to_ascii_lowercase();
        let weak_list = [
            "password",
            "qwerty",
            "12345678",
            "iloveyou",
            "admin1234",
            "welcome12",
            "11111111",
            "00000000",
            "dragon123",
            "monkey123",
        ];
        if weak_list.iter().any(|w| lower == *w) {
            return Err(ServiceError::bad_request("密码长度需在 10-128 个字符之间"));
        }
        let hash = password::hash(new_password).map_err(|e| {
            tracing::error!("password hash failed: {}", e);
            ServiceError::internal("密码加密失败")
        })?;
        let ok = self
            .user_repo
            .update_password_hash(target_id, &hash)
            .await?;
        // Invalidate all tokens for this user so they must re-login
        if let Err(e) = self.user_repo.delete_tokens_by_user_id(target_id).await {
            tracing::error!(
                "Failed to invalidate tokens after password reset for user {}: {}",
                target_id,
                e
            );
        }
        Ok(ActionOutcome {
            ok,
            error_msg: if !ok {
                Some("用户不存在".into())
            } else {
                None
            },
            deleted_count: None,
            new_role: None,
        })
    }

    pub async fn toggle_user_admin(
        &self,
        target_id: i64,
        actor_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if target_id == actor_id {
            return Err(ServiceError::bad_request("不能对自己执行此操作"));
        }
        let new_state = self.user_repo.toggle_admin(target_id).await?;
        let role: Option<i16> = if new_state {
            self.user_repo.get_user_role(target_id).await?
        } else {
            None
        };
        Ok(ActionOutcome {
            ok: new_state,
            error_msg: if !new_state {
                Some("用户不存在".into())
            } else {
                None
            },
            deleted_count: None,
            new_role: role,
        })
    }

    pub async fn approve_user(
        &self,
        target_id: i64,
        approved: bool,
    ) -> Result<ActionOutcome, ServiceError> {
        if approved {
            let ok = self.user_repo.approve_user(target_id, true).await?;
            Ok(ActionOutcome {
                ok,
                error_msg: if !ok {
                    Some("用户不存在".into())
                } else {
                    None
                },
                deleted_count: None,
                new_role: None,
            })
        } else {
            let deleted = self.user_repo.delete_user(target_id).await?;
            Ok(ActionOutcome {
                ok: deleted,
                error_msg: if !deleted {
                    Some("用户不存在".into())
                } else {
                    None
                },
                deleted_count: None,
                new_role: None,
            })
        }
    }

    pub async fn kick_user(&self, target_id: i64) -> Result<i64, ServiceError> {
        let deleted = self.user_repo.delete_tokens_by_user_id(target_id).await?;
        Ok(deleted as i64)
    }
}
