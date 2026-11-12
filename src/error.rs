use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(&'static str),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl From<&'static str> for SearchError {
    fn from(err: &'static str) -> SearchError {
        SearchError::Msg(err)
    }
}
