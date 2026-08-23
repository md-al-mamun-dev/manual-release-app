use chrono::Utc;
use sqlx::PgPool;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct StaleRunnerReaper {
    pool: PgPool,
}

impl StaleRunnerReaper {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run_reaper_cycle(&self) {
        info!("Running stale runner reaper cycle...");

        let output = match tokio::process::Command::new("docker")
            .args([
                "ps",
                "-aq",
                "--filter",
                "label=cicd.managed=true",
                "--format",
                "{{.ID}}||{{.Label \"cicd.job_id\"}}||{{.Label \"cicd.release_id\"}}||{{.Label \"cicd.runner_id\"}}",
            ])
            .output()
            .await
        {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to run docker ps for reaper: {}", e);
                return;
            }
        };

        if !output.status.success() {
            error!(
                "Docker ps for reaper failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let containers = String::from_utf8_lossy(&output.stdout);
        let max_lifetime = chrono::Duration::hours(2);
        let now = Utc::now();

        for line in containers.lines() {
            let parts: Vec<&str> = line.split("||").collect();
            if parts.len() != 4 {
                warn!("Reaper encountered unparseable docker ps output: {}", line);
                continue;
            }

            let container_id = parts[0];
            let job_id_str = parts[1];
            let release_id_str = parts[2];
            // let runner_id_str = parts[3];

            if job_id_str.is_empty() {
                warn!(
                    "Found managed runner {} without job_id. Removing.",
                    container_id
                );
                self.force_remove_container(container_id).await;
                continue;
            }

            let job_id = match Uuid::parse_str(job_id_str) {
                Ok(id) => id,
                Err(_) => {
                    warn!(
                        "Runner {} has invalid job_id {}. Removing.",
                        container_id, job_id_str
                    );
                    self.force_remove_container(container_id).await;
                    continue;
                }
            };

            // Query job state
            let job_row = match sqlx::query!(
                "SELECT status, started_at, queued_at FROM jobs WHERE id = $1",
                job_id
            )
            .fetch_optional(&self.pool)
            .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    warn!(
                        "Job {} not found in database for runner {}. Removing.",
                        job_id, container_id
                    );
                    self.force_remove_container(container_id).await;
                    continue;
                }
                Err(e) => {
                    error!(
                        "Database error checking job {} for runner {}: {}",
                        job_id, container_id, e
                    );
                    continue;
                }
            };

            let is_terminal = matches!(
                job_row.status.as_str(),
                "SUCCEEDED" | "FAILED" | "CANCELLED"
            );

            if is_terminal {
                info!(
                    "Runner {} belongs to terminal job {} (status: {}). Removing.",
                    container_id, job_id, job_row.status
                );
                self.force_remove_container(container_id).await;
                continue;
            }

            // Job is RUNNING or QUEUED. Check lifetime.
            let start_time = job_row.started_at.unwrap_or(job_row.queued_at);
            if now.signed_duration_since(start_time) > max_lifetime {
                warn!(
                    "Runner {} for job {} has exceeded max lifetime. Canceling job and removing runner.",
                    container_id, job_id
                );

                // Fail the job
                let _ = crate::domain::job::fail_job(
                    &self.pool,
                    job_id,
                    "TIMEOUT",
                    "Runner exceeded maximum lifetime of 2 hours",
                )
                .await;

                if let Ok(release_id) = Uuid::parse_str(release_id_str) {
                    let _ = sqlx::query!(
                        "UPDATE releases SET status = 'FAILED' WHERE id = $1",
                        release_id
                    )
                    .execute(&self.pool)
                    .await;
                }

                self.force_remove_container(container_id).await;
            } else {
                // Active and within lifetime. Leave it alone.
            }
        }

        info!("Stale runner reaper cycle completed.");
    }

    async fn force_remove_container(&self, container_id: &str) {
        // Find runner name/volume name if available, or just rm -f the container
        info!("Force removing orphaned container {}", container_id);

        let mut inspect_cmd = tokio::process::Command::new("docker");
        inspect_cmd.args(["inspect", "--format", "{{.Name}}", container_id]);

        if let Ok(output) = inspect_cmd.output().await {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .trim_start_matches('/')
                    .to_string();

                let _ = tokio::process::Command::new("docker")
                    .args(["rm", "-f", container_id])
                    .output()
                    .await;

                if name.starts_with("cicd-runner-") {
                    let unique_id = name.replace("cicd-runner-", "");
                    let volume_name = format!("cicd-workspace-{}", unique_id);
                    let _ = tokio::process::Command::new("docker")
                        .args(["volume", "rm", "-f", &volume_name])
                        .output()
                        .await;
                }
            } else {
                let _ = tokio::process::Command::new("docker")
                    .args(["rm", "-f", container_id])
                    .output()
                    .await;
            }
        } else {
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", container_id])
                .output()
                .await;
        }
    }
}
