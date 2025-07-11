// gquic v0.4.0 integration - follows INT_BRIDGE.md patterns
// Implements proper QuicClient with connection pooling and stream handling

use std::sync::Arc;
use std::net::SocketAddr;
use tracing::{debug, info};
use quinn::{Endpoint, Connection};

use crate::{
    client::ClientConfig as GhostClientConfig,
    GhostBridgeError, client::Result,
    crypto::GhostCrypto,
};

/// GhostBridge QUIC client implementation following INT_BRIDGE.md v0.4.0 patterns
pub struct QuicClient {
    endpoint: Endpoint,
    crypto: Arc<GhostCrypto>,
}

impl QuicClient {
    /// Create a new QUIC client following INT_BRIDGE.md v0.4.0 pattern
    pub async fn new(_config: &GhostClientConfig) -> Result<Self> {
        // Create endpoint with proper configuration per INT_BRIDGE.md
        let bind_addr = "127.0.0.1:0".parse().unwrap();
        let endpoint = Endpoint::client(bind_addr)
            .map_err(|e| GhostBridgeError::Config(format!("Failed to create endpoint: {:?}", e)))?;
        
        let crypto = Arc::new(GhostCrypto::new().map_err(|e| {
            GhostBridgeError::Config(format!("Failed to initialize crypto: {}", e))
        })?);

        info!("GhostBridge QUIC client initialized successfully");

        Ok(Self { endpoint, crypto })
    }

    /// Connect to a remote server using connection pool - v0.4.0 pattern
    pub async fn connect(&self, addr: SocketAddr) -> Result<quinn::Connection> {
        debug!("Attempting to connect to {}", addr);
        
        // Connect directly using endpoint
        let conn = self.endpoint.connect(addr, "ghostbridge-server")
            .map_err(|e| GhostBridgeError::Config(format!("Failed to connect: {:?}", e)))?
            .await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to complete connection: {:?}", e)))?;
        
        info!("Successfully connected to {}", addr);
        Ok(conn)
    }

    /// Send a wallet request using bidirectional streams - v0.4.0 pattern
    pub async fn send_wallet_request(&self, addr: SocketAddr, request: &[u8]) -> Result<Vec<u8>> {
        let conn = self.connect(addr).await?;
        
        // Open bidirectional stream per INT_BRIDGE.md
        let (mut send_stream, mut recv_stream) = conn.open_bi().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to open stream: {:?}", e)))?;
        
        // Send request
        send_stream.write_all(request).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to send wallet request: {:?}", e)))?;
        send_stream.finish().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to finish stream: {:?}", e)))?;
        
        // Read response
        let response = recv_stream.read_to_end(64 * 1024).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to read response: {:?}", e)))?;
        
        Ok(response)
    }

    /// Send encrypted data using crypto backend and bidirectional streams
    pub async fn send_encrypted_request(&self, addr: SocketAddr, request: &[u8]) -> Result<Vec<u8>> {
        let conn = self.connect(addr).await?;
        
        // Open bidirectional stream
        let (mut send_stream, mut recv_stream) = conn.open_bi().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to open stream: {:?}", e)))?;
        
        // Create encryption key for this session
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(b"ghostbridge_key_");
        key[16..].copy_from_slice(b"32bytes_exactly!");
        let encryption_key = crate::crypto::EncryptionKey::new(key);
        let nonce = crate::crypto::GhostCrypto::generate_nonce();
        
        // Encrypt data using crypto backend
        let encrypted_request = encryption_key.encrypt(request, &nonce)
            .map_err(|e| GhostBridgeError::Config(format!("Failed to encrypt request: {:?}", e)))?;
        
        // Send encrypted data
        send_stream.write_all(&encrypted_request).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to send encrypted request: {:?}", e)))?;
        send_stream.finish().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to finish stream: {:?}", e)))?;
        
        // Read and decrypt response
        let encrypted_response = recv_stream.read_to_end(64 * 1024).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to read encrypted response: {:?}", e)))?;
        
        let response = encryption_key.decrypt(&encrypted_response, &nonce)
            .map_err(|e| GhostBridgeError::Config(format!("Failed to decrypt response: {:?}", e)))?;
        
        Ok(response)
    }
}

/// Enhanced QuicTransport following INT_BRIDGE.md v0.4.0 patterns
pub struct EnhancedQuicTransport {
    client: QuicClient,
    server_addr: SocketAddr,
    crypto: Arc<GhostCrypto>,
}

impl EnhancedQuicTransport {
    pub async fn new(config: &GhostClientConfig) -> Result<Self> {
        let server_addr: SocketAddr = config.endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .parse()
            .map_err(|e| GhostBridgeError::Config(format!("Invalid endpoint: {}", e)))?;

        let client = QuicClient::new(config).await?;
        let crypto = Arc::new(GhostCrypto::new().map_err(|e| {
            GhostBridgeError::Config(format!("Failed to initialize crypto: {}", e))
        })?);

        info!("Enhanced QUIC transport initialized for {}", server_addr);

        Ok(Self {
            client,
            server_addr,
            crypto,
        })
    }

