use thiserror::Error;
use rustc_serialize::json;
use std::io;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] json::BuilderError),
    #[error("{0}")]
    Msg(&'static str),
    #[error("{0}")]
    Request(#[from] reqwest::Error),
}
