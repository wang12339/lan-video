use atmos_video_backend::util::pagination::PaginationParams;

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
