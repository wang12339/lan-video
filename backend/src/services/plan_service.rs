use crate::repositories::plan_repo::{
    CreatePlanRequest, PlanConfig, PlanRepository, UpdatePlanRequest,
};
use crate::util::error::ServiceError;

#[derive(Clone)]
pub struct PlanService {
    plan_repo: PlanRepository,
}

impl PlanService {
    pub fn new(plan_repo: PlanRepository) -> Self {
        Self { plan_repo }
    }

    /// 获取所有活跃套餐
    pub async fn list_active_plans(&self) -> Result<Vec<PlanConfig>, ServiceError> {
        self.plan_repo
            .list_active()
            .await
            .map_err(|e| ServiceError::internal(format!("获取套餐列表失败: {}", e)))
    }

    /// 获取所有套餐（包括禁用的）
    pub async fn list_all_plans(&self) -> Result<Vec<PlanConfig>, ServiceError> {
        self.plan_repo
            .list_all()
            .await
            .map_err(|e| ServiceError::internal(format!("获取套餐列表失败: {}", e)))
    }

    /// 根据 ID 获取套餐
    pub async fn get_plan(&self, plan_id: i64) -> Result<PlanConfig, ServiceError> {
        self.plan_repo
            .get_by_id(plan_id)
            .await
            .map_err(|e| ServiceError::internal(format!("获取套餐失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("套餐不存在"))
    }

    /// 创建套餐
    pub async fn create_plan(&self, req: CreatePlanRequest) -> Result<PlanConfig, ServiceError> {
        // 验证参数
        if req.name.trim().is_empty() {
            return Err(ServiceError::validation("套餐名称不能为空"));
        }
        if req.slug.trim().is_empty() {
            return Err(ServiceError::validation("套餐标识不能为空"));
        }
        if !req
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ServiceError::validation(
                "套餐标识只能包含小写字母、数字和连字符",
            ));
        }
        if req.max_users <= 0 {
            return Err(ServiceError::validation("最大用户数必须大于 0"));
        }
        if req.max_storage_bytes <= 0 {
            return Err(ServiceError::validation("最大存储空间必须大于 0"));
        }
        if req.max_upload_size_mb <= 0 || req.max_upload_size_mb > 10240 {
            return Err(ServiceError::validation(
                "上传文件大小限制须在 1-10240 MB 之间",
            ));
        }
        if req.max_videos_per_user <= 0 {
            return Err(ServiceError::validation("用户视频数量上限必须大于 0"));
        }
        if req.storage_quota_gb <= 0 {
            return Err(ServiceError::validation("存储配额必须大于 0"));
        }

        self.plan_repo.create(&req).await.map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                ServiceError::validation("套餐名称或标识已存在")
            } else {
                ServiceError::internal(format!("创建套餐失败: {}", e))
            }
        })
    }

    /// 更新套餐
    pub async fn update_plan(
        &self,
        plan_id: i64,
        req: UpdatePlanRequest,
    ) -> Result<PlanConfig, ServiceError> {
        // 验证参数
        if let Some(ref name) = req.name {
            if name.trim().is_empty() {
                return Err(ServiceError::validation("套餐名称不能为空"));
            }
        }
        if let Some(max_users) = req.max_users {
            if max_users <= 0 {
                return Err(ServiceError::validation("最大用户数必须大于 0"));
            }
        }
        if let Some(max_storage_bytes) = req.max_storage_bytes {
            if max_storage_bytes <= 0 {
                return Err(ServiceError::validation("最大存储空间必须大于 0"));
            }
        }
        if let Some(max_upload_size_mb) = req.max_upload_size_mb {
            if max_upload_size_mb <= 0 || max_upload_size_mb > 10240 {
                return Err(ServiceError::validation(
                    "上传文件大小限制须在 1-10240 MB 之间",
                ));
            }
        }
        if let Some(max_videos_per_user) = req.max_videos_per_user {
            if max_videos_per_user <= 0 {
                return Err(ServiceError::validation("用户视频数量上限必须大于 0"));
            }
        }
        if let Some(storage_quota_gb) = req.storage_quota_gb {
            if storage_quota_gb <= 0 {
                return Err(ServiceError::validation("存储配额必须大于 0"));
            }
        }

        self.plan_repo
            .update(plan_id, &req)
            .await
            .map_err(|e| ServiceError::internal(format!("更新套餐失败: {}", e)))?
            .ok_or_else(|| ServiceError::not_found("套餐不存在"))
    }

    /// 启用/禁用套餐
    pub async fn set_status(&self, plan_id: i64, active: bool) -> Result<(), ServiceError> {
        self.plan_repo
            .set_active(plan_id, active)
            .await
            .map_err(|e| ServiceError::internal(format!("更新套餐状态失败: {}", e)))?;
        Ok(())
    }

    /// 删除套餐
    pub async fn delete_plan(&self, plan_id: i64) -> Result<(), ServiceError> {
        self.plan_repo
            .delete(plan_id)
            .await
            .map_err(|e| ServiceError::internal(format!("删除套餐失败: {}", e)))?;
        Ok(())
    }
}
