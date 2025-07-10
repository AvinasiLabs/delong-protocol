//! Defines the context for key derivation, ensuring different keys are generated for different purposes.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    EthAccount,
    Encryption,
}

impl ToString for KeyKind {
    fn to_string(&self) -> String {
        match self {
            KeyKind::EthAccount => "eth_account".to_string(),
            KeyKind::Encryption => "encryption".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyContext {
    kind: KeyKind,
    name: String,
    purpose: String,
}

impl KeyContext {
    pub fn new(kind: KeyKind, name: &str, purpose: &str) -> Self {
        Self {
            kind,
            name: name.to_string(),
            purpose: purpose.to_string(),
        }
    }

    pub fn path(&self) -> PathBuf {
        let safe_name = self.name.replace('/', "_");
        Path::new(&self.kind.to_string()).join(safe_name)
    }

    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    pub fn salt(&self) -> Vec<u8> {
        format!("{}:{}:{}", self.kind.to_string(), self.name, self.purpose).into_bytes()
    }

    pub fn info(&self) -> Vec<u8> {
        format!(
            "purpose={},kind={},name={},version=1",
            self.purpose,
            self.kind.to_string(),
            self.name
        )
        .into_bytes()
    }

    pub fn cache_key(&self) -> String {
        format!("{}:{}", self.path().display(), self.purpose())
    }
}

// Pre-defined key contexts for common operations
pub const KEY_CTX_TEE_CONTRACT_OWNER: &str = "tee-contract-owner"; 