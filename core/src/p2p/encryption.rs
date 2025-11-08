/// Encryption module for secure P2P communication
/// Implements RSA for key exchange and AES-GCM for message encryption
use crate::error::{MeshError, Result};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, generic_array::GenericArray},
    Aes256Gcm, Key,
};
use base64::{engine::general_purpose, Engine as _};
use rsa::{pkcs1v15::SigningKey, pkcs8::{DecodePublicKey, EncodePublicKey}, RsaPrivateKey, RsaPublicKey, pkcs1v15::VerifyingKey, pkcs1v15::EncryptingKey, pkcs1v15::DecryptingKey, Oaep};
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Encryption manager for a node
pub struct EncryptionManager {
    private_key: Arc<RwLock<RsaPrivateKey>>,
    public_key: Arc<RsaPublicKey>,
}

impl EncryptionManager {
    /// Generate a new encryption manager with RSA key pair
    pub fn new() -> Result<Self> {
        let mut rng = rand::thread_rng();
        let bits = 2048;
        let private_key = RsaPrivateKey::new(&mut rng, bits)
            .map_err(|e| MeshError::Peer(format!("Failed to generate RSA key: {}", e)))?;
        let public_key = RsaPublicKey::from(&private_key);
        
        Ok(Self {
            private_key: Arc::new(RwLock::new(private_key)),
            public_key: Arc::new(public_key),
        })
    }

    /// Get public key as base64-encoded string
    pub fn get_public_key_string(&self) -> Result<String> {
        use rsa::pkcs8::EncodePublicKey;
        let pub_key_der = self.public_key.to_public_key_der()
            .map_err(|e| MeshError::Peer(format!("Failed to serialize public key: {}", e)))?;
        Ok(general_purpose::STANDARD.encode(pub_key_der.as_bytes()))
    }

    /// Parse public key from base64-encoded string
    pub fn parse_public_key(encoded: &str) -> Result<RsaPublicKey> {
        use rsa::pkcs8::DecodePublicKey;
        let der_bytes = general_purpose::STANDARD.decode(encoded)
            .map_err(|e| MeshError::Peer(format!("Failed to decode public key: {}", e)))?;
        RsaPublicKey::from_public_key_der(&der_bytes)
            .map_err(|e| MeshError::Peer(format!("Failed to parse public key: {}", e)))
    }

    /// Encrypt data with peer's public key (RSA OAEP) - for small data only (key exchange)
    pub fn encrypt_with_public_key(&self, data: &[u8], peer_public_key: &RsaPublicKey) -> Result<Vec<u8>> {
        use rsa::rand_core::OsRng;
        
        let mut rng = OsRng;
        let padding = Oaep::new::<Sha256>();
        peer_public_key.encrypt(&mut rng, padding, data)
            .map_err(|e| MeshError::Peer(format!("RSA encryption failed: {}", e)))
    }

    /// Decrypt data with our private key (RSA OAEP)
    pub async fn decrypt_with_private_key(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        let private_key = self.private_key.read().await;
        let padding = Oaep::new::<Sha256>();
        private_key.decrypt(padding, encrypted)
            .map_err(|e| MeshError::Peer(format!("RSA decryption failed: {}", e)))
    }

    /// Generate AES session key
    pub fn generate_session_key() -> (Key<Aes256Gcm>, Vec<u8>) {
        let key = Aes256Gcm::generate_key(&mut OsRng);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        (key, nonce.as_slice().to_vec())
    }

    /// Encrypt message with AES-GCM
    pub fn encrypt_aes(data: &[u8], key: &Key<Aes256Gcm>, nonce: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != 12 {
            return Err(MeshError::Peer("Nonce must be 12 bytes".to_string()));
        }
        let cipher = Aes256Gcm::new(key);
        let nonce_array = GenericArray::from_slice(nonce);
        cipher.encrypt(nonce_array, data)
            .map_err(|e| MeshError::Peer(format!("AES encryption failed: {}", e)))
    }

    /// Decrypt message with AES-GCM
    pub fn decrypt_aes(encrypted: &[u8], key: &Key<Aes256Gcm>, nonce: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != 12 {
            return Err(MeshError::Peer("Nonce must be 12 bytes".to_string()));
        }
        let cipher = Aes256Gcm::new(key);
        let nonce_array = GenericArray::from_slice(nonce);
        cipher.decrypt(nonce_array, encrypted)
            .map_err(|e| MeshError::Peer(format!("AES decryption failed: {}", e)))
    }

