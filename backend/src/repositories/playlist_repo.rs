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

    pub async fn list_user_playlists(&self, user_id: i64) -> Result<Vec<PlaylistRow>, sqlx::Error> {
        sqlx::query_as::<_, PlaylistRow>(
            "SELECT * FROM playlists WHERE user_id = $1 ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_public_playlists(&self) -> Result<Vec<PlaylistRow>, sqlx::Error> {
        sqlx::query_as::<_, PlaylistRow>(
            "SELECT * FROM playlists WHERE is_public = true ORDER BY updated_at DESC LIMIT 50",
        )
        .fetch_all(&self.pool)
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
    ) -> Result<PlaylistItemRow, sqlx::Error> {
        // Get max position
        let max_pos: Option<(i32,)> =
            sqlx::query_as("SELECT MAX(position) FROM playlist_items WHERE playlist_id = $1")
                .bind(playlist_id)
                .fetch_optional(&self.pool)
                .await?;
        let next_pos = max_pos.map(|(p,)| p).unwrap_or(-1) + 1;

        sqlx::query_as::<_, PlaylistItemRow>(
            r#"INSERT INTO playlist_items (playlist_id, video_id, position)
               VALUES ($1, $2, $3) ON CONFLICT (playlist_id, video_id) DO NOTHING
               RETURNING *"#,
        )
        .bind(playlist_id)
        .bind(video_id)
        .bind(next_pos)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn remove_video(&self, playlist_id: i64, video_id: i64) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM playlist_items WHERE playlist_id = $1 AND video_id = $2")
                .bind(playlist_id)
                .bind(video_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_playlist_items(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<PlaylistItemRow>, sqlx::Error> {
        sqlx::query_as::<_, PlaylistItemRow>(
            "SELECT * FROM playlist_items WHERE playlist_id = $1 ORDER BY position",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await
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
}
