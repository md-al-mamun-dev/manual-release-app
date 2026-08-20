use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::release::{Release, ReleaseStateTransition, ReleaseStatus};

#[derive(Clone)]
pub struct ReleaseRepository {
    pool: PgPool,
}

pub struct CreateReleaseInput<'a> {
    pub id: Uuid,

    pub project_id: Uuid,

    pub source_inspection_id: Uuid,

    pub version: &'a str,

    pub git_commit: &'a str,

    pub git_branch: Option<&'a str>,

    pub source_dirty: bool,

    pub requested_by: Option<&'a str>,

    pub actor: &'a str,
}

impl ReleaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_with_initial_transition(
        &self,
        input: CreateReleaseInput<'_>,
    ) -> Result<Release, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        let release = sqlx::query_as::<_, Release>(
            r#"
                INSERT INTO releases (
                    id,
                    project_id,
                    source_inspection_id,
                    version,
                    git_commit,
                    git_branch,
                    source_dirty,
                    status,
                    requested_by
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
                    $9
                )
                RETURNING
                    id,
                    project_id,
                    source_inspection_id,
                    version,
                    git_commit,
                    git_branch,
                    source_dirty,
                    status,
                    requested_by,
                    created_at,
                    updated_at
                "#,
        )
        .bind(input.id)
        .bind(input.project_id)
        .bind(input.source_inspection_id)
        .bind(input.version)
        .bind(input.git_commit)
        .bind(input.git_branch)
        .bind(input.source_dirty)
        .bind(ReleaseStatus::Created.as_db_str())
        .bind(input.requested_by)
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO release_state_transitions (
                release_id,
                from_status,
                to_status,
                actor,
                reason,
                metadata
            )
            VALUES (
                $1,
                NULL,
                $2,
                $3,
                $4,
                $5
            )
            "#,
        )
        .bind(release.id)
        .bind(ReleaseStatus::Created.as_db_str())
        .bind(input.actor)
        .bind("release created")
        .bind(json!({
            "sourceInspectionId":
                input.source_inspection_id
        }))
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        Ok(release)
    }

    pub async fn transition_status(
        &self,
        release_id: Uuid,
        from_status: &str,
        to_status: &str,
        actor: &str,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query!(
            r#"
            UPDATE releases
            SET status = $1, updated_at = NOW()
            WHERE id = $2 AND status = $3
            "#,
            to_status,
            release_id,
            from_status
        )
        .execute(&mut *transaction)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO release_state_transitions (
                release_id,
                from_status,
                to_status,
                actor,
                reason,
                metadata
            )
            VALUES ($1, $2, $3, $4, $5, '{}'::jsonb)
            "#,
            release_id,
            from_status,
            to_status,
            actor,
            reason
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(())
    }
    pub async fn find_by_id(&self, release_id: Uuid) -> Result<Option<Release>, sqlx::Error> {
        sqlx::query_as::<_, Release>(
            r#"
            SELECT
                id,
                project_id,
                source_inspection_id,
                version,
                git_commit,
                git_branch,
                source_dirty,
                status,
                requested_by,
                created_at,
                updated_at
            FROM releases
            WHERE id = $1
            "#,
        )
        .bind(release_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_all_by_project(&self, project_id: Uuid) -> Result<Vec<Release>, sqlx::Error> {
        sqlx::query_as::<_, Release>(
            r#"
            SELECT
                id,
                project_id,
                source_inspection_id,
                version,
                git_commit,
                git_branch,
                source_dirty,
                status,
                requested_by,
                created_at,
                updated_at
            FROM releases
            WHERE project_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn transitions(
        &self,
        release_id: Uuid,
    ) -> Result<Vec<ReleaseStateTransition>, sqlx::Error> {
        sqlx::query_as::<_, ReleaseStateTransition>(
            r#"
            SELECT
                id,
                release_id,
                from_status,
                to_status,
                actor,
                reason,
                metadata,
                created_at
            FROM release_state_transitions
            WHERE release_id = $1
            ORDER BY id ASC
            "#,
        )
        .bind(release_id)
        .fetch_all(&self.pool)
        .await
    }
}
