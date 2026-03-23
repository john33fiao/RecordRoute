use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("record path must start with DB/")]
    InvalidRecordPath,
}
