use thiserror::Error;
use rustc_serialize::json;
use std::io;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] json::BuilderError),
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