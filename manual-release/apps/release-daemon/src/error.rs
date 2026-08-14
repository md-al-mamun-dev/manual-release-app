use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    Unprocessable(String),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("internal server error")]
    Internal,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl ApiError {
    fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Internal => "INTERNAL_ERROR",
            Self::Unprocessable(_) => "INSPECTION_FAILED",
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: ErrorDetail {
                code: self.code(),
                message: self.to_string(),
            },
        })
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(database_error) = &error {
            match database_error.code().as_deref() {
                Some("23505") => {
                    return ApiError::Conflict(
                        "a record with the same unique value already exists".to_string(),
                    );
                }

                Some("23503") => {
                    return ApiError::Conflict(
                        "the operation conflicts with an existing record".to_string(),
                    );
                }

                _ => {}
            }
        }

        tracing::error!(
            error = ?error,
            "database operation failed"
        );

        ApiError::Internal
    }
}
