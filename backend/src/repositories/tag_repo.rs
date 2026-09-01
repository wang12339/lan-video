use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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

    pub async fn create_tag(
        &self,
        tenant_id: i64,
        name: &str,
        color: Option<&str>,
    ) -> Result<Tag, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            INSERT INTO tags (tenant_id, name, color)
            VALUES ($1, $2, $3)
            RETURNING id, name, color, usage_count
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(color)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_tag_by_name(
        &self,
        tenant_id: i64,
        name: &str,
    ) -> Result<Option<Tag>, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE tenant_id = $1 AND name = $2
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_tag_by_id(
        &self,
        tenant_id: i64,
        id: i32,
    ) -> Result<Option<Tag>, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE tenant_id = $1 AND id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_tags(
        &self,
        tenant_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Tag>, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE tenant_id = $1
            ORDER BY usage_count DESC, name ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_tag(
        &self,
        tenant_id: i64,
        id: i32,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<Tag, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            UPDATE tags
            SET 
                name = COALESCE($2, name),
                color = COALESCE($3, color)
            WHERE id = $1 AND tenant_id = $4
            RETURNING id, name, color, usage_count
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete_tag(&self, tenant_id: i64, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tags WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
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
        tenant_id: i64,
        video_id: i64,
        tag_ids: &[i32],
    ) -> Result<(), sqlx::Error> {
        if tag_ids.is_empty() {
            return Ok(());
        }
        let mut builder = sqlx::QueryBuilder::new(
            "WITH inserted AS (INSERT INTO video_tags (tenant_id, video_id, tag_id) VALUES ",
        );
        builder.push_values(tag_ids, |mut b, &tag_id| {
            b.push_bind(tenant_id).push_bind(video_id).push_bind(tag_id);
        });
        builder.push(
            " ON CONFLICT DO NOTHING RETURNING tag_id) \
             UPDATE tags SET usage_count = usage_count + 1 \
             FROM inserted WHERE tags.id = inserted.tag_id AND tags.tenant_id = ",
        );
        builder.push_bind(tenant_id);
        builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Remove a single video-tag association, decrementing the tag's
    /// `usage_count` in the same transaction as the delete.
    pub async fn remove_tag_from_video(
        &self,
        tenant_id: i64,
        video_id: i64,
        tag_id: i32,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            "DELETE FROM video_tags WHERE tenant_id = $1 AND video_id = $2 AND tag_id = $3",
        )
        .bind(tenant_id)
        .bind(video_id)
        .bind(tag_id)
        .execute(&mut *tx)
        .await?;

        // Only decrement if a row was actually deleted
        if result.rows_affected() > 0 {
            sqlx::query(
                "UPDATE tags SET usage_count = GREATEST(usage_count - 1, 0) WHERE id = $1 AND tenant_id = $2",
            )
            .bind(tag_id)
            .bind(tenant_id)
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
        tenant_id: i64,
        video_id: i64,
        tag_ids: &[i32],
    ) -> Result<(), sqlx::Error> {
        if tag_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "WITH deleted AS (DELETE FROM video_tags WHERE tenant_id = $1 AND video_id = $2 AND tag_id = ANY($3) RETURNING tag_id) \
             UPDATE tags SET usage_count = GREATEST(usage_count - 1, 0) \
             FROM deleted WHERE tags.id = deleted.tag_id AND tags.tenant_id = $1",
        )
        .bind(tenant_id)
        .bind(video_id)
        .bind(tag_ids)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_video_tags(
        &self,
        tenant_id: i64,
        video_id: i64,
    ) -> Result<Vec<Tag>, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT t.id, t.name, t.color, t.usage_count
            FROM tags t
            INNER JOIN video_tags vt ON t.id = vt.tag_id
            WHERE vt.video_id = $1 AND vt.tenant_id = $2 AND t.tenant_id = $2
            ORDER BY t.name ASC
            "#,
        )
        .bind(video_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_popular_tags(
        &self,
        tenant_id: i64,
        limit: i64,
    ) -> Result<Vec<Tag>, sqlx::Error> {
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE tenant_id = $1 AND usage_count > 0
            ORDER BY usage_count DESC
            LIMIT $2
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_tags_by_ids(
        &self,
        tenant_id: i64,
        ids: &[i32],
    ) -> Result<Vec<Tag>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_as::<_, Tag>(
            r#"
            SELECT id, name, color, usage_count
            FROM tags
            WHERE tenant_id = $1 AND id = ANY($2)
            "#,
        )
        .bind(tenant_id)
        .bind(ids)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn count_tags(&self, tenant_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tags WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
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
