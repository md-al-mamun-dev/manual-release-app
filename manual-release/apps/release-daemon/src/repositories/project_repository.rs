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
        if input.name.is_none()
            && input.repository_path.is_none()
            && input.repository_url.is_none()
            && input.default_branch.is_none()
        {
            return self.find_active_by_id(project_id).await;
        }

        let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new("UPDATE projects SET ");
        let mut separated = query.separated(", ");

        if let Some(name) = &input.name {
            separated.push("name = ");
            separated.push_bind_unseparated(name);
        }

        if let Some(repository_path) = &input.repository_path {
            separated.push("repository_path = ");
            separated.push_bind_unseparated(repository_path);
        }

        if let Some(repository_url) = &input.repository_url {
            separated.push("repository_url = ");
            separated.push_bind_unseparated(repository_url);
        }

        if let Some(default_branch) = &input.default_branch {
            separated.push("default_branch = ");
            separated.push_bind_unseparated(default_branch);
        }

        separated.push("updated_at = NOW()");

        query.push(" WHERE id = ");
        query.push_bind(project_id);
        query.push(" AND archived_at IS NULL RETURNING id, name, repository_path, repository_url, default_branch, created_at, updated_at, archived_at");

        query
            .build_query_as::<Project>()
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
