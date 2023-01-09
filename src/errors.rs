use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt::{Display, Formatter};

#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Error {
    FileTooLarge {
        max_size: usize,
    },
    FileTypeNotAllowed,

    InvalidData,
    MissingData,

    DatabaseError,
    UnknownStore,
    UnknownError,

    NotFound,
    ProcessingError,
    StorageError,

    MetaParseFailed,
    MissingContentType,
    CannotProxy,
    InternalRequestFailed,
    RequestFailed,
    ValidationFailed,
}

impl Display for Error {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match &self {
            Error::FileTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Error::FileTypeNotAllowed => StatusCode::BAD_REQUEST,

            Error::InvalidData => StatusCode::BAD_REQUEST,
            Error::MissingData => StatusCode::BAD_REQUEST,

            Error::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
            Error::UnknownStore => StatusCode::BAD_REQUEST,
            Error::UnknownError => StatusCode::INTERNAL_SERVER_ERROR,

            Error::NotFound => StatusCode::NOT_FOUND,
            Error::ProcessingError => StatusCode::INTERNAL_SERVER_ERROR,
            Error::StorageError => StatusCode::INTERNAL_SERVER_ERROR,

            Error::MetaParseFailed => StatusCode::INTERNAL_SERVER_ERROR,
            Error::MissingContentType => StatusCode::BAD_REQUEST,
            Error::CannotProxy => StatusCode::BAD_REQUEST,
            Error::InternalRequestFailed => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RequestFailed => StatusCode::BAD_REQUEST,
            Error::ValidationFailed => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
