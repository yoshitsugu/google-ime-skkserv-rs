use failure::Fail;
use rustc_serialize::json;
use std::io;

#[derive(Debug, Fail)]
pub enum SearchError {
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] io::Error),
    #[fail(display = "{}", _0)]
    Json(#[fail(cause)] json::BuilderError),
    #[fail(display = "{}", _0)]
    Msg(&'static str),
    #[fail(display = "{}", _0)]
    Request(#[fail(cause)] reqwest::Error),
}

impl From<io::Error> for SearchError {
    fn from(err: io::Error) -> SearchError {
        SearchError::Io(err)
    }
}

impl From<json::BuilderError> for SearchError {
    fn from(err: json::BuilderError) -> SearchError {
        SearchError::Json(err)
    }
}

impl From<&'static str> for SearchError {
    fn from(err: &'static str) -> SearchError {
        SearchError::Msg(err)
    }
}

impl From<reqwest::Error> for SearchError {
    fn from(err: reqwest::Error) -> SearchError {
        SearchError::Request(err)
    }
}
