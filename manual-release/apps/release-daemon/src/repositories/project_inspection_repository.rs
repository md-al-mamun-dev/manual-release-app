use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::project_inspection::ProjectInspection;

#[derive(Clone)]
pub struct ProjectInspectionRepository {
    pool: PgPool,
}

impl ProjectInspectionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn start(&self, project_id: Uuid) -> Result<ProjectInspection, sqlx::Error> {
        sqlx::query_as::<_, ProjectInspection>(
            r#"
            INSERT INTO project_inspections (
                id,
                project_id,
                status
            )
            VALUES (
                $1,
                $2,
                'RUNNING'
            )
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn succeed(
        &self,
        inspection_id: Uuid,
        repository_path: &str,
        git_commit: &str,
        git_branch: Option<&str>,
        git_dirty: bool,
        report: Value,
    ) -> Result<ProjectInspection, sqlx::Error> {
        sqlx::query_as::<_, ProjectInspection>(
            r#"
            UPDATE project_inspections
            SET
                status = 'SUCCEEDED',
                canonical_repository_path = $2,
                git_commit = $3,
                git_branch = $4,
                git_dirty = $5,
                report = $6,
                finished_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(inspection_id)
        .bind(repository_path)
        .bind(git_commit)
        .bind(git_branch)
        .bind(git_dirty)
        .bind(report)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn fail(
        &self,
        inspection_id: Uuid,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE project_inspections
            SET
                status = 'FAILED',
                error_code = $2,
                error_message = $3,
                finished_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(inspection_id)
        .bind(error_code)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn latest(&self, project_id: Uuid) -> Result<Option<ProjectInspection>, sqlx::Error> {
        sqlx::query_as::<_, ProjectInspection>(
            r#"
            SELECT *
            FROM project_inspections
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
    }
}
