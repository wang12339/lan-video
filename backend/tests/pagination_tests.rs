use atmos_video_backend::util::pagination::{
    PaginationParams, DEFAULT_PAGE_SIZE, MAX_PAGE, MAX_PAGE_SIZE,
};

#[test]
fn test_pagination_default() {
    let p = PaginationParams::new(None, None);
    assert_eq!(p.page, 1);
    assert_eq!(p.page_size, 20);
    assert_eq!(p.offset(), 0);
}

#[test]
fn test_pagination_clamp_page() {
    let p = PaginationParams::new(Some(-1), None);
    assert_eq!(p.page, 1);

    let p = PaginationParams::new(Some(100000), None);
    assert_eq!(p.page, 10000); // MAX_PAGE
}

#[test]
fn test_pagination_clamp_page_size() {
    let p = PaginationParams::new(None, Some(0));
    assert_eq!(p.page_size, 1);

    let p = PaginationParams::new(None, Some(1000));
    assert_eq!(p.page_size, 100); // MAX_PAGE_SIZE
}

#[test]
fn test_pagination_offset() {
    let p = PaginationParams::new(Some(3), Some(10));
    assert_eq!(p.offset(), 20); // (3-1) * 10
}

#[test]
fn test_pagination_first_page_offset() {
    let p = PaginationParams::new(Some(1), Some(20));
    assert_eq!(p.offset(), 0);
}

#[test]
fn test_pagination_second_page_offset() {
    let p = PaginationParams::new(Some(2), Some(20));
    assert_eq!(p.offset(), 20);
}

#[test]
fn test_pagination_negative_page_clamps_to_one() {
    let p = PaginationParams::new(Some(-100), None);
    assert_eq!(p.page, 1);
    assert_eq!(p.offset(), 0);
}

#[test]
fn test_pagination_negative_page_size_clamps_to_one() {
    let p = PaginationParams::new(None, Some(-50));
    assert_eq!(p.page_size, 1);
}

#[test]
fn test_pagination_zero_page_clamps_to_one() {
    let p = PaginationParams::new(Some(0), None);
    assert_eq!(p.page, 1);
    assert_eq!(p.offset(), 0);
}

#[test]
fn test_pagination_exactly_max_page() {
    let p = PaginationParams::new(Some(MAX_PAGE), Some(1));
    assert_eq!(p.page, MAX_PAGE);
    assert_eq!(p.offset(), (MAX_PAGE - 1) * 1);
}

#[test]
fn test_pagination_exactly_max_page_size() {
    let p = PaginationParams::new(Some(1), Some(MAX_PAGE_SIZE));
    assert_eq!(p.page_size, MAX_PAGE_SIZE);
    assert_eq!(p.offset(), 0);
}

#[test]
fn test_pagination_i64_extreme_values() {
    let p = PaginationParams::new(Some(i64::MAX), Some(i64::MAX));
    assert_eq!(p.page, MAX_PAGE);
    assert_eq!(p.page_size, MAX_PAGE_SIZE);

    let p = PaginationParams::new(Some(i64::MIN), Some(i64::MIN));
    assert_eq!(p.page, 1);
    assert_eq!(p.page_size, 1);
}

#[test]
fn test_pagination_default_constants() {
    assert_eq!(DEFAULT_PAGE_SIZE, 20);
    assert_eq!(MAX_PAGE, 10000);
    assert_eq!(MAX_PAGE_SIZE, 100);
}
