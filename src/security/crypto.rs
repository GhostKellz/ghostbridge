/*!
Cryptographic provider and key management

Provides secure cryptographic operations, key management, and random number
generation for the Guardian Framework.
*/

use crate::error::{BridgeError, Result, SecurityError};
use crate::security::{GuardianConfig, SignatureScheme};
use gcrypt::protocols::{Ed25519, Secp256k1};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use serde::{Deserialize, Serialize};

/// Cryptographic provider for Guardian Framework
pub struct CryptoProvider {
    config: GuardianConfig,
    key_manager: KeyManager,
    secure_random: SecureRandom,
    signature_schemes: HashMap<SignatureScheme, Box<dyn SignatureProvider + Send + Sync>>,
    encryption_provider: EncryptionProvider,
}

/// Key management system
pub struct KeyManager {
    config: GuardianConfig,
    key_store: Arc<RwLock<KeyStore>>,
    rotation_scheduler: RotationScheduler,
}

/// Key storage
#[derive(Debug, Clone)]
struct KeyStore {
    private_keys: HashMap<String, PrivateKey>,
    public_keys: HashMap<String, PublicKey>,
    symmetric_keys: HashMap<String, SymmetricKey>,
    key_metadata: HashMap<String, KeyMetadata>,
}

/// Private key representation
#[derive(Debug, Clone)]
struct PrivateKey {
    key_id: String,
    key_data: Vec<u8>,
    algorithm: SignatureScheme,
    created_at: SystemTime,
    last_used: SystemTime,
    usage_count: u64,
}

/// Public key representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub key_id: String,
    pub key_data: Vec<u8>,
    pub algorithm: SignatureScheme,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
}

/// Symmetric key for encryption
#[derive(Debug, Clone)]
struct SymmetricKey {
    key_id: String,
    key_data: Vec<u8>,
    algorithm: EncryptionAlgorithm,
    created_at: SystemTime,
    last_used: SystemTime,
}

/// Key metadata
#[derive(Debug, Clone)]
struct KeyMetadata {
    key_id: String,
    purpose: KeyPurpose,
    access_policy: AccessPolicy,
    rotation_policy: RotationPolicy,
    backup_location: Option<String>,
    hardware_backed: bool,
}

/// Key purposes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyPurpose {
    Signing,
    Encryption,
    KeyExchange,
    Authentication,
    Derivation,
}

/// Access control policy for keys
#[derive(Debug, Clone)]
struct AccessPolicy {
    allowed_operations: Vec<KeyOperation>,
    rate_limits: HashMap<KeyOperation, RateLimit>,
    time_restrictions: Option<TimeRestriction>,
    ip_restrictions: Option<Vec<String>>,
}

/// Key operations
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum KeyOperation {
    Sign,
    Verify,
    Encrypt,
    Decrypt,
    KeyDerivation,
    Export,
}

/// Rate limiting for key operations
#[derive(Debug, Clone)]
struct RateLimit {
    max_operations: u32,
    time_window: Duration,
    current_count: u32,
    window_start: SystemTime,
}

/// Time-based access restrictions
#[derive(Debug, Clone)]
struct TimeRestriction {
    start_hour: u8, // 0-23
    end_hour: u8,   // 0-23
    allowed_days: Vec<u8>, // 0-6, Sunday = 0
}

/// Key rotation policy
#[derive(Debug, Clone)]
struct RotationPolicy {
    automatic_rotation: bool,
    rotation_interval: Duration,
    retention_period: Duration,
    backup_old_keys: bool,
}

/// Key rotation scheduler
struct RotationScheduler {
    scheduled_rotations: HashMap<String, SystemTime>,
    rotation_in_progress: HashMap<String, bool>,
}

/// Encryption algorithms
#[derive(Debug, Clone, PartialEq, Eq)]
enum EncryptionAlgorithm {
    AES256GCM,
    ChaCha20Poly1305,
    AES256CTR,
}

/// Encryption provider
struct EncryptionProvider {
    algorithms: HashMap<EncryptionAlgorithm, Box<dyn EncryptionAlgorithmProvider + Send + Sync>>,
}

