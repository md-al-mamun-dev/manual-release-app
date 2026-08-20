use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;

use crate::app_state::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Health check successful", body = HealthResponse),
        (status = 503, description = "Service degraded", body = HealthResponse)
    )
)]
#[get("/health")]
pub async fn health(state: web::Data<AppState>) -> impl Responder {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => HttpResponse::Ok().json(HealthResponse {
            status: "ok",
            database: "ok",
        }),

        Err(error) => {
            tracing::error!(
                error = ?error,
                "health database check failed"
            );

            HttpResponse::ServiceUnavailable().json(HealthResponse {
                status: "degraded",
                database: "unavailable",
            })
        }
    }
}
