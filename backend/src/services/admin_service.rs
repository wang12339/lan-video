use crate::repositories::user_repo::{UserRepository, UserWithStatus};
use crate::util::error::ServiceError;
use crate::util::password;
use crate::util::password::{MAX_PASSWORD_LEN, MIN_PASSWORD_LEN};

/// Service layer for administrative user-management operations.
///
/// Provides methods to list, delete, kick, approve, and modify user accounts
/// within a tenant. All mutation methods return an [`ActionOutcome`] indicating
/// success or failure with an optional error message.
#[derive(Clone)]
pub struct AdminService {
    user_repo: UserRepository,
}

/// Result of an admin user-management action.
///
/// `error_msg` is `None` on success, and contains a human-readable
/// error description when the action could not be completed (e.g. user not found).
pub struct ActionOutcome {
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Human-readable error message, or `None` on success.
    pub error_msg: Option<String>,
    /// Number of records deleted (used by bulk-delete operations).
    pub deleted_count: Option<i64>,
    /// The user's new role value after the action, if applicable.
    pub new_role: Option<i16>,
}

impl AdminService {
    /// Creates a new `AdminService` backed by the given user repository.
    pub fn new(user_repo: UserRepository) -> Self {
        Self { user_repo }
    }

    /// Lists all users belonging to the specified tenant.
    ///
    /// Returns each user together with their online/offline status.
    pub async fn list_users(&self, tenant_id: i64) -> Result<Vec<UserWithStatus>, ServiceError> {
        let users = self.user_repo.list_users(tenant_id).await?;
        Ok(users)
    }

    /// Deletes the user identified by `target_id`.
    ///
    /// Returns an error if the actor attempts to delete themselves.
    /// Returns `ok: false` with an error message if the target user does not exist.
    pub async fn delete_user(
        &self,
        target_id: i64,
        actor_id: i64,
        actor_tenant_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if target_id == actor_id {
            return Err(ServiceError::bad_request("不能对自己执行此操作"));
        }
        if !self.user_in_tenant(target_id, actor_tenant_id).await? {
            return Ok(ActionOutcome {
                ok: false,
                error_msg: Some("用户不存在".into()),
                deleted_count: None,
                new_role: None,
            });
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

    /// Resets the password for the user identified by `target_id`.
    ///
    /// The new password is validated against length constraints and a weak-password list,
    /// then hashed and persisted. All existing auth tokens for the user are invalidated
    /// to force a re-login.
    ///
    /// Returns an error if the password does not meet policy requirements.
    pub async fn reset_user_password(
        &self,
        target_id: i64,
        new_password: &str,
        actor_tenant_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if new_password.chars().count() < MIN_PASSWORD_LEN
            || new_password.chars().count() > MAX_PASSWORD_LEN
        {
            return Err(ServiceError::bad_request(format!(
                "密码长度需在 {}-{} 个字符之间",
                MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
            )));
        }
        if !self.user_in_tenant(target_id, actor_tenant_id).await? {
            return Ok(ActionOutcome {
                ok: false,
                error_msg: Some("用户不存在".into()),
                deleted_count: None,
                new_role: None,
            });
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
            return Err(ServiceError::bad_request(format!(
                "密码长度需在 {}-{} 个字符之间",
                MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
            )));
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

    /// Toggles the admin status of the user identified by `target_id`.
    ///
    /// Returns an error if the actor attempts to toggle their own admin status.
    /// On success, returns the user's updated role in `new_role`.
    pub async fn toggle_user_admin(
        &self,
        target_id: i64,
        actor_id: i64,
        actor_tenant_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if target_id == actor_id {
            return Err(ServiceError::bad_request("不能对自己执行此操作"));
        }
        if !self.user_in_tenant(target_id, actor_tenant_id).await? {
            return Ok(ActionOutcome {
                ok: false,
                error_msg: Some("用户不存在".into()),
                deleted_count: None,
                new_role: None,
            });
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

    /// Approves or rejects a pending user registration.
    ///
    /// When `approved` is `true`, the user is marked as approved and can log in.
    /// When `approved` is `false`, the user record is permanently deleted.
    pub async fn approve_user(
        &self,
        target_id: i64,
        approved: bool,
        actor_tenant_id: i64,
    ) -> Result<ActionOutcome, ServiceError> {
        if !self.user_in_tenant(target_id, actor_tenant_id).await? {
            return Ok(ActionOutcome {
                ok: false,
                error_msg: Some("用户不存在".into()),
                deleted_count: None,
                new_role: None,
            });
        }
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

    /// Kicks a user by invalidating all of their active auth tokens.
    ///
    /// Returns the number of tokens that were deleted, which effectively forces
    /// the user to re-authenticate on all devices.
    pub async fn kick_user(
        &self,
        target_id: i64,
        actor_tenant_id: i64,
    ) -> Result<i64, ServiceError> {
        if !self.user_in_tenant(target_id, actor_tenant_id).await? {
            return Err(ServiceError::not_found("用户不存在"));
        }
        let deleted = self.user_repo.delete_tokens_by_user_id(target_id).await?;
        Ok(deleted as i64)
    }

    /// Admin user-management endpoints must verify the target user lives in the
    /// acting admin's tenant before mutating by bare user id (cross-tenant IDOR).
    async fn user_in_tenant(&self, user_id: i64, tenant_id: i64) -> Result<bool, ServiceError> {
        self.user_repo
            .user_in_tenant(user_id, tenant_id)
            .await
            .map_err(ServiceError::from)
    }
}
