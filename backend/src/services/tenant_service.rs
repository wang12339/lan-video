use serde::Serialize;

use crate::repositories::tenant_repo::{TenantConfig, TenantRepository, TenantSettings};
use crate::util::error::ServiceError;

/// 租户统计数据
#[derive(Clone, Debug, Serialize)]
pub struct TenantStats {
    /// 租户 ID
    pub tenant_id: i64,
    /// 租户 slug
    pub slug: String,
    /// 租户名称
    pub name: String,
    /// 用户总数
    pub user_count: i64,
    /// 视频总数
    pub video_count: i64,
    /// 已用存储（字节）
    pub storage_used_bytes: f64,
    /// 存储上限（字节）
    pub max_storage_bytes: i64,
    /// 存储使用率（百分比）
    pub storage_usage_percent: f64,
}

/// 租户服务层，负责租户信息查询、配置管理和统计数据聚合。
///
/// 所有方法通过 [`TenantRepository`] 访问数据库，遵循 handler → service → repository 分层规则。
#[derive(Clone)]
pub struct TenantService {
    tenant_repo: TenantRepository,
}

impl TenantService {
    /// 创建租户服务实例。
    ///
    /// # 参数
    /// * `tenant_repo` - 租户数据仓库
    pub fn new(tenant_repo: TenantRepository) -> Self {
        Self { tenant_repo }
    }

    /// 获取所有租户列表。
    ///
    /// # 返回
    /// * `Ok(Vec<TenantConfig>)` - 租户配置列表
    /// * `Err(ServiceError::Internal)` - 数据库查询失败
    pub async fn list_tenants(&self) -> Result<Vec<TenantConfig>, ServiceError> {
        self.tenant_repo
            .list_all()
            .await
            .map_err(|e| ServiceError::internal(format!("获取租户列表失败: {}", e)))
    }

    /// 根据 ID 获取租户完整配置信息。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    ///
    /// # 返回
    /// * `Ok(TenantConfig)` - 租户配置
    /// * `Err(ServiceError::NotFound)` - 租户不存在
    /// * `Err(ServiceError::Internal)` - 数据库查询失败
    pub async fn get_tenant(&self, tenant_id: i64) -> Result<TenantConfig, ServiceError> {
        self.tenant_repo
            .get_by_id(tenant_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取租户失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("租户不存在"))
    }

    /// 更新租户配置。
    ///
    /// 更新前会验证配置的合法性（上传大小限制、存储配额等），
    /// 验证失败返回 [`ServiceError::Validation`]。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    /// * `settings` - 新的租户配置
    ///
    /// # 返回
    /// * `Ok(())` - 更新成功
    /// * `Err(ServiceError::NotFound)` - 租户不存在
    /// * `Err(ServiceError::Validation)` - 配置验证失败
    /// * `Err(ServiceError::Internal)` - 数据库更新失败
    pub async fn update_settings(
        &self,
        tenant_id: i64,
        settings: TenantSettings,
    ) -> Result<(), ServiceError> {
        // 验证配置
        self.validate_settings(&settings)?;

        // 确认租户存在
        self.tenant_repo
            .get_by_id(tenant_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取租户失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("租户不存在"))?;

        self.tenant_repo
            .update_settings(tenant_id, settings)
            .await
            .map_err(|e| ServiceError::internal(format!("更新租户配置失败: {}", e)))?;

        Ok(())
    }

    /// 获取租户使用统计。
    ///
    /// 返回用户数、视频数、存储使用量和存储上限。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    ///
    /// # 返回
    /// * `Ok(TenantStats)` - 租户统计数据
    /// * `Err(ServiceError::NotFound)` - 租户不存在
    /// * `Err(ServiceError::Internal)` - 数据库查询失败
    pub async fn get_stats(&self, tenant_id: i64) -> Result<TenantStats, ServiceError> {
        let row = self
            .tenant_repo
            .get_usage_stats(tenant_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取租户统计失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("租户不存在"))?;

        Ok(TenantStats {
            tenant_id: row.tenant_id,
            slug: row.slug,
            name: row.name,
            user_count: row.user_count,
            video_count: row.video_count,
            storage_used_bytes: row.storage_used_bytes,
            max_storage_bytes: row.max_storage_bytes,
            storage_usage_percent: row.storage_usage_percent,
        })
    }

    /// 设置租户状态（启用/禁用）。
    ///
    /// # 参数
    /// * `tenant_id` - 租户 ID
    /// * `active` - 是否启用
    ///
    /// # 返回
    /// * `Ok(())` - 操作成功
    /// * `Err(ServiceError::NotFound)` - 租户不存在
    /// * `Err(ServiceError::Internal)` - 数据库更新失败
    pub async fn set_status(&self, tenant_id: i64, active: bool) -> Result<(), ServiceError> {
        // 确认租户存在
        self.tenant_repo
            .get_by_id(tenant_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取租户失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("租户不存在"))?;

        self.tenant_repo
            .set_active(tenant_id, active)
            .await
            .map_err(|e| ServiceError::internal(format!("更新租户状态失败: {}", e)))?;

        Ok(())
    }

