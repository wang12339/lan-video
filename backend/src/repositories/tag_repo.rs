use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
}

#[derive(Debug, Clone)]
pub struct TagRepository {
    pool: PgPool,
}

impl TagRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_tag(&self, name: &str, color: Option<&str>) -> Result<Tag, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO tags (name, color)
            VALUES ($1, $2)
            RETURNING id, name, color, usage_count
            "#,
        )
        .bind(name)
        .bind(color)
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_tag(row))
    }

    pub async fn find_tag_by_name(&self, name: &str) -> Result<Option<Tag>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_tag))
    }

    pub async fn find_tag_by_id(&self, id: i32) -> Result<Option<Tag>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_tag))
    }

    pub async fn list_tags(&self, limit: i64, offset: i64) -> Result<Vec<Tag>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            ORDER BY usage_count DESC, name ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_tag).collect())
    }

    pub async fn update_tag(
        &self,
        id: i32,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<Tag, sqlx::Error> {
        let row = sqlx::query(
            r#"
            UPDATE tags
            SET 
                name = COALESCE($2, name),
                color = COALESCE($3, color)
            WHERE id = $1
            RETURNING id, name, color, usage_count
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_tag(row))
    }

    pub async fn delete_tag(&self, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tags WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Batch insert video-tag associations in a single round-trip, incrementing
    /// each tag's `usage_count` by exactly one per association actually inserted.
    ///
    /// The `INSERT ... ON CONFLICT DO NOTHING ... RETURNING` + `UPDATE ... FROM`
    /// data-modifying CTE attributes each increment to the tag that owns the
    /// inserted row, instead of applying the batch total to every tag in the list.
    pub async fn add_tags_to_video_batch(
        &self,
        video_id: i64,
        tag_ids: &[i32],
    ) -> Result<(), sqlx::Error> {
        if tag_ids.is_empty() {
            return Ok(());
        }
        let mut builder = sqlx::QueryBuilder::new(
            "WITH inserted AS (INSERT INTO video_tags (video_id, tag_id) VALUES ",
        );
        builder.push_values(tag_ids, |mut b, &tag_id| {
            b.push_bind(video_id).push_bind(tag_id);
        });
        builder.push(
            " ON CONFLICT DO NOTHING RETURNING tag_id) \
             UPDATE tags SET usage_count = usage_count + 1 \
             FROM inserted WHERE tags.id = inserted.tag_id",
        );
        builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Remove a single video-tag association, decrementing the tag's
    /// `usage_count` in the same transaction as the delete.
    pub async fn remove_tag_from_video(
        &self,
        video_id: i64,
        tag_id: i32,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query("DELETE FROM video_tags WHERE video_id = $1 AND tag_id = $2")
            .bind(video_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;

        // Only decrement if a row was actually deleted
        if result.rows_affected() > 0 {
            sqlx::query("UPDATE tags SET usage_count = GREATEST(usage_count - 1, 0) WHERE id = $1")
                .bind(tag_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Batch delete video-tag associations in a single round-trip, decrementing
    /// each tag's `usage_count` by one per association actually removed.
    ///
    /// The `DELETE ... RETURNING` + `UPDATE ... FROM` CTE decrements each tag by
    /// exactly the number of its own rows that were deleted (the `video_tags`
    /// primary key guarantees at most one row per video/tag pair).
    pub async fn remove_tags_from_video_batch(
        &self,
        video_id: i64,
        tag_ids: &[i32],
    ) -> Result<(), sqlx::Error> {
        if tag_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "WITH deleted AS (DELETE FROM video_tags WHERE video_id = $1 AND tag_id = ANY($2) RETURNING tag_id) \
             UPDATE tags SET usage_count = GREATEST(usage_count - 1, 0) \
             FROM deleted WHERE tags.id = deleted.tag_id",
        )
        .bind(video_id)
        .bind(tag_ids)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_video_tags(&self, video_id: i64) -> Result<Vec<Tag>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.name, t.color, t.usage_count
            FROM tags t
            INNER JOIN video_tags vt ON t.id = vt.tag_id
            WHERE vt.video_id = $1
            ORDER BY t.name ASC
            "#,
        )
        .bind(video_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_tag).collect())
    }

    pub async fn get_popular_tags(&self, limit: i64) -> Result<Vec<Tag>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE usage_count > 0
            ORDER BY usage_count DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_tag).collect())
    }

    pub async fn find_tags_by_ids(&self, ids: &[i32]) -> Result<Vec<Tag>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE id = ANY($1)
            "#,
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_tag).collect())
    }

    pub async fn count_tags(&self) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM tags")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }
}

/// Map a `tags` table row into a [`Tag`].
fn row_to_tag(row: PgRow) -> Tag {
    Tag {
        id: row.get("id"),
        name: row.get("name"),
        color: row.get("color"),
        usage_count: row.get("usage_count"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_creation() {
        let tag = Tag {
            id: 1,
            name: "test".to_string(),
            color: Some("#FF5733".to_string()),
            usage_count: 0,
        };

        assert_eq!(tag.name, "test");
        assert_eq!(tag.color, Some("#FF5733".to_string()));
        assert_eq!(tag.usage_count, 0);
    }
}
