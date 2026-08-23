use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::release_image::ReleaseImage;

pub struct ReleaseImageRepository {
    pool: PgPool,
}

impl ReleaseImageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_release_image(
        &self,
        release_image: &ReleaseImage,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO release_images (id, release_id, job_id, git_sha, image_tag, image_digest, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            release_image.id,
            release_image.release_id,
            release_image.job_id,
            release_image.git_sha,
            release_image.image_tag,
            release_image.image_digest,
            release_image.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_by_release_id(
        &self,
        release_id: Uuid,
    ) -> Result<Option<ReleaseImage>, sqlx::Error> {
        let row = sqlx::query_as!(
            ReleaseImage,
            r#"
            SELECT id, release_id, job_id, git_sha, image_tag, image_digest, created_at
            FROM release_images
            WHERE release_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            release_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }
}