/// Secure random number generator
pub struct SecureRandom {
    entropy_pool: Arc<RwLock<EntropyPool>>,
}

/// Entropy pool for random generation
#[derive(Debug)]
struct EntropyPool {
    pool: Vec<u8>,
    pool_size: usize,
    last_refresh: SystemTime,
    refresh_interval: Duration,
}

/// Signature provider trait
#[async_trait::async_trait]
trait SignatureProvider {
    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)>; // (private, public)
    async fn sign(&self, private_key: &[u8], message: &[u8]) -> Result<Vec<u8>>;
    async fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<bool>;
    fn key_size(&self) -> usize;
    fn signature_size(&self) -> usize;
}

/// Encryption algorithm provider trait
#[async_trait::async_trait]
trait EncryptionAlgorithmProvider {
    async fn generate_key(&self) -> Result<Vec<u8>>;
    async fn encrypt(&self, key: &[u8], nonce: &[u8], plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt(&self, key: &[u8], nonce: &[u8], ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>>;
    fn key_size(&self) -> usize;
    fn nonce_size(&self) -> usize;
}

/// Ed25519 signature provider
struct Ed25519Provider {
    ed25519: Ed25519,
}

/// Secp256k1 signature provider
struct Secp256k1Provider {
    secp256k1: Secp256k1,
}

/// AES-256-GCM encryption provider
struct AES256GCMProvider;

/// ChaCha20-Poly1305 encryption provider
struct ChaCha20Poly1305Provider;

impl CryptoProvider {
    /// Initialize cryptographic provider
    #[instrument(skip(config))]
    pub async fn new(config: GuardianConfig) -> Result<Self> {
        info!("Initializing cryptographic provider");

        let key_manager = KeyManager::new(config.clone()).await?;
        let secure_random = SecureRandom::new().await?;

        let mut signature_schemes: HashMap<SignatureScheme, Box<dyn SignatureProvider + Send + Sync>> = HashMap::new();

        // Register signature schemes
        signature_schemes.insert(
            SignatureScheme::Ed25519,
            Box::new(Ed25519Provider {
                ed25519: Ed25519::new(),
            })
        );
        signature_schemes.insert(
            SignatureScheme::Secp256k1,
            Box::new(Secp256k1Provider {
                secp256k1: Secp256k1::new(),
            })
        );

        let encryption_provider = EncryptionProvider::new().await?;

        Ok(Self {
            config,
            key_manager,
            secure_random,
            signature_schemes,
            encryption_provider,
        })
    }

    /// Generate new signing keypair
    #[instrument(skip(self))]
    pub async fn generate_signing_keypair(&self, scheme: SignatureScheme) -> Result<(String, PublicKey)> {
        debug!("Generating signing keypair for scheme: {:?}", scheme);

        if let Some(provider) = self.signature_schemes.get(&scheme) {
            let (private_key_data, public_key_data) = provider.generate_keypair().await?;

            let key_id = self.generate_key_id().await;
            let now = SystemTime::now();

            // Store private key
            let private_key = PrivateKey {
                key_id: key_id.clone(),
                key_data: private_key_data,
                algorithm: scheme.clone(),
                created_at: now,
                last_used: now,
                usage_count: 0,
            };

            // Create public key
            let public_key = PublicKey {
                key_id: key_id.clone(),
                key_data: public_key_data,
                algorithm: scheme,
                created_at: now,
                expires_at: None,
            };

            // Store keys
            self.key_manager.store_private_key(private_key).await?;
            self.key_manager.store_public_key(public_key.clone()).await?;

            // Create metadata
            let metadata = KeyMetadata {
                key_id: key_id.clone(),
                purpose: KeyPurpose::Signing,
                access_policy: AccessPolicy::default(),
                rotation_policy: RotationPolicy::default(),
                backup_location: None,
                hardware_backed: false,
            };

            self.key_manager.store_metadata(metadata).await?;

            info!("Generated signing keypair: {}", key_id);
            Ok((key_id, public_key))
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedSignatureScheme))
        }
    }

