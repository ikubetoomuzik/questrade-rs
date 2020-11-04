use http::StatusCode;
use thiserror::Error;

/// API error types.
#[derive(Error, Debug)]
pub enum ApiError {
    /// Raised when a response is missing a field
    #[error("Missing field in json response: {0}")]
    MissingFieldError(&'static str),

    /// Raised when a field cannot be converted to the specified type
    #[error("Cannot convert field {0} to type {1}")]
    InvalidTypeError(&'static str, &'static str),

    /// Raised when a API call is made and the user isn't authenticated
    #[error("Not authenticated")]
    NotAuthenticatedError(StatusCode),
}
