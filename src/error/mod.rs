use async_ftp::FtpError;
use serde_json::Error as SerdeError;
use std::io::Error;

#[derive(Debug)]
pub enum CustomError {
    Io(String),
    Ftp(String),
    Serde(String),
}

impl From<Error> for CustomError {
    fn from(src: Error) -> Self {
        Self::Io(src.to_string())
    }
}
impl From<FtpError> for CustomError {
    fn from(src: FtpError) -> Self {
        Self::Ftp(src.to_string())
    }
}
impl From<SerdeError> for CustomError {
    fn from(src: SerdeError) -> Self {
        Self::Serde(src.to_string())
    }
}
