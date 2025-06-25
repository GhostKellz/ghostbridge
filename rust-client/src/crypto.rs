use std::convert::TryFrom;
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, KeyInit};
use chacha20poly1305::aead::Aead;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use hkdf::Hkdf;
use blake3::Hasher;
use sha2::Sha256;
use zeroize::Zeroize;

/// Cryptographic operations for GhostBridge
/// Implements the recommendations for secure communication:
/// - X25519 key exchange for secure channels
/// - ChaCha20-Poly1305 for high-performance encryption  
/// - HKDF for proper key derivation
/// - Ed25519 for signatures
pub struct GhostCrypto {
    signing_key: SigningKey,
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Key generation failed")]
    KeyGeneration,
    
    #[error("Encryption failed: {0}")]
    Encryption(String),
    
    #[error("Decryption failed: {0}")]
    Decryption(String),
    
    #[error("Signature verification failed")]
    SignatureVerification,
    
    #[error("Key derivation failed")]
    KeyDerivation,
    
    #[error("Invalid key length")]
    InvalidKeyLength,
}

pub type Result<T> = std::result::Result<T, CryptoError>;

impl GhostCrypto {
    /// Create new crypto instance with fresh Ed25519 keypair
    pub fn new() -> Result<Self> {
        use rand::RngCore;
        let mut csprng = rand::rngs::OsRng;
        let mut secret_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        
        Ok(Self { signing_key })
    }
    
    /// Load from existing secret key
    pub fn from_secret_key(secret_bytes: &[u8]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(
            secret_bytes.try_into()
                .map_err(|_| CryptoError::InvalidKeyLength)?
        );
        
        Ok(Self { signing_key })
    }
    
    /// Get our public key for identity
    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
    
    /// Sign a message with our Ed25519 key
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }
    
    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> Result<()> {
        let verifying_key = VerifyingKey::from_bytes(
            public_key.try_into()
                .map_err(|_| CryptoError::SignatureVerification)?
        ).map_err(|_| CryptoError::SignatureVerification)?;
        
        let signature = Signature::from_bytes(
            signature.try_into()
                .map_err(|_| CryptoError::SignatureVerification)?
        );
            
        verifying_key.verify(message, &signature)
            .map_err(|_| CryptoError::SignatureVerification)
    }
    
    /// Perform X25519 key exchange and derive encryption key
    pub fn key_exchange(&self, peer_public_key: &[u8]) -> Result<EncryptionKey> {
        // Generate ephemeral X25519 key
        let ephemeral_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);
        
        // Peer's X25519 public key
        let peer_public = X25519PublicKey::from(
            <[u8; 32]>::try_from(peer_public_key)
                .map_err(|_| CryptoError::InvalidKeyLength)?
        );
        
        // Perform DH exchange
        let shared_secret = ephemeral_secret.diffie_hellman(&peer_public);
        
        // Derive encryption key using HKDF
        let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        let mut encryption_key = [0u8; 32];
        hk.expand(b"ghostbridge-encryption", &mut encryption_key)
            .map_err(|_| CryptoError::KeyDerivation)?;
            
        Ok(EncryptionKey {
            key: encryption_key,
            ephemeral_public: ephemeral_public.to_bytes(),
        })
    }
    
    /// Hash data using BLAKE3
    pub fn hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize().into()
    }
    
    /// Generate secure random nonce
    pub fn generate_nonce() -> [u8; 12] {
        use rand::RngCore;
        let mut nonce = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        nonce
    }
}

/// Encryption key derived from X25519 key exchange
pub struct EncryptionKey {
    key: [u8; 32],
    pub ephemeral_public: [u8; 32],
}

impl EncryptionKey {
    /// Encrypt data using ChaCha20-Poly1305
    pub fn encrypt(&self, plaintext: &[u8], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        let key = Key::from_slice(&self.key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(nonce);
        
        cipher.encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::Encryption(e.to_string()))
    }
    
    /// Decrypt data using ChaCha20-Poly1305
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        let key = Key::from_slice(&self.key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(nonce);
        
        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

/// WASM-safe crypto operations for web integration
#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    
    #[wasm_bindgen]
    pub struct WasmGhostCrypto {
        inner: GhostCrypto,
    }
    
    #[wasm_bindgen]
    impl WasmGhostCrypto {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Result<WasmGhostCrypto, JsValue> {
            GhostCrypto::new()
                .map(|inner| Self { inner })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        
        #[wasm_bindgen]
        pub fn public_key(&self) -> Vec<u8> {
            self.inner.public_key().to_vec()
        }
        
        #[wasm_bindgen]
        pub fn sign(&self, message: &[u8]) -> Vec<u8> {
            self.inner.sign(message).to_vec()
        }
        
        #[wasm_bindgen]
        pub fn verify(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
            self.inner.verify(message, signature, public_key).is_ok()
        }
        
        #[wasm_bindgen]
        pub fn hash(&self, data: &[u8]) -> Vec<u8> {
            self.inner.hash(data).to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_generation() {
        let crypto = GhostCrypto::new().unwrap();
        assert_eq!(crypto.public_key().len(), 32);
    }
    
    #[test]
    fn test_sign_verify() {
        let crypto = GhostCrypto::new().unwrap();
        let message = b"hello world";
        let signature = crypto.sign(message);
        let public_key = crypto.public_key();
        
        assert!(crypto.verify(message, &signature, &public_key).is_ok());
    }
    
    #[test]
    fn test_encryption() {
        let crypto1 = GhostCrypto::new().unwrap();
        let crypto2 = GhostCrypto::new().unwrap();
        
        // Simulate key exchange (simplified for test)
        let fake_peer_key = [1u8; 32]; // In real usage, this would be crypto2's X25519 public key
        let encryption_key = crypto1.key_exchange(&fake_peer_key).unwrap();
        
        let plaintext = b"secret message";
        let nonce = GhostCrypto::generate_nonce();
        
        let ciphertext = encryption_key.encrypt(plaintext, &nonce).unwrap();
        let decrypted = encryption_key.decrypt(&ciphertext, &nonce).unwrap();
        
        assert_eq!(plaintext, decrypted.as_slice());
    }
    
    #[test]
    fn test_hash() {
        let crypto = GhostCrypto::new().unwrap();
        let data = b"test data";
        let hash1 = crypto.hash(data);
        let hash2 = crypto.hash(data);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32);
    }
}
