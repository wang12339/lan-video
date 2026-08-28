use sqlx::error::ErrorKind;

#[inline]
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.kind() == ErrorKind::UniqueViolation)
}

#[inline]
pub fn is_foreign_key_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err) if db_err.kind() == ErrorKind::ForeignKeyViolation)
}

#[inline]
pub fn get_constraint_name(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    }
}
