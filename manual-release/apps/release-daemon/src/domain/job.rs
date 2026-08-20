use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug)]
pub struct JobEvent {
    pub job_id: Uuid,
    pub step_id: Option<Uuid>,
    pub stream: String,
    pub level: String,
    pub message: String,
}

pub async fn append_job_event(
    pool: &PgPool,
    job_id: Uuid,
    step_id: Option<Uuid>,
    stream: &str,
    level: &str,
    message: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO job_events (job_id, step_id, stream, level, message)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        job_id,
        step_id,
        stream,
        level,
        message
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fail_job(
    pool: &PgPool,
    job_id: Uuid,
    error_code: &str,
    error_message: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE jobs
        SET status = 'FAILED',
            error_code = $1,
            error_message = $2,
            finished_at = NOW()
        WHERE id = $3
        "#,
        error_code,
        error_message,
        job_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fail_step(
    pool: &PgPool,
    step_id: Uuid,
    error_code: &str,
    error_message: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE job_steps
        SET status = 'FAILED',
            error_code = $1,
            error_message = $2,
            finished_at = NOW()
        WHERE id = $3
        "#,
        error_code,
        error_message,
        step_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn succeed_step(pool: &PgPool, step_id: Uuid) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        UPDATE job_steps
        SET status = 'SUCCEEDED',
            finished_at = NOW()
        WHERE id = $1
        "#,
        step_id
    )
    .execute(pool)
    .await?;

    Ok(())
}
