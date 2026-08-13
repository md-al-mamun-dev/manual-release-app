use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;

use crate::app_state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    database: &'static str,
}

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
