use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::environment::{CreateEnvironmentRequest, Environment, UpdateEnvironmentRequest};

#[derive(Clone)]
pub struct EnvironmentRepository {
    pool: PgPool,
}

impl EnvironmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        id: Uuid,
        project_id: Uuid,
        input: &CreateEnvironmentRequest,
    ) -> Result<Environment, sqlx::Error> {
        sqlx::query_as::<_, Environment>(
            r#"
        INSERT INTO environments (
            id,
            project_id,
            name,
            environment_type,
            ssh_host,
            ssh_port,
            ssh_user,
            remote_app_directory,
            server_architecture,
            ssh_identity_secret_ref,
            registry_credential_secret_ref,
            remote_env_file_path
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            $12
        )
        RETURNING
            id,
            project_id,
            name,
            environment_type,
            ssh_host,
            ssh_port,
            ssh_user,
            remote_app_directory,
            server_architecture,
            ssh_identity_secret_ref,
            registry_credential_secret_ref,
            remote_env_file_path,
            created_at,
            updated_at,
            archived_at
        "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(&input.name)
        .bind(input.environment_type.as_db_str())
        .bind(&input.ssh_host)
        .bind(i32::from(input.ssh_port))
        .bind(&input.ssh_user)
        .bind(&input.remote_app_directory)
        .bind(&input.server_architecture)
        .bind(&input.ssh_identity_secret_ref)
        .bind(&input.registry_credential_secret_ref)
        .bind(&input.remote_env_file_path)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_all_active_by_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Environment>, sqlx::Error> {
        sqlx::query_as::<_, Environment>(
            r#"
        SELECT
            id,
            project_id,
            name,
            environment_type,
            ssh_host,
            ssh_port,
            ssh_user,
            remote_app_directory,
            server_architecture,
            ssh_identity_secret_ref,
            registry_credential_secret_ref,
            remote_env_file_path,
            created_at,
            updated_at,
            archived_at
        FROM environments
        WHERE
            project_id = $1
            AND archived_at IS NULL
        ORDER BY
            CASE environment_type
                WHEN 'STAGING' THEN 1
                WHEN 'PRODUCTION' THEN 2
                ELSE 99
            END,
            created_at
        "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find_active_by_id(
        &self,
        environment_id: Uuid,
    ) -> Result<Option<Environment>, sqlx::Error> {
        sqlx::query_as::<_, Environment>(
            r#"
        SELECT
            e.id,
            e.project_id,
            e.name,
            e.environment_type,
            e.ssh_host,
            e.ssh_port,
            e.ssh_user,
            e.remote_app_directory,
            e.server_architecture,
            e.ssh_identity_secret_ref,
            e.registry_credential_secret_ref,
            e.remote_env_file_path,
            e.created_at,
            e.updated_at,
            e.archived_at
        FROM environments AS e
        INNER JOIN projects AS p
            ON p.id = e.project_id
        WHERE
            e.id = $1
            AND e.archived_at IS NULL
            AND p.archived_at IS NULL
        "#,
        )
        .bind(environment_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        environment_id: Uuid,
        input: &UpdateEnvironmentRequest,
    ) -> Result<Option<Environment>, sqlx::Error> {
        sqlx::query_as::<_, Environment>(
            r#"
        UPDATE environments
        SET
            name = $2,
            environment_type = $3,
            ssh_host = $4,
            ssh_port = $5,
            ssh_user = $6,
            remote_app_directory = $7,
            server_architecture = $8,
            ssh_identity_secret_ref = $9,
            registry_credential_secret_ref = $10,
            remote_env_file_path = $11,
            updated_at = NOW()
        WHERE
            id = $1
            AND archived_at IS NULL
        RETURNING
            id,
            project_id,
            name,
            environment_type,
            ssh_host,
            ssh_port,
            ssh_user,
            remote_app_directory,
            server_architecture,
            ssh_identity_secret_ref,
            registry_credential_secret_ref,
            remote_env_file_path,
            created_at,
            updated_at,
            archived_at
        "#,
        )
        .bind(environment_id)
        .bind(&input.name)
        .bind(input.environment_type.as_db_str())
        .bind(&input.ssh_host)
        .bind(i32::from(input.ssh_port))
        .bind(&input.ssh_user)
        .bind(&input.remote_app_directory)
        .bind(&input.server_architecture)
        .bind(&input.ssh_identity_secret_ref)
        .bind(&input.registry_credential_secret_ref)
        .bind(&input.remote_env_file_path)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn archive(&self, environment_id: Uuid) -> Result<Option<Environment>, sqlx::Error> {
        sqlx::query_as::<_, Environment>(
            r#"
        UPDATE environments
        SET
            archived_at = NOW(),
            updated_at = NOW()
        WHERE
            id = $1
            AND archived_at IS NULL
        RETURNING
            id,
            project_id,
            name,
            environment_type,
            ssh_host,
            ssh_port,
            ssh_user,
            remote_app_directory,
            server_architecture,
            ssh_identity_secret_ref,
            registry_credential_secret_ref,
            remote_env_file_path,
            created_at,
            updated_at,
            archived_at
        "#,
        )
        .bind(environment_id)
        .fetch_optional(&self.pool)
        .await
    }
}
