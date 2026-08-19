use crate::error::{AppError, AppResult};

pub struct SecretStore {
    service: String,
    account: String,
}

impl SecretStore {
    pub fn new() -> Self {
        Self {
            service: "studio.mosaic.modpacks".into(),
            account: "curseforge-api-key".into(),
        }
    }

    fn entry(&self) -> AppResult<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.account).map_err(|error| {
            AppError::Message(format!("O cofre do sistema não está disponível: {error}"))
        })
    }

    pub fn get_curseforge_key(&self) -> Option<String> {
        self.entry().ok()?.get_password().ok()
    }

    pub fn set_curseforge_key(&self, value: &str) -> AppResult<()> {
        self.entry()?.set_password(value).map_err(|error| {
            AppError::Message(format!(
                "Não foi possível proteger a chave no cofre do sistema: {error}"
            ))
        })
    }

    pub fn clear_curseforge_key(&self) -> AppResult<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::Message(format!(
                "Não foi possível remover a chave do cofre: {error}"
            ))),
        }
    }
}