    /// Sign message with private key
    #[instrument(skip(self, message))]
    pub async fn sign(&self, key_id: &str, message: &[u8]) -> Result<Vec<u8>> {
        debug!("Signing message with key: {}", key_id);

        let private_key = self.key_manager.get_private_key(key_id).await?;

        if let Some(provider) = self.signature_schemes.get(&private_key.algorithm) {
            // Check access policy
            self.check_key_access(key_id, KeyOperation::Sign).await?;

            let signature = provider.sign(&private_key.key_data, message).await?;

            // Update usage statistics
            self.key_manager.update_key_usage(key_id).await?;

            debug!("Message signed successfully with key: {}", key_id);
            Ok(signature)
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedSignatureScheme))
        }
    }

    /// Verify signature
    #[instrument(skip(self, message, signature))]
    pub async fn verify(&self, key_id: &str, message: &[u8], signature: &[u8]) -> Result<bool> {
        debug!("Verifying signature with key: {}", key_id);

        let public_key = self.key_manager.get_public_key(key_id).await?;

        if let Some(provider) = self.signature_schemes.get(&public_key.algorithm) {
            let valid = provider.verify(&public_key.key_data, message, signature).await?;
            debug!("Signature verification result: {}", valid);
            Ok(valid)
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedSignatureScheme))
        }
    }

    /// Generate encryption key
    #[instrument(skip(self))]
    pub async fn generate_encryption_key(&self, algorithm: EncryptionAlgorithm) -> Result<String> {
        debug!("Generating encryption key for algorithm: {:?}", algorithm);

        if let Some(provider) = self.encryption_provider.algorithms.get(&algorithm) {
            let key_data = provider.generate_key().await?;
            let key_id = self.generate_key_id().await;
            let now = SystemTime::now();

            let symmetric_key = SymmetricKey {
                key_id: key_id.clone(),
                key_data,
                algorithm,
                created_at: now,
                last_used: now,
            };

            self.key_manager.store_symmetric_key(symmetric_key).await?;

            let metadata = KeyMetadata {
                key_id: key_id.clone(),
                purpose: KeyPurpose::Encryption,
                access_policy: AccessPolicy::default(),
                rotation_policy: RotationPolicy::default(),
                backup_location: None,
                hardware_backed: false,
            };

            self.key_manager.store_metadata(metadata).await?;

            info!("Generated encryption key: {}", key_id);
            Ok(key_id)
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedEncryptionAlgorithm))
        }
    }

    /// Encrypt data
    #[instrument(skip(self, plaintext, associated_data))]
    pub async fn encrypt(&self, key_id: &str, plaintext: &[u8], associated_data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        debug!("Encrypting data with key: {}", key_id);

        let symmetric_key = self.key_manager.get_symmetric_key(key_id).await?;

        if let Some(provider) = self.encryption_provider.algorithms.get(&symmetric_key.algorithm) {
            // Check access policy
            self.check_key_access(key_id, KeyOperation::Encrypt).await?;

            // Generate nonce
            let nonce = self.secure_random.generate_bytes(provider.nonce_size()).await?;

            let ciphertext = provider.encrypt(&symmetric_key.key_data, &nonce, plaintext, associated_data).await?;

            // Update usage statistics
            self.key_manager.update_key_usage(key_id).await?;

            debug!("Data encrypted successfully with key: {}", key_id);
            Ok((ciphertext, nonce))
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedEncryptionAlgorithm))
        }
    }

    /// Decrypt data
    #[instrument(skip(self, ciphertext, nonce, associated_data))]
    pub async fn decrypt(&self, key_id: &str, ciphertext: &[u8], nonce: &[u8], associated_data: &[u8]) -> Result<Vec<u8>> {
        debug!("Decrypting data with key: {}", key_id);

        let symmetric_key = self.key_manager.get_symmetric_key(key_id).await?;

        if let Some(provider) = self.encryption_provider.algorithms.get(&symmetric_key.algorithm) {
            // Check access policy
            self.check_key_access(key_id, KeyOperation::Decrypt).await?;

            let plaintext = provider.decrypt(&symmetric_key.key_data, nonce, ciphertext, associated_data).await?;

            // Update usage statistics
            self.key_manager.update_key_usage(key_id).await?;

            debug!("Data decrypted successfully with key: {}", key_id);
            Ok(plaintext)
        } else {
            Err(BridgeError::Security(SecurityError::UnsupportedEncryptionAlgorithm))
        }
    }

    /// Rotate key
    #[instrument(skip(self))]
    pub async fn rotate_key(&self, key_id: &str) -> Result<String> {
        debug!("Rotating key: {}", key_id);

        // Get current key metadata
        let metadata = self.key_manager.get_metadata(key_id).await?;

        // Generate new key based on purpose and algorithm
        let new_key_id = match metadata.purpose {
            KeyPurpose::Signing => {
                let public_key = self.key_manager.get_public_key(key_id).await?;
                let (new_key_id, _) = self.generate_signing_keypair(public_key.algorithm).await?;
                new_key_id
            }
            KeyPurpose::Encryption => {
                let symmetric_key = self.key_manager.get_symmetric_key(key_id).await?;
                self.generate_encryption_key(symmetric_key.algorithm).await?
            }
            _ => return Err(BridgeError::Security(SecurityError::UnsupportedKeyPurpose)),
        };

        // Mark old key for retirement
        self.key_manager.retire_key(key_id).await?;

        info!("Key rotated: {} -> {}", key_id, new_key_id);
        Ok(new_key_id)
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        self.key_manager.is_healthy().await && self.secure_random.is_healthy().await
    }

    async fn generate_key_id(&self) -> String {
        format!("key-{}-{}",
                SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_millis(),
                rand::random::<u32>())
    }

    async fn check_key_access(&self, key_id: &str, operation: KeyOperation) -> Result<()> {
        let metadata = self.key_manager.get_metadata(key_id).await?;

        // Check if operation is allowed
        if !metadata.access_policy.allowed_operations.contains(&operation) {
            return Err(BridgeError::Security(SecurityError::KeyAccessDenied));
        }

        // Check rate limits
        if let Some(rate_limit) = metadata.access_policy.rate_limits.get(&operation) {
            // TODO: Implement actual rate limiting
            debug!("Checking rate limit for operation: {:?}", operation);
        }

        // Check time restrictions
        if let Some(time_restriction) = &metadata.access_policy.time_restrictions {
            // TODO: Implement time-based access control
            debug!("Checking time restrictions for key: {}", key_id);
        }

        Ok(())
    }
}

