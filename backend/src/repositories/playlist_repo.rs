use sqlx::{PgPool, Row};

#[derive(Debug, sqlx::FromRow)]
pub struct PlaylistRow {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub cover_url: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PlaylistItemRow {
    pub id: i64,
    pub playlist_id: i64,
    pub video_id: i64,
    pub position: i32,
    pub added_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct PlaylistRepository {
    pool: PgPool,
}

impl PlaylistRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_playlist(
        &self,
        user_id: i64,
        name: &str,
        description: Option<&str>,
        is_public: bool,
    ) -> Result<PlaylistRow, sqlx::Error> {
        sqlx::query_as::<_, PlaylistRow>(
            r#"INSERT INTO playlists (user_id, name, description, is_public)
               VALUES ($1, $2, $3, $4) RETURNING *"#,
        )
        .bind(user_id)
        .bind(name)
        .bind(description)
        .bind(is_public)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_playlist(&self, playlist_id: i64) -> Result<Option<PlaylistRow>, sqlx::Error> {
        sqlx::query_as::<_, PlaylistRow>("SELECT * FROM playlists WHERE id = $1")
            .bind(playlist_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn update_playlist(
        &self,
        playlist_id: i64,
        name: Option<&str>,
        description: Option<&str>,
        is_public: Option<bool>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE playlists SET
               name = COALESCE($2, name),
               description = COALESCE($3, description),
               is_public = COALESCE($4, is_public),
               updated_at = CURRENT_TIMESTAMP
               WHERE id = $1"#,
        )
        .bind(playlist_id)
        .bind(name)
        .bind(description)
        .bind(is_public)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_playlist(&self, playlist_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM playlists WHERE id = $1")
            .bind(playlist_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn add_video(
        &self,
        playlist_id: i64,
        video_id: i64,
    ) -> Result<Option<PlaylistItemRow>, sqlx::Error> {
        // The MAX(position)+1 subquery alone is racy: two concurrent inserts
        // can compute the same next position (UNIQUE(playlist_id, video_id)
        // does not cover position). A per-playlist advisory lock serializes
        // inserts, and the playlist's updated_at is touched in the same
        // transaction so a failed insert cannot leave a stale timestamp.
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('playlist_items:' || $1::text))")
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query_as::<_, PlaylistItemRow>(
            r#"INSERT INTO playlist_items (playlist_id, video_id, position)
               VALUES ($1, $2, (SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_items WHERE playlist_id = $1))
               ON CONFLICT (playlist_id, video_id) DO NOTHING
               RETURNING *"#,
        )
        .bind(playlist_id)
        .bind(video_id)
        .fetch_optional(&mut *tx)
        .await?;
        sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(row)
    }

    pub async fn remove_video(&self, playlist_id: i64, video_id: i64) -> Result<bool, sqlx::Error> {
        // Serialize with add_video (same advisory lock) so the renumbering
        // below can never race a MAX(position)+1 insert.
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('playlist_items:' || $1::text))")
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;
        let result =
            sqlx::query("DELETE FROM playlist_items WHERE playlist_id = $1 AND video_id = $2")
                .bind(playlist_id)
                .bind(video_id)
                .execute(&mut *tx)
                .await?;
        if result.rows_affected() > 0 {
            // Compress the remaining items so positions stay dense (no holes),
            // matching the ORDER BY position, added_at used when listing.
            sqlx::query(
                r#"UPDATE playlist_items
                   SET position = sub.rn
                   FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY position, added_at) - 1 AS rn
                         FROM playlist_items WHERE playlist_id = $1) AS sub
                   WHERE playlist_items.id = sub.id AND playlist_items.position <> sub.rn"#,
            )
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_user_playlists_with_counts(
        &self,
        user_id: i64,
    ) -> Result<Vec<(PlaylistRow, i64)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT p.*, COALESCE(i.item_count, 0) as item_count
               FROM playlists p
               LEFT JOIN (SELECT playlist_id, COUNT(*) as item_count FROM playlist_items GROUP BY playlist_id) i ON i.playlist_id = p.id
               WHERE p.user_id = $1
               ORDER BY p.updated_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let playlist = PlaylistRow {
                    id: r.get("id"),
                    user_id: r.get("user_id"),
                    name: r.get("name"),
                    description: r.get("description"),
                    is_public: r.get("is_public"),
                    cover_url: r.get("cover_url"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                };
                let count: i64 = r.get("item_count");
                (playlist, count)
            })
            .collect())
    }

    pub async fn count_playlist_items(&self, playlist_id: i64) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM playlist_items WHERE playlist_id = $1")
                .bind(playlist_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    pub async fn list_playlist_videos(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<PlaylistVideoRow>, sqlx::Error> {
        sqlx::query_as::<_, PlaylistVideoRow>(
            r#"SELECT v.id, v.title, v.description, v.source_type, v.cover_url, v.stream_url,
                      v.category, v.views, v.duration
               FROM playlist_items i
               JOIN videos v ON i.video_id = v.id
               WHERE i.playlist_id = $1
               ORDER BY i.position ASC, i.added_at ASC"#,
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Reorder all videos in a playlist by assigning sequential positions
    /// (0, 1, 2, …) in the caller-supplied order.
    ///
    /// Uses the same per-playlist advisory lock as `add_video` / `remove_video`
    /// so concurrent mutations are serialized.
    pub async fn reorder_videos(
        &self,
        playlist_id: i64,
        video_ids: &[i64],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('playlist_items:' || $1::text))")
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;

        for (pos, video_id) in video_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE playlist_items SET position = $3 WHERE playlist_id = $1 AND video_id = $2",
            )
            .bind(playlist_id)
            .bind(video_id)
            .bind(pos as i32)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(playlist_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct PlaylistVideoRow {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub source_type: String,
    pub cover_url: Option<String>,
    pub stream_url: String,
    pub category: String,
    pub views: i64,
    pub duration: i64,
}