    /// Hybrid encryption: encrypt data with AES, encrypt AES key with RSA
    pub async fn hybrid_encrypt(&self, data: &[u8], peer_public_key: &RsaPublicKey) -> Result<EncryptedMessage> {
        // Generate AES session key
        let (aes_key, nonce) = Self::generate_session_key();
        
        // Encrypt data with AES
        let encrypted_data = Self::encrypt_aes(data, &aes_key, &nonce)?;
        
        // Encrypt AES key with RSA (AES-256 key is 32 bytes, fits in RSA-2048)
        let aes_key_bytes = aes_key.as_slice();
        let encrypted_key = self.encrypt_with_public_key(aes_key_bytes, peer_public_key)?;
        
        Ok(EncryptedMessage {
            encrypted_key,
            nonce,
            encrypted_data,
        })
    }

    /// Hybrid decryption: decrypt AES key with RSA, decrypt data with AES
    pub async fn hybrid_decrypt(&self, encrypted: &EncryptedMessage) -> Result<Vec<u8>> {
        // Decrypt AES key with RSA
        let aes_key_bytes = self.decrypt_with_private_key(&encrypted.encrypted_key).await?;
        let aes_key = Key::<Aes256Gcm>::from_slice(&aes_key_bytes);
        
        // Decrypt data with AES
        Self::decrypt_aes(&encrypted.encrypted_data, aes_key, &encrypted.nonce)
    }
}

impl Clone for EncryptionManager {
    fn clone(&self) -> Self {
        Self {
            private_key: self.private_key.clone(),
            public_key: Arc::new(self.public_key.as_ref().clone()),
        }
    }
}

/// Encrypted message structure
#[derive(Debug, Clone)]
pub struct EncryptedMessage {
    pub encrypted_key: Vec<u8>,    // AES key encrypted with RSA
    pub nonce: Vec<u8>,            // AES nonce
    pub encrypted_data: Vec<u8>,   // Data encrypted with AES
}

impl EncryptedMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data)
            .map_err(|e| MeshError::Serialization(e))
    }
}

impl serde::Serialize for EncryptedMessage {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("EncryptedMessage", 3)?;
        state.serialize_field("key", &general_purpose::STANDARD.encode(&self.encrypted_key))?;
        state.serialize_field("nonce", &general_purpose::STANDARD.encode(&self.nonce))?;
        state.serialize_field("data", &general_purpose::STANDARD.encode(&self.encrypted_data))?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for EncryptedMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct EncryptedMessageVisitor;

        impl<'de> Visitor<'de> for EncryptedMessageVisitor {
            type Value = EncryptedMessage;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct EncryptedMessage")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<EncryptedMessage, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut encrypted_key = None;
                let mut nonce = None;
                let mut encrypted_data = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "key" => {
                            if encrypted_key.is_some() {
                                return Err(de::Error::duplicate_field("key"));
                            }
                            let encoded: String = map.next_value()?;
                            encrypted_key = Some(general_purpose::STANDARD.decode(&encoded)
                                .map_err(de::Error::custom)?);
                        }
                        "nonce" => {
                            if nonce.is_some() {
                                return Err(de::Error::duplicate_field("nonce"));
                            }
                            let encoded: String = map.next_value()?;
                            nonce = Some(general_purpose::STANDARD.decode(&encoded)
                                .map_err(de::Error::custom)?);
                        }
                        "data" => {
                            if encrypted_data.is_some() {
                                return Err(de::Error::duplicate_field("data"));
                            }
                            let encoded: String = map.next_value()?;
                            encrypted_data = Some(general_purpose::STANDARD.decode(&encoded)
                                .map_err(de::Error::custom)?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let encrypted_key = encrypted_key.ok_or_else(|| de::Error::missing_field("key"))?;
                let nonce = nonce.ok_or_else(|| de::Error::missing_field("nonce"))?;
                let encrypted_data = encrypted_data.ok_or_else(|| de::Error::missing_field("data"))?;

                Ok(EncryptedMessage {
                    encrypted_key,
                    nonce,
                    encrypted_data,
                })
            }
        }

        deserializer.deserialize_map(EncryptedMessageVisitor)
    }
}
