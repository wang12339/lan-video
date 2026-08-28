use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Plan {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub max_users: i32,
    pub max_storage_bytes: i64,
    pub max_upload_size_mb: i32,
    pub max_videos_per_user: i32,
    pub storage_quota_gb: i32,
    pub registration_enabled: bool,
    pub is_active: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanConfig {
    pub plan_id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub max_users: i32,
    pub max_storage_bytes: i64,
    pub max_upload_size_mb: i32,
    pub max_videos_per_user: i32,
    pub storage_quota_gb: i32,
    pub registration_enabled: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatePlanRequest {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub max_users: i32,
    pub max_storage_bytes: i64,
    pub max_upload_size_mb: i32,
    pub max_videos_per_user: i32,
    pub storage_quota_gb: i32,
    pub registration_enabled: bool,
    pub sort_order: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdatePlanRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub max_users: Option<i32>,
    pub max_storage_bytes: Option<i64>,
    pub max_upload_size_mb: Option<i32>,
    pub max_videos_per_user: Option<i32>,
    pub storage_quota_gb: Option<i32>,
    pub registration_enabled: Option<bool>,
    pub sort_order: Option<i32>,
}

/// 查询超时时间（秒）
const QUERY_TIMEOUT_SECS: u64 = 5;
/// 最大重试次数
const MAX_RETRIES: u32 = 3;
/// 重试基础延迟（毫秒）
const RETRY_BASE_DELAY_MS: u64 = 100;

/// 执行带超时和重试的数据库查询
async fn execute_with_retry<T, F, Fut>(label: &str, query_fn: F) -> Result<T, sqlx::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(QUERY_TIMEOUT_SECS),
            query_fn(),
        )
        .await;

        match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(e)) => {
                tracing::warn!(
                    query = %label,
                    attempt = attempt + 1,
                    error = %e,
                    "query failed, retrying..."
                );
                last_error = Some(e);
            }
            Err(_timeout) => {
                last_error = Some(sqlx::Error::PoolTimedOut);
                tracing::warn!(
                    query = %label,
                    attempt = attempt + 1,
                    timeout_secs = QUERY_TIMEOUT_SECS,
                    "query timed out, retrying..."
                );
            }
        }

        if attempt < MAX_RETRIES - 1 {
            let delay_ms = RETRY_BASE_DELAY_MS * 2u64.pow(attempt);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }

    Err(last_error.unwrap_or(sqlx::Error::PoolTimedOut))
}

#[derive(Clone)]
pub struct PlanRepository {
    pool: PgPool,
}

