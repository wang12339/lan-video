use sqlx::error::ErrorKind;

/// 检查是否为唯一约束冲突
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.kind() == ErrorKind::UniqueViolation)
}

/// 检查是否为外键约束冲突
pub fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.kind() == ErrorKind::ForeignKeyViolation)
}

/// 获取约束名称
pub fn get_constraint_name(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    }
}
