/// 最大页码
pub const MAX_PAGE: i64 = 10000;
/// 默认每页数量
pub const DEFAULT_PAGE_SIZE: i64 = 20;
/// 最大每页数量
pub const MAX_PAGE_SIZE: i64 = 100;

/// 分页参数
pub struct PaginationParams {
    pub page: i64,
    pub page_size: i64,
}

impl PaginationParams {
    pub fn new(page: Option<i64>, page_size: Option<i64>) -> Self {
        Self {
            page: page.unwrap_or(1).max(1).min(MAX_PAGE),
            page_size: page_size
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .max(1)
                .min(MAX_PAGE_SIZE),
        }
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}