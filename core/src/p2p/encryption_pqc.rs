/// Post-Quantum Cryptography (PQC) encryption module
/// Implements ML-KEM (formerly Kyber) for key exchange (post-quantum secure)
use crate::error::{MeshError, Result};
use base64::{engine::general_purpose, Engine as _};

/// PQC encryption manager using ML-KEM-768 (formerly Kyber-768)
pub struct PqcEncryptionManager {
    /// ML-KEM-768 public key
    pub public_key: Vec<u8>,
    /// ML-KEM-768 secret key
    #[allow(dead_code)]
    secret_key: Vec<u8>,
}

impl PqcEncryptionManager {
    /// Create a new PQC encryption manager with Kyber768 key pair
    pub fn new() -> Result<Self> {
        #[cfg(feature = "pqc")]
        {
            use pqcrypto_kyber::kyber768::*;
            use pqcrypto_traits::kem::{PublicKey as PqcPublicKey, SecretKey as PqcSecretKey};
            
            let (public_key, secret_key) = keypair();
            
            Ok(Self {
                public_key: public_key.as_bytes().to_vec(),
                secret_key: secret_key.as_bytes().to_vec(),
            })
        }
        
        #[cfg(not(feature = "pqc"))]
        {
            // Fallback: return empty keys if PQC not enabled
            Ok(Self {
                public_key: Vec::new(),
                secret_key: Vec::new(),
            })
        }
    }

    /// Get public key as base64-encoded string
    pub fn get_public_key_string(&self) -> String {
        general_purpose::STANDARD.encode(&self.public_key)
    }

    /// Parse public key from base64-encoded string
    pub fn parse_public_key(encoded: &str) -> Result<Vec<u8>> {
        general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| MeshError::Peer(format!("Failed to decode PQC public key: {}", e)))
    }

    /// Encapsulate (generate shared secret and ciphertext for peer's public key)
    pub fn encapsulate(_peer_public_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        #[cfg(feature = "pqc")]
        {
            use pqcrypto_kyber::kyber768::*;
            use pqcrypto_traits::kem::{
                Ciphertext as PqcCiphertext, PublicKey as PqcPublicKey, SharedSecret,
            };
            
            let pk = <PublicKey as PqcPublicKey>::from_bytes(_peer_public_key)
                .map_err(|_| MeshError::Peer("Invalid Kyber768 public key".to_string()))?;
            
            let (shared_secret, ciphertext) = encapsulate(&pk);
            
            Ok((
                shared_secret.as_bytes().to_vec(),
                ciphertext.as_bytes().to_vec(),
            ))
        }
        
        #[cfg(not(feature = "pqc"))]
        {
            Err(MeshError::Peer(
                "PQC encryption not enabled. Enable 'pqc' feature.".to_string(),
            ))
        }
    }

    /// Decapsulate (recover shared secret from ciphertext using our secret key)
    pub fn decapsulate(&self, _ciphertext: &[u8]) -> Result<Vec<u8>> {
        #[cfg(feature = "pqc")]
        {
            use pqcrypto_kyber::kyber768::*;
            use pqcrypto_traits::kem::{
                Ciphertext as PqcCiphertext, SecretKey as PqcSecretKey, SharedSecret,
            };
            
            let ct = <Ciphertext as PqcCiphertext>::from_bytes(_ciphertext)
                .map_err(|_| MeshError::Peer("Invalid Kyber768 ciphertext".to_string()))?;
            
            let sk = <SecretKey as PqcSecretKey>::from_bytes(&self.secret_key)
                .map_err(|_| MeshError::Peer("Invalid Kyber768 secret key".to_string()))?;
            
            let shared_secret = decapsulate(&ct, &sk);
            
            Ok(shared_secret.as_bytes().to_vec())
        }
        
        #[cfg(not(feature = "pqc"))]
        {
            Err(MeshError::Peer(
                "PQC encryption not enabled. Enable 'pqc' feature.".to_string(),
            ))
        }
    }
}

impl Default for PqcEncryptionManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback: empty keys if PQC not available
            Self {
                public_key: Vec::new(),
                secret_key: Vec::new(),
            }
        })
    }
}

/// Check if PQC is available
pub fn is_pqc_available() -> bool {
    #[cfg(feature = "pqc")]
    {
        true
    }
    
    #[cfg(not(feature = "pqc"))]
    {
        false
    }
}
