use core::fmt;

pub type Result<T> = core::result::Result<T, KmsError>;

#[derive(Debug, Clone, PartialEq)]
pub enum KmsError {
    CryptoError,
    InvalidKey,
    KeyNotFound,
    StorageError,
    SerializationError,
    InvalidInput,
}

impl fmt::Display for KmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KmsError::CryptoError => write!(f, "Cryptographic operation failed"),
            KmsError::InvalidKey => write!(f, "Invalid key format or data"),
            KmsError::KeyNotFound => write!(f, "Key not found"),
            KmsError::StorageError => write!(f, "Storage operation failed"),
            KmsError::SerializationError => write!(f, "Serialization/deserialization failed"),
            KmsError::InvalidInput => write!(f, "Invalid input parameters"),
        }
    }
}