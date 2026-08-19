use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Falha de rede: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Falha de arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Dados locais inválidos: {0}")]
    Json(#[from] serde_json::Error),
    #[error("URL inválida: {0}")]
    Url(#[from] url::ParseError),
}

pub type AppResult<T> = Result<T, AppError>;

pub trait IntoMessage<T> {
    fn message(self) -> Result<T, String>;
}
impl<T, E: std::fmt::Display> IntoMessage<T> for Result<T, E> {
    fn message(self) -> Result<T, String> {
        self.map_err(|error| error.to_string())
    }
}
