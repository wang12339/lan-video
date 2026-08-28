pub const MAX_PAGE: i64 = 10000;
pub const DEFAULT_PAGE_SIZE: i64 = 20;
pub const MAX_PAGE_SIZE: i64 = 100;

pub struct PaginationParams {
    pub page: i64,
    pub page_size: i64,
}

impl PaginationParams {
    pub fn new(page: Option<i64>, page_size: Option<i64>) -> Self {
        Self {
            page: page.unwrap_or(1).clamp(1, MAX_PAGE),
            page_size: page_size
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .clamp(1, MAX_PAGE_SIZE),
        }
    }

    #[inline]
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}