impl PlanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 获取所有活跃套餐
    pub async fn list_active(&self) -> Result<Vec<PlanConfig>, sqlx::Error> {
        let plans = execute_with_retry("plan_list_active", || {
            sqlx::query_as::<_, Plan>(
                r#"SELECT id, name, slug, description, max_users, max_storage_bytes, 
                          max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                          registration_enabled, is_active, sort_order
                   FROM plans 
                   WHERE is_active = true 
                   ORDER BY sort_order, id"#,
            )
            .fetch_all(&self.pool)
        })
        .await?;

        Ok(plans.iter().map(Self::to_config).collect())
    }

    /// 获取所有套餐（包括禁用的）
    pub async fn list_all(&self) -> Result<Vec<PlanConfig>, sqlx::Error> {
        let plans = execute_with_retry("plan_list_all", || {
            sqlx::query_as::<_, Plan>(
                r#"SELECT id, name, slug, description, max_users, max_storage_bytes, 
                          max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                          registration_enabled, is_active, sort_order
                   FROM plans 
                   ORDER BY sort_order, id"#,
            )
            .fetch_all(&self.pool)
        })
        .await?;

        Ok(plans.iter().map(Self::to_config).collect())
    }

    /// 根据 ID 获取套餐
    pub async fn get_by_id(&self, plan_id: i64) -> Result<Option<PlanConfig>, sqlx::Error> {
        let plan = execute_with_retry("plan_get_by_id", || {
            sqlx::query_as::<_, Plan>(
                r#"SELECT id, name, slug, description, max_users, max_storage_bytes, 
                          max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                          registration_enabled, is_active, sort_order
                   FROM plans 
                   WHERE id = $1"#,
            )
            .bind(plan_id)
            .fetch_optional(&self.pool)
        })
        .await?;

        Ok(plan.map(|p| Self::to_config(&p)))
    }

    /// 根据 slug 获取套餐
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<PlanConfig>, sqlx::Error> {
        let plan = execute_with_retry("plan_get_by_slug", || {
            sqlx::query_as::<_, Plan>(
                r#"SELECT id, name, slug, description, max_users, max_storage_bytes, 
                          max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                          registration_enabled, is_active, sort_order
                   FROM plans 
                   WHERE slug = $1"#,
            )
            .bind(slug)
            .fetch_optional(&self.pool)
        })
        .await?;

        Ok(plan.map(|p| Self::to_config(&p)))
    }

    /// 创建套餐
    pub async fn create(&self, req: &CreatePlanRequest) -> Result<PlanConfig, sqlx::Error> {
        let plan = execute_with_retry("plan_create", || {
            sqlx::query_as::<_, Plan>(
                r#"INSERT INTO plans (name, slug, description, max_users, max_storage_bytes, 
                                     max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                                     registration_enabled, sort_order)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                   RETURNING id, name, slug, description, max_users, max_storage_bytes, 
                             max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                             registration_enabled, is_active, sort_order"#,
            )
            .bind(&req.name)
            .bind(&req.slug)
            .bind(&req.description)
            .bind(req.max_users)
            .bind(req.max_storage_bytes)
            .bind(req.max_upload_size_mb)
            .bind(req.max_videos_per_user)
            .bind(req.storage_quota_gb)
            .bind(req.registration_enabled)
            .bind(req.sort_order.unwrap_or(0))
            .fetch_one(&self.pool)
        })
        .await?;

        Ok(Self::to_config(&plan))
    }

    /// 更新套餐
    pub async fn update(
        &self,
        plan_id: i64,
        req: &UpdatePlanRequest,
    ) -> Result<Option<PlanConfig>, sqlx::Error> {
        let plan = execute_with_retry("plan_update", || {
            sqlx::query_as::<_, Plan>(
                r#"UPDATE plans 
                   SET name = COALESCE($2, name),
                       description = COALESCE($3, description),
                       max_users = COALESCE($4, max_users),
                       max_storage_bytes = COALESCE($5, max_storage_bytes),
                       max_upload_size_mb = COALESCE($6, max_upload_size_mb),
                       max_videos_per_user = COALESCE($7, max_videos_per_user),
                       storage_quota_gb = COALESCE($8, storage_quota_gb),
                       registration_enabled = COALESCE($9, registration_enabled),
                       sort_order = COALESCE($10, sort_order),
                       updated_at = CURRENT_TIMESTAMP
                   WHERE id = $1
                   RETURNING id, name, slug, description, max_users, max_storage_bytes, 
                             max_upload_size_mb, max_videos_per_user, storage_quota_gb, 
                             registration_enabled, is_active, sort_order"#,
            )
            .bind(plan_id)
            .bind(&req.name)
            .bind(&req.description)
            .bind(req.max_users)
            .bind(req.max_storage_bytes)
            .bind(req.max_upload_size_mb)
            .bind(req.max_videos_per_user)
            .bind(req.storage_quota_gb)
            .bind(req.registration_enabled)
            .bind(req.sort_order)
            .fetch_one(&self.pool)
        })
        .await?;

        Ok(Some(Self::to_config(&plan)))
    }

    /// 启用/禁用套餐
    pub async fn set_active(&self, plan_id: i64, is_active: bool) -> Result<bool, sqlx::Error> {
        let rows = execute_with_retry("plan_set_active", || {
            sqlx::query(
                "UPDATE plans SET is_active = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
            )
            .bind(plan_id)
            .bind(is_active)
            .execute(&self.pool)
        })
        .await?;

        Ok(rows.rows_affected() > 0)
    }

    /// 删除套餐
    pub async fn delete(&self, plan_id: i64) -> Result<bool, sqlx::Error> {
        let rows = execute_with_retry("plan_delete", || {
            sqlx::query("DELETE FROM plans WHERE id = $1")
                .bind(plan_id)
                .execute(&self.pool)
        })
        .await?;

        Ok(rows.rows_affected() > 0)
    }

    fn to_config(plan: &Plan) -> PlanConfig {
        PlanConfig {
            plan_id: plan.id,
            name: plan.name.clone(),
            slug: plan.slug.clone(),
            description: plan.description.clone(),
            max_users: plan.max_users,
            max_storage_bytes: plan.max_storage_bytes,
            max_upload_size_mb: plan.max_upload_size_mb,
            max_videos_per_user: plan.max_videos_per_user,
            storage_quota_gb: plan.storage_quota_gb,
            registration_enabled: plan.registration_enabled,
            sort_order: plan.sort_order,
        }
    }
}
