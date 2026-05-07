use actix_web::{HttpResponse, ResponseError};
use application::error::AppError;
use serde::Serialize;
use std::fmt;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

/// Newtype wrapper so we can implement `ResponseError` for `AppError`
/// without adding actix-web as a dependency of the application crate.
#[derive(Debug)]
pub struct ApiError(pub AppError);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let body = ErrorBody { error: self.0.to_string() };
        match &self.0 {
            AppError::NotFound(_) => HttpResponse::NotFound().json(body),
            AppError::Conflict(_) => HttpResponse::Conflict().json(body),
            AppError::InvalidInput(_) => HttpResponse::BadRequest().json(body),
            AppError::Internal(_) => {
                tracing::error!(err = %self.0, "internal server error");
                HttpResponse::InternalServerError().json(body)
            }
        }
    }
}
