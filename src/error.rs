use thiserror::Error;
use tower_lsp::jsonrpc::ErrorCode;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    MdzkError(#[from] mdzk::error::Error),
}

impl From<Error> for tower_lsp::jsonrpc::Error {
    fn from(err: Error) -> Self {
        Self {
            code: ErrorCode::InternalError,
            message: err.to_string(),
            data: None,
        }
    }
}
