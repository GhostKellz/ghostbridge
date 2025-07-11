// QUIC transport using quinn library - stable implementation
use std::sync::Arc;
use std::net::SocketAddr;
use bytes::Bytes;
use tracing::{debug, error, info};
use quinn::{Endpoint, Connection};

use crate::{
    client::ClientConfig as GhostClientConfig,
    ghost::chain::v1::{DomainQuery, DomainResponse},
    GhostBridgeError, client::Result,
    crypto::GhostCrypto,
};

pub struct QuicTransport {
    client_endpoint: Endpoint,
    server_addr: SocketAddr,
    crypto: Arc<GhostCrypto>,
}

impl QuicTransport {
    pub async fn new(config: &GhostClientConfig) -> Result<Self> {
        let server_addr: SocketAddr = config.endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .parse()
            .map_err(|e| GhostBridgeError::Config(format!("Invalid endpoint: {}", e)))?;

        // Initialize crypto for QUIC transport
        let crypto = Arc::new(GhostCrypto::new().map_err(|e| {
            GhostBridgeError::Config(format!("Failed to initialize crypto: {}", e))
        })?);

        // Create Quinn client endpoint with proper configuration
        let mut client_config = quinn::ClientConfig::with_native_roots();
        
        let bind_addr = "127.0.0.1:0".parse().unwrap();
        let client_endpoint = Endpoint::client(bind_addr)
            .map_err(|e| GhostBridgeError::Config(format!("Failed to create endpoint: {:?}", e)))?;

        info!("QUIC transport initialized for {} with Quinn", server_addr);

        Ok(Self {
            client_endpoint,
            server_addr,
            crypto,
        })
    }

    pub async fn resolve_domain(
        &self,
        domain: String,
        record_types: Vec<String>,
    ) -> Result<DomainResponse> {
        debug!("Resolving domain {} via QUIC", domain);
        
        // Create domain query following INT_BRIDGE.md format
        let query = serde_json::json!({
            "type": "resolve_domain",
            "domain": domain,
            "record_types": record_types,
        });

        // Serialize query
        let query_data = serde_json::to_vec(&query)
            .map_err(|e| GhostBridgeError::Config(format!("Serialization error: {}", e)))?;

        // Connect to server using Quinn
        let conn = self.client_endpoint.connect(self.server_addr, "ghostbridge-server")
            .map_err(|e| GhostBridgeError::Config(format!("Failed to connect: {:?}", e)))?
            .await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to complete connection: {:?}", e)))?;
        
        // Open bidirectional stream for request/response
        let (mut send_stream, mut recv_stream) = conn.open_bi().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to open stream: {:?}", e)))?;
        
        // Send query
        send_stream.write_all(&query_data).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to send query: {:?}", e)))?;
        send_stream.finish().await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to finish stream: {:?}", e)))?;

        // Receive response
        let response_data = recv_stream.read_to_end(64 * 1024).await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to receive response: {:?}", e)))?;

        // Deserialize response
        let response = deserialize_domain_response(&response_data)?;

        debug!("Domain {} resolved successfully", domain);
        Ok(response)
    }

    pub async fn stream_blocks(&self) -> Result<QuicBlockStream> {
        debug!("Starting block streaming via QUIC");
        
        // Connect to server using Quinn
        let conn = self.client_endpoint.connect(self.server_addr, "ghostbridge-server")
            .map_err(|e| GhostBridgeError::Config(format!("Failed to connect: {:?}", e)))?
            .await
            .map_err(|e| GhostBridgeError::Config(format!("Failed to complete connection: {:?}", e)))?;
        
        // Open bidirectional stream for block subscription
        let (mut send_stream, recv_stream) = conn.open_bi().await
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

        Ok(QuicBlockStream {
            recv_stream,
            crypto: self.crypto.clone(),
        })
    }
}

// QUIC block streaming implementation using bidirectional streams
pub struct QuicBlockStream {
    recv_stream: quinn::RecvStream,
    crypto: Arc<GhostCrypto>,
}

impl futures::Stream for QuicBlockStream {
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
                let block_response = deserialize_block_response_helper(&data);
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

impl QuicBlockStream {
    fn deserialize_block_response(&self, data: &[u8]) -> Result<crate::BlockResponse> {
        deserialize_block_response_helper(data)
    }
}

// Helper function to avoid borrowing issues in Stream::poll_next
fn deserialize_block_response_helper(data: &[u8]) -> Result<crate::BlockResponse> {
    // In production, use proper protobuf deserialization
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

// Simplified serialization for prototype
fn serialize_domain_query(query: &DomainQuery) -> Result<Vec<u8>> {
    // In production, use proper protobuf serialization
    let json = serde_json::json!({
        "domain": query.domain,
        "record_types": query.record_types,
    });
    
    serde_json::to_vec(&json)
        .map_err(|e| GhostBridgeError::Config(format!("Serialization error: {}", e)))
}

fn deserialize_domain_response(data: &[u8]) -> Result<DomainResponse> {
    // In production, use proper protobuf deserialization
    let json: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| GhostBridgeError::Config(format!("Deserialization error: {}", e)))?;
    
    Ok(DomainResponse {
        domain: json["domain"].as_str().unwrap_or("").to_string(),
        records: vec![],
        owner_id: json["owner_id"].as_str().unwrap_or("").to_string(),
        signature: vec![],
        timestamp: json["timestamp"].as_u64().unwrap_or(0),
        ttl: json["ttl"].as_u64().unwrap_or(0) as u32,
    })
}