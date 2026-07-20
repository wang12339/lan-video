use serde::{Deserialize, Serialize};
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

        Ok(Tag {
            id: row.get("id"),
            name: row.get("name"),
            color: row.get("color"),
            usage_count: row.get("usage_count"),
        })
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

        Ok(row.map(|r| Tag {
            id: r.get("id"),
            name: r.get("name"),
            color: r.get("color"),
            usage_count: r.get("usage_count"),
        }))
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

        Ok(row.map(|r| Tag {
            id: r.get("id"),
            name: r.get("name"),
            color: r.get("color"),
            usage_count: r.get("usage_count"),
        }))
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

        Ok(rows
            .into_iter()
            .map(|r| Tag {
                id: r.get("id"),
                name: r.get("name"),
                color: r.get("color"),
                usage_count: r.get("usage_count"),
            })
            .collect())
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

        Ok(Tag {
            id: row.get("id"),
            name: row.get("name"),
            color: row.get("color"),
            usage_count: row.get("usage_count"),
        })
    }

    pub async fn delete_tag(&self, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM tags WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add_tag_to_video(&self, video_id: i64, tag_id: i32) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO video_tags (video_id, tag_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(video_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await?;

        sqlx::query("UPDATE tags SET usage_count = usage_count + 1 WHERE id = $1")
            .bind(tag_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn remove_tag_from_video(
        &self,
        video_id: i64,
        tag_id: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM video_tags WHERE video_id = $1 AND tag_id = $2")
            .bind(video_id)
            .bind(tag_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE tags SET usage_count = GREATEST(usage_count - 1, 0) WHERE id = $1")
            .bind(tag_id)
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

        Ok(rows
            .into_iter()
            .map(|r| Tag {
                id: r.get("id"),
                name: r.get("name"),
                color: r.get("color"),
                usage_count: r.get("usage_count"),
            })
            .collect())
    }

    pub async fn search_videos_by_tags(
        &self,
        tag_ids: &[i32],
        page: i64,
        size: i64,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT video_id
            FROM video_tags
            WHERE tag_id = ANY($1)
            GROUP BY video_id
            HAVING COUNT(DISTINCT tag_id) = $2
            ORDER BY video_id
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(tag_ids)
        .bind(tag_ids.len() as i64)
        .bind(size)
        .bind(page * size)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| r.get::<i64, _>("video_id"))
            .collect())
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

        Ok(rows
            .into_iter()
            .map(|r| Tag {
                id: r.get("id"),
                name: r.get("name"),
                color: r.get("color"),
                usage_count: r.get("usage_count"),
            })
            .collect())
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

        Ok(rows
            .into_iter()
            .map(|r| Tag {
                id: r.get("id"),
                name: r.get("name"),
                color: r.get("color"),
                usage_count: r.get("usage_count"),
            })
            .collect())
    }

    pub async fn count_tags(&self) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM tags")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
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