    /// 创建新租户。
    ///
    /// # 参数
    /// * `name` - 租户名称
    /// * `slug` - 租户唯一标识（用于子域名）
    /// * `custom_domain` - 自定义域名（可选）
    /// * `plan` - 套餐类型
    /// * `max_users` - 最大用户数
    /// * `max_storage_bytes` - 最大存储空间（字节）
    /// * `settings` - 租户配置
    ///
    /// # 返回
    /// * `Ok(TenantConfig)` - 创建成功
    /// * `Err(ServiceError::Validation)` - 参数验证失败
    /// * `Err(ServiceError::Internal)` - 数据库错误（如 slug 重复）
    #[allow(clippy::too_many_arguments)]
    pub async fn create_tenant(
        &self,
        name: &str,
        slug: &str,
        custom_domain: Option<&str>,
        plan: &str,
        max_users: i32,
        max_storage_bytes: i64,
        settings: TenantSettings,
    ) -> Result<TenantConfig, ServiceError> {
        // 验证配置
        self.validate_settings(&settings)?;

        // 验证必填参数
        if name.trim().is_empty() {
            return Err(ServiceError::validation("租户名称不能为空"));
        }
        if slug.trim().is_empty() {
            return Err(ServiceError::validation("租户标识不能为空"));
        }
        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ServiceError::validation(
                "租户标识只能包含小写字母、数字和连字符",
            ));
        }
        if max_users <= 0 {
            return Err(ServiceError::validation("最大用户数必须大于 0"));
        }
        if max_storage_bytes <= 0 {
            return Err(ServiceError::validation("最大存储空间必须大于 0"));
        }

        self.tenant_repo
            .create(
                name,
                slug,
                custom_domain,
                plan,
                max_users,
                max_storage_bytes,
                &settings,
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                    ServiceError::validation("租户标识或域名已存在")
                } else {
                    ServiceError::internal(format!("创建租户失败: {}", e))
                }
            })
    }

    /// 验证租户配置的合法性。
    ///
    /// # 校验规则
    /// - `max_upload_size_mb` 必须在 1..=10240 范围内（最大 10 GB）
    /// - `max_videos_per_user` 必须大于 0
    /// - `storage_quota_gb` 必须大于 0
    /// - `custom_theme` 如果存在，长度不超过 64 字符
    fn validate_settings(&self, settings: &TenantSettings) -> Result<(), ServiceError> {
        if settings.max_upload_size_mb == 0 || settings.max_upload_size_mb > 10240 {
            return Err(ServiceError::validation(
                "上传文件大小限制须在 1-10240 MB 之间",
            ));
        }

        if settings.max_videos_per_user == 0 {
            return Err(ServiceError::validation("用户视频数量上限必须大于 0"));
        }

        if settings.storage_quota_gb == 0 {
            return Err(ServiceError::validation("存储配额必须大于 0"));
        }

        if let Some(ref theme) = settings.custom_theme {
            if theme.len() > 64 {
                return Err(ServiceError::validation(
                    "自定义主题标识长度不能超过 64 个字符",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_settings() -> TenantSettings {
        TenantSettings {
            max_upload_size_mb: 500,
            max_videos_per_user: 100,
            registration_enabled: true,
            custom_theme: None,
            storage_quota_gb: 50,
        }
    }

    #[tokio::test]
    async fn validate_settings_valid() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let settings = default_settings();
        assert!(service.validate_settings(&settings).is_ok());
    }

    #[tokio::test]
    async fn validate_settings_zero_upload_size() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let mut settings = default_settings();
        settings.max_upload_size_mb = 0;
        let err = service.validate_settings(&settings).unwrap_err();
        match err {
            ServiceError::Validation(msg) => assert!(msg.contains("上传文件大小")),
            _ => panic!("expected Validation error"),
        }
    }

    #[tokio::test]
    async fn validate_settings_excessive_upload_size() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let mut settings = default_settings();
        settings.max_upload_size_mb = 10241;
        assert!(service.validate_settings(&settings).is_err());
    }

    #[tokio::test]
    async fn validate_settings_zero_videos() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let mut settings = default_settings();
        settings.max_videos_per_user = 0;
        assert!(service.validate_settings(&settings).is_err());
    }

    #[tokio::test]
    async fn validate_settings_zero_storage_quota() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let mut settings = default_settings();
        settings.storage_quota_gb = 0;
        assert!(service.validate_settings(&settings).is_err());
    }

    #[tokio::test]
    async fn validate_settings_long_theme() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let mut settings = default_settings();

        // 65 字符 — 超限
        settings.custom_theme = Some("a".repeat(65));
        assert!(service.validate_settings(&settings).is_err());

        // 64 字符 — 恰好在边界内
        settings.custom_theme = Some("a".repeat(64));
        assert!(service.validate_settings(&settings).is_ok());
    }

    #[tokio::test]
    async fn validate_settings_none_theme_is_ok() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(1))
            .connect_lazy("postgres://127.0.0.1:1/test")
            .expect("lazy pool");
        let service =
            TenantService::new(TenantRepository::new(pool, "http://localhost:3000".into()));
        let settings = default_settings();
        assert!(settings.custom_theme.is_none());
        assert!(service.validate_settings(&settings).is_ok());
    }
}
