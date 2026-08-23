use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::release_image::ReleaseImage;
use chrono::Utc;

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
            SELECT id, release_id, job_id, git_sha, image_tag, image_digest, created_at, registry, repository, remote_digest, publication_status, updated_at
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

    pub async fn update_publishing_details(
        &self,
        release_id: Uuid,
        registry: &str,
        repository: &str,
        remote_digest: &str,
        publication_status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE release_images
            SET registry = $1, repository = $2, remote_digest = $3, publication_status = $4, updated_at = $5
            WHERE release_id = $6
            "#,
            registry,
            repository,
            remote_digest,
            publication_status,
            Utc::now(),
            release_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