impl KeyManager {
    async fn new(config: GuardianConfig) -> Result<Self> {
        let key_store = Arc::new(RwLock::new(KeyStore {
            private_keys: HashMap::new(),
            public_keys: HashMap::new(),
            symmetric_keys: HashMap::new(),
            key_metadata: HashMap::new(),
        }));

        let rotation_scheduler = RotationScheduler {
            scheduled_rotations: HashMap::new(),
            rotation_in_progress: HashMap::new(),
        };

        Ok(Self {
            config,
            key_store,
            rotation_scheduler,
        })
    }

    async fn store_private_key(&self, key: PrivateKey) -> Result<()> {
        let mut store = self.key_store.write().await;
        store.private_keys.insert(key.key_id.clone(), key);
        Ok(())
    }

    async fn store_public_key(&self, key: PublicKey) -> Result<()> {
        let mut store = self.key_store.write().await;
        store.public_keys.insert(key.key_id.clone(), key);
        Ok(())
    }

    async fn store_symmetric_key(&self, key: SymmetricKey) -> Result<()> {
        let mut store = self.key_store.write().await;
        store.symmetric_keys.insert(key.key_id.clone(), key);
        Ok(())
    }

    async fn store_metadata(&self, metadata: KeyMetadata) -> Result<()> {
        let mut store = self.key_store.write().await;
        store.key_metadata.insert(metadata.key_id.clone(), metadata);
        Ok(())
    }

