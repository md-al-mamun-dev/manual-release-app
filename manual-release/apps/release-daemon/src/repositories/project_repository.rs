use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::project::{CreateProjectRequest, Project, UpdateProjectRequest};

#[derive(Clone)]
pub struct ProjectRepository {
    pool: PgPool,
}

impl ProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        id: Uuid,
        input: &CreateProjectRequest,
    ) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (
                id,
                name,
                repository_path,
                repository_url,
                default_branch
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5
            )
            RETURNING
                id,
                name,
                repository_path,
                repository_url,
                default_branch,
                created_at,
                updated_at,
                archived_at
            "#,
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.repository_path)
        .bind(&input.repository_url)
        .bind(&input.default_branch)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_all_active(&self) -> Result<Vec<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            SELECT
                id,
                name,
                repository_path,
                repository_url,
                default_branch,
                created_at,
                updated_at,
                archived_at
            FROM projects
            WHERE archived_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_active_by_id(
        &self,
        project_id: Uuid,
    ) -> Result<Option<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            SELECT
                id,
                name,
                repository_path,
                repository_url,
                default_branch,
                created_at,
                updated_at,
                archived_at
            FROM projects
            WHERE
                id = $1
                AND archived_at IS NULL
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        input: &UpdateProjectRequest,
    ) -> Result<Option<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            UPDATE projects
            SET
                name = COALESCE($2, name),
                repository_path = COALESCE(
                    $3,
                    repository_path
                ),
                repository_url = COALESCE(
                    $4,
                    repository_url
                ),
                default_branch = COALESCE(
                    $5,
                    default_branch
                ),
                updated_at = NOW()
            WHERE
                id = $1
                AND archived_at IS NULL
            RETURNING
                id,
                name,
                repository_path,
                repository_url,
                default_branch,
                created_at,
                updated_at,
                archived_at
            "#,
        )
        .bind(project_id)
        .bind(&input.name)
        .bind(&input.repository_path)
        .bind(&input.repository_url)
        .bind(&input.default_branch)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn archive(&self, project_id: Uuid) -> Result<Option<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>(
            r#"
            UPDATE projects
            SET
                archived_at = NOW(),
                updated_at = NOW()
            WHERE
                id = $1
                AND archived_at IS NULL
            RETURNING
                id,
                name,
                repository_path,
                repository_url,
                default_branch,
                created_at,
                updated_at,
                archived_at
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
    }
}
