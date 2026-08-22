use crate::repositories::share_repo::{ShareLink, ShareRepository};
use crate::util::error::ServiceError;
use rand::Rng;

/// 分享链接服务，负责创建、管理视频分享链接。
#[derive(Clone)]
pub struct ShareService {
    repo: ShareRepository,
}

impl ShareService {
    /// 创建分享服务实例。
    ///
    /// # Arguments
    /// * `repo` - 分享链接的数据库仓库
    pub fn new(repo: ShareRepository) -> Self {
        Self { repo }
    }

    /// 创建视频分享链接，返回原始令牌和持久化的分享记录。
    ///
    /// 令牌仅返回一次，之后将以哈希形式存储。过期时间在 1 到 365 天之间，
    /// 未指定时默认为 3 小时。
    ///
    /// # Arguments
    /// * `video_id` - 要分享的视频 ID
    /// * `user_id` - 当前认证用户的 ID（必须是视频上传者或管理员）
    /// * `expires_in_days` - 可选的过期天数（1..=365），默认为 3 小时
    ///
    /// # Returns
    /// * `Ok((token, share))` - 原始令牌字符串和分享记录
    /// * `Err(ServiceError)` - 创建失败时返回错误
    ///
    /// # Security
    /// 授权边界（仅视频上传者或管理员可创建分享）由调用方强制执行 ——
    /// `handlers::shares::create_share_link` 在调用此方法前会检查 `VideoOwnership`。
    /// 请勿从未经认证或非上传者的路径直接调用此方法。
    pub async fn create_share_link(
        &self,
        video_id: i64,
        user_id: i64,
        expires_in_days: Option<i32>,
    ) -> Result<(String, ShareLink), ServiceError> {
        let token = generate_token();
        let base = chrono::Utc::now().naive_utc();
        // Inputs are clamped (1..=365 days, or the fixed 3h default), so the
        // addition can never overflow; the fallback path is dead weight.
        let expires_at = match expires_in_days {
            Some(days) => base + chrono::Duration::days(days.clamp(1, 365) as i64),
            None => base + chrono::Duration::hours(3),
        };
        let share = self
            .repo
            .create_share_link(video_id, user_id, &token, Some(expires_at))
            .await?;
        Ok((token, share))
    }

    /// 获取指定用户创建的所有分享链接列表。
    ///
    /// # Arguments
    /// * `user_id` - 用户 ID
    ///
    /// # Returns
    /// * `Ok(Vec<ShareLink>)` - 用户的所有分享链接
    /// * `Err(ServiceError)` - 查询失败时返回错误
    pub async fn list_my_shares(&self, user_id: i64) -> Result<Vec<ShareLink>, ServiceError> {
        let shares = self.repo.list_shares_for_user(user_id).await?;
        Ok(shares)
    }

    /// 撤销（删除）用户自己的分享链接。
    ///
    /// 仅允许删除属于指定用户的分享链接。
    ///
    /// # Arguments
    /// * `share_id` - 分享链接 ID
    /// * `user_id` - 当前认证用户的 ID
    ///
    /// # Returns
    /// * `Ok(())` - 删除成功
    /// * `Err(ServiceError::NotFound)` - 分享链接不存在或不属于该用户
    pub async fn revoke_my_share(&self, share_id: i64, user_id: i64) -> Result<(), ServiceError> {
        let deleted = self.repo.delete_share_by_owner(share_id, user_id).await?;
        if !deleted {
            return Err(ServiceError::not_found("分享链接不存在"));
        }
        Ok(())
    }

    /// 删除指定视频的分享链接（支持管理员权限）。
    ///
    /// 管理员可删除任何分享链接，普通用户只能删除自己创建的链接。
    ///
    /// # Arguments
    /// * `video_id` - 视频 ID
    /// * `share_id` - 分享链接 ID
    /// * `user_id` - 当前认证用户的 ID
    /// * `is_admin` - 当前用户是否为管理员
    ///
    /// # Returns
    /// * `Ok(())` - 删除成功
    /// * `Err(ServiceError::NotFound)` - 分享链接不存在或无权删除
    pub async fn delete_share_link(
        &self,
        video_id: i64,
        share_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<(), ServiceError> {
        let deleted = self
            .repo
            .delete_share_with_auth(share_id, video_id, user_id, is_admin)
            .await?;
        if !deleted {
            return Err(ServiceError::not_found("分享链接不存在"));
        }
        Ok(())
    }

    /// 通过分享令牌获取关联的视频信息。
    ///
    /// 验证令牌格式并查询其有效性，返回对应的分享记录。
    ///
    /// # Arguments
    /// * `token` - 32 位字母数字分享令牌
    ///
    /// # Returns
    /// * `Ok(ShareLink)` - 有效的分享记录
    /// * `Err(ServiceError::BadRequest)` - 令牌格式无效
    /// * `Err(ServiceError::NotFound)` - 令牌不存在或已过期
    pub async fn get_share_video(&self, token: &str) -> Result<ShareLink, ServiceError> {
        if !is_valid_share_token(token) {
            return Err(ServiceError::bad_request("分享链接格式无效"));
        }
        let token_hash = crate::repositories::share_repo::hash_share_token(token);
        let share = self
            .repo
            .is_valid_token_hash(&token_hash)
            .await?
            .ok_or_else(|| ServiceError::not_found("分享链接不存在"))?;
        Ok(share)
    }
}

/// 生成 32 位随机字母数字令牌。
///
/// 使用 `rand::thread_rng()` 生成密码学安全的随机数。
/// 字符集包含数字（0-9）、小写字母（a-z）和大写字母（A-Z）。
fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            match idx {
                0..10 => (b'0' + idx) as char,
                10..36 => (b'a' + idx - 10) as char,
                36..62 => (b'A' + idx - 36) as char,
                _ => unreachable!(),
            }
        })
        .collect()
}

/// 验证分享令牌格式：必须为 32 个字母数字字符。
///
/// # Arguments
/// * `token` - 要验证的令牌字符串
///
/// # Returns
/// * `true` - 令牌格式有效
/// * `false` - 令牌长度不为 32 或包含非字母数字字符
pub fn is_valid_share_token(token: &str) -> bool {
    token.len() == 32 && token.chars().all(|c| c.is_ascii_alphanumeric())
}