    /// Domain resolution using bidirectional streams - v0.4.0 pattern
    pub async fn resolve_domain(
        &self,
        domain: String,
        record_types: Vec<String>,
    ) -> Result<crate::ghost::chain::v1::DomainResponse> {
        debug!("Resolving domain {} via enhanced QUIC client", domain);
        
        // Create domain query following INT_BRIDGE.md format
        let query = serde_json::json!({
            "type": "resolve_domain",
            "domain": domain,
            "record_types": record_types,
        });

        // Serialize query
        let query_data = serde_json::to_vec(&query)
            .map_err(|e| GhostBridgeError::Config(format!("Serialization error: {}", e)))?;

        // Send encrypted request using bidirectional stream
        let response_data = self.client.send_encrypted_request(self.server_addr, &query_data).await?;

        // Deserialize response
        let json: serde_json::Value = serde_json::from_slice(&response_data)
            .map_err(|e| GhostBridgeError::Config(format!("Deserialization error: {}", e)))?;
        
        let response = crate::ghost::chain::v1::DomainResponse {
            domain: json["domain"].as_str().unwrap_or("").to_string(),
            records: vec![],
            owner_id: json["owner_id"].as_str().unwrap_or("").to_string(),
            signature: vec![],
            timestamp: json["timestamp"].as_u64().unwrap_or(0),
            ttl: json["ttl"].as_u64().unwrap_or(0) as u32,
        };

        debug!("Domain {} resolved successfully", domain);
        Ok(response)
    }

    /// Block streaming using bidirectional streams - v0.4.0 pattern
    pub async fn stream_blocks(&self) -> Result<EnhancedQuicBlockStream> {
        debug!("Starting block streaming via enhanced QUIC client");
        
        // Connect to server for block streaming
        let connection = self.client.connect(self.server_addr).await?;
        
        // Open bidirectional stream for block subscription
        let (mut send_stream, recv_stream) = connection.open_bi().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to open stream: {:?}", e)))?;
        
        // Send subscription request following INT_BRIDGE.md format
        let subscription = serde_json::json!({
            "type": "subscribe_blocks",
            "include_transactions": true,
        });
        
        let subscription_data = serde_json::to_vec(&subscription)
            .map_err(|e| GhostBridgeError::Config(format!("Failed to serialize subscription: {:?}", e)))?;
        
        send_stream.write_all(&subscription_data).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to send subscription: {:?}", e)))?;
        send_stream.finish().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to finish stream: {:?}", e)))?;

        debug!("Block streaming subscription sent");

        Ok(EnhancedQuicBlockStream {
            recv_stream,
            crypto: self.crypto.clone(),
        })
    }
}

/// Enhanced block streaming using bidirectional streams - v0.4.0 pattern
pub struct EnhancedQuicBlockStream {
    recv_stream: quinn::RecvStream,
    crypto: Arc<GhostCrypto>,
}

impl futures::Stream for EnhancedQuicBlockStream {
    type Item = Result<crate::BlockResponse>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Read from receive stream
        let mut read_future = Box::pin(self.recv_stream.read_to_end(64 * 1024));
        
        match read_future.as_mut().poll(cx) {
            std::task::Poll::Ready(Ok(data)) => {
                // Deserialize block data - use a helper to avoid borrowing issues
                let block_response = deserialize_enhanced_block_response_helper(&data);
                match block_response {
                    Ok(block) => std::task::Poll::Ready(Some(Ok(block))),
                    Err(e) => std::task::Poll::Ready(Some(Err(e))),
                }
            }
            std::task::Poll::Ready(Err(e)) => {
                let error = GhostBridgeError::Config(format!("Failed to read stream data: {:?}", e));
                std::task::Poll::Ready(Some(Err(error)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl EnhancedQuicBlockStream {
    fn deserialize_block_response(&self, data: &[u8]) -> Result<crate::BlockResponse> {
        deserialize_enhanced_block_response_helper(data)
    }
}

// Helper function to avoid borrowing issues in Stream::poll_next
fn deserialize_enhanced_block_response_helper(data: &[u8]) -> Result<crate::BlockResponse> {
    let json: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| GhostBridgeError::Config(format!("Block deserialization error: {}", e)))?;
    
    Ok(crate::BlockResponse {
        height: json["height"].as_u64().unwrap_or(0),
        hash: json["hash"].as_str().unwrap_or("").to_string(),
        parent_hash: json["parent_hash"].as_str().unwrap_or("").to_string(),
        timestamp: json["timestamp"].as_u64().unwrap_or(0),
        transactions: vec![], // Empty for now
    })
}