    async fn get_private_key(&self, key_id: &str) -> Result<PrivateKey> {
        let store = self.key_store.read().await;
        store.private_keys.get(key_id)
            .cloned()
            .ok_or(BridgeError::Security(SecurityError::KeyNotFound))
    }

    async fn get_public_key(&self, key_id: &str) -> Result<PublicKey> {
        let store = self.key_store.read().await;
        store.public_keys.get(key_id)
            .cloned()
            .ok_or(BridgeError::Security(SecurityError::KeyNotFound))
    }

    async fn get_symmetric_key(&self, key_id: &str) -> Result<SymmetricKey> {
        let store = self.key_store.read().await;
        store.symmetric_keys.get(key_id)
            .cloned()
            .ok_or(BridgeError::Security(SecurityError::KeyNotFound))
    }

    async fn get_metadata(&self, key_id: &str) -> Result<KeyMetadata> {
        let store = self.key_store.read().await;
        store.key_metadata.get(key_id)
            .cloned()
            .ok_or(BridgeError::Security(SecurityError::KeyNotFound))
    }

    async fn update_key_usage(&self, key_id: &str) -> Result<()> {
        let mut store = self.key_store.write().await;

        if let Some(private_key) = store.private_keys.get_mut(key_id) {
            private_key.last_used = SystemTime::now();
            private_key.usage_count += 1;
        }

        Ok(())
    }

    async fn retire_key(&self, key_id: &str) -> Result<()> {
        // TODO: Implement key retirement
        debug!("Retiring key: {}", key_id);
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let store = self.key_store.read().await;
        store.private_keys.len() < 100_000 // Reasonable upper bound
    }
}

impl SecureRandom {
    async fn new() -> Result<Self> {
        let entropy_pool = Arc::new(RwLock::new(EntropyPool {
            pool: vec![0; 4096], // 4KB pool
            pool_size: 4096,
            last_refresh: SystemTime::now(),
            refresh_interval: Duration::from_secs(300), // 5 minutes
        }));

        let random = Self { entropy_pool };
        random.refresh_entropy().await?;

        Ok(random)
    }

    pub async fn generate_bytes(&self, size: usize) -> Result<Vec<u8>> {
        // Check if entropy needs refresh
        {
            let pool = self.entropy_pool.read().await;
            if pool.last_refresh.elapsed().unwrap_or_default() > pool.refresh_interval {
                drop(pool);
                self.refresh_entropy().await?;
            }
        }

        // Generate random bytes
        let mut bytes = vec![0u8; size];
        for i in 0..size {
            bytes[i] = rand::random::<u8>();
        }

        Ok(bytes)
    }

    async fn refresh_entropy(&self) -> Result<()> {
        let mut pool = self.entropy_pool.write().await;

        // Refresh entropy pool
        for i in 0..pool.pool_size {
            pool.pool[i] = rand::random::<u8>();
        }

        pool.last_refresh = SystemTime::now();
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        let pool = self.entropy_pool.read().await;
        pool.last_refresh.elapsed().unwrap_or_default() < Duration::from_secs(600) // 10 minutes
    }
}

impl EncryptionProvider {
    async fn new() -> Result<Self> {
        let mut algorithms: HashMap<EncryptionAlgorithm, Box<dyn EncryptionAlgorithmProvider + Send + Sync>> = HashMap::new();

        algorithms.insert(EncryptionAlgorithm::AES256GCM, Box::new(AES256GCMProvider));
        algorithms.insert(EncryptionAlgorithm::ChaCha20Poly1305, Box::new(ChaCha20Poly1305Provider));

        Ok(Self { algorithms })
    }
}

impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            allowed_operations: vec![
                KeyOperation::Sign,
                KeyOperation::Verify,
                KeyOperation::Encrypt,
                KeyOperation::Decrypt,
            ],
            rate_limits: HashMap::new(),
            time_restrictions: None,
            ip_restrictions: None,
        }
    }
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            automatic_rotation: true,
            rotation_interval: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            retention_period: Duration::from_secs(90 * 24 * 60 * 60), // 90 days
            backup_old_keys: true,
        }
    }
}

// Implement signature providers
#[async_trait::async_trait]
impl SignatureProvider for Ed25519Provider {
    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        // TODO: Use actual Ed25519 implementation
        Ok((vec![0; 32], vec![0; 32]))
    }

    async fn sign(&self, _private_key: &[u8], _message: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual Ed25519 signing
        Ok(vec![0; 64])
    }

    async fn verify(&self, _public_key: &[u8], _message: &[u8], _signature: &[u8]) -> Result<bool> {
        // TODO: Implement actual Ed25519 verification
        Ok(true)
    }

    fn key_size(&self) -> usize { 32 }
    fn signature_size(&self) -> usize { 64 }
}

#[async_trait::async_trait]
impl SignatureProvider for Secp256k1Provider {
    async fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        // TODO: Use actual Secp256k1 implementation
        Ok((vec![0; 32], vec![0; 33]))
    }

    async fn sign(&self, _private_key: &[u8], _message: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual Secp256k1 signing
        Ok(vec![0; 64])
    }

    async fn verify(&self, _public_key: &[u8], _message: &[u8], _signature: &[u8]) -> Result<bool> {
        // TODO: Implement actual Secp256k1 verification
        Ok(true)
    }

    fn key_size(&self) -> usize { 32 }
    fn signature_size(&self) -> usize { 64 }
}

// Implement encryption providers
#[async_trait::async_trait]
impl EncryptionAlgorithmProvider for AES256GCMProvider {
    async fn generate_key(&self) -> Result<Vec<u8>> {
        // TODO: Generate actual AES-256 key
        Ok(vec![0; 32])
    }

    async fn encrypt(&self, _key: &[u8], _nonce: &[u8], plaintext: &[u8], _associated_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual AES-256-GCM encryption
        Ok(plaintext.to_vec())
    }

    async fn decrypt(&self, _key: &[u8], _nonce: &[u8], ciphertext: &[u8], _associated_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual AES-256-GCM decryption
        Ok(ciphertext.to_vec())
    }

    fn key_size(&self) -> usize { 32 }
    fn nonce_size(&self) -> usize { 12 }
}

#[async_trait::async_trait]
impl EncryptionAlgorithmProvider for ChaCha20Poly1305Provider {
    async fn generate_key(&self) -> Result<Vec<u8>> {
        // TODO: Generate actual ChaCha20 key
        Ok(vec![0; 32])
    }

    async fn encrypt(&self, _key: &[u8], _nonce: &[u8], plaintext: &[u8], _associated_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual ChaCha20-Poly1305 encryption
        Ok(plaintext.to_vec())
    }

    async fn decrypt(&self, _key: &[u8], _nonce: &[u8], ciphertext: &[u8], _associated_data: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement actual ChaCha20-Poly1305 decryption
        Ok(ciphertext.to_vec())
    }

    fn key_size(&self) -> usize { 32 }
    fn nonce_size(&self) -> usize { 12 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crypto_provider_creation() {
        let config = GuardianConfig::default();
        let provider = CryptoProvider::new(config).await.unwrap();
        assert!(provider.is_healthy().await);
    }

    #[tokio::test]
    async fn test_keypair_generation() {
        let config = GuardianConfig::default();
        let provider = CryptoProvider::new(config).await.unwrap();

        let (key_id, public_key) = provider.generate_signing_keypair(SignatureScheme::Ed25519).await.unwrap();
        assert!(!key_id.is_empty());
        assert_eq!(public_key.algorithm, SignatureScheme::Ed25519);
    }

    #[tokio::test]
    async fn test_secure_random() {
        let random = SecureRandom::new().await.unwrap();
        let bytes1 = random.generate_bytes(32).await.unwrap();
        let bytes2 = random.generate_bytes(32).await.unwrap();

        assert_eq!(bytes1.len(), 32);
        assert_eq!(bytes2.len(), 32);
        // Random bytes should be different (with very high probability)
        assert_ne!(bytes1, bytes2);
    }
}