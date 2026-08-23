use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::security_scan::SecurityScan;

#[derive(Clone)]
pub struct SecurityScanRepository {
    pool: PgPool,
}

impl SecurityScanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, scan: &SecurityScan) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO security_scans (
                id, release_id, image_digest,
                critical_vulnerabilities, high_vulnerabilities,
                medium_vulnerabilities, low_vulnerabilities,
                passed, report_json, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            scan.id,
            scan.release_id,
            scan.image_digest,
            scan.critical_vulnerabilities,
            scan.high_vulnerabilities,
            scan.medium_vulnerabilities,
            scan.low_vulnerabilities,
            scan.passed,
            scan.report_json,
            scan.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<SecurityScan>, sqlx::Error> {
        let scan = sqlx::query_as!(
            SecurityScan,
            r#"
            SELECT
                id, release_id, image_digest,
                critical_vulnerabilities, high_vulnerabilities,
                medium_vulnerabilities, low_vulnerabilities,
                passed, report_json, created_at
            FROM security_scans
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(scan)
    }

    pub async fn find_latest_by_release_id(
        &self,
        release_id: Uuid,
    ) -> Result<Option<SecurityScan>, sqlx::Error> {
        let scan = sqlx::query_as!(
            SecurityScan,
            r#"
            SELECT
                id, release_id, image_digest,
                critical_vulnerabilities, high_vulnerabilities,
                medium_vulnerabilities, low_vulnerabilities,
                passed, report_json, created_at
            FROM security_scans
            WHERE release_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            release_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(scan)
    }
}
