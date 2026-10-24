//! Armazenamento das chaves privadas do dispositivo.

use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::IdentityError;

/// Erros de armazenamento.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// O cofre do sistema operacional recusou ou falhou na operação.
    #[error("cofre do sistema: {0}")]
    Keyring(#[from] keyring::Error),
    /// O conteúdo guardado não é uma identidade válida.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// O armazenamento em memória ficou inconsistente (thread em pânico).
    #[error("armazenamento em memória inconsistente")]
    Poisoned,
}

/// Onde as chaves privadas do dispositivo ficam guardadas.
pub trait KeyStore {
    /// Lê o segredo guardado, ou `None` se ainda não existe.
    ///
    /// # Errors
    ///
    /// Retorna erro se o armazenamento falhar.
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError>;

    /// Guarda o segredo, substituindo o anterior.
    ///
    /// # Errors
    ///
    /// Retorna erro se o armazenamento falhar.
    fn save(&self, secret: &[u8]) -> Result<(), StoreError>;
}

/// Cofre do sistema operacional: Gerenciador de Credenciais no Windows,
/// Keychain no macOS e keyutils do kernel no Linux.
#[derive(Debug)]
pub struct KeyringStore {
    entry: keyring::Entry,
}

impl KeyringStore {
    /// Nome do serviço usado no cofre do sistema.
    pub const SERVICE: &'static str = "omegadesk";

    /// Abre a entrada `account` do cofre. Use nomes diferentes para o agente
    /// (serviço do sistema) e para o cliente (usuário).
    ///
    /// # Errors
    ///
    /// Retorna erro se o cofre do sistema não estiver disponível.
    pub fn new(account: &str) -> Result<Self, StoreError> {
        Ok(Self {
            entry: keyring::Entry::new(Self::SERVICE, account)?,
        })
    }
}

impl KeyStore for KeyringStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        match self.entry.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, secret: &[u8]) -> Result<(), StoreError> {
        Ok(self.entry.set_secret(secret)?)
    }
}

/// Armazenamento em memória, para testes e para identidades efêmeras
/// (por exemplo, o `QuickSupport`).
#[derive(Debug, Default)]
pub struct MemoryStore {
    secret: Mutex<Option<Zeroizing<Vec<u8>>>>,
}

impl KeyStore for MemoryStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        let guard = self.secret.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(guard.clone())
    }

    fn save(&self, secret: &[u8]) -> Result<(), StoreError> {
        let mut guard = self.secret.lock().map_err(|_| StoreError::Poisoned)?;
        *guard = Some(Zeroizing::new(secret.to_vec()));
        Ok(())
    }
}
