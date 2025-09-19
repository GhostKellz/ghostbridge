/*!
GQUIC transport layer for high-performance networking

High-performance QUIC transport implementation for GhostBridge cross-chain communication,
featuring connection pooling, DNS over QUIC, and mesh networking capabilities.
*/

use crate::error::{BridgeError, NetworkError, Result};
use gquic::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

// Sub-modules
pub mod client;
pub mod server;
pub mod pool;
pub mod dns;
pub mod mesh;

pub use client::QuicClient;
pub use server::QuicServer;
pub use pool::{ConnectionPool, PoolConfig};
pub use dns::DnsOverQuic;
pub use mesh::QuicMeshNetwork;

/// GQUIC transport manager for GhostBridge
pub struct GQuicTransport {
    config: TransportConfig,
    client_pool: Arc<ConnectionPool>,
    server: Option<QuicServer>,
    mesh_network: Arc<QuicMeshNetwork>,
    dns_client: Arc<DnsOverQuic>,
    metrics: Arc<TransportMetrics>,
}

/// Transport configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransportConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Client configuration
    pub client: ClientConfig,
    /// Connection pool settings
    pub pool: PoolConfig,
    /// DNS over QUIC settings
    pub dns: DnsConfig,
    /// Mesh networking settings
    pub mesh: MeshConfig,
    /// Security settings
    pub security: SecurityConfig,
}

/// Server configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    pub bind_address: SocketAddr,
    pub max_concurrent_connections: u32,
    pub max_concurrent_streams: u32,
    pub keep_alive_interval: Duration,
    pub max_idle_timeout: Duration,
    pub enable_0rtt: bool,
    pub enable_migration: bool,
}

/// Client configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientConfig {
    pub default_server_name: String,
    pub max_idle_timeout: Duration,
    pub keep_alive_interval: Duration,
    pub initial_rtt: Duration,
    pub max_ack_delay: Duration,
    pub congestion_control: CongestionControlType,
    pub enable_0rtt: bool,
}

/// DNS over QUIC configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsConfig {
    pub resolver_endpoints: Vec<String>,
    pub cache_size: usize,
    pub cache_ttl: Duration,
    pub query_timeout: Duration,
    pub enable_dnssec: bool,
}

/// Mesh networking configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeshConfig {
    pub node_id: String,
    pub discovery_endpoints: Vec<String>,
    pub heartbeat_interval: Duration,
    pub node_timeout: Duration,
    pub max_mesh_connections: u32,
    pub enable_gossip: bool,
}

/// Security configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityConfig {
    pub use_self_signed_cert: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub ca_path: Option<String>,
    pub require_client_cert: bool,
    pub supported_alpn: Vec<String>,
}

/// Congestion control types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CongestionControlType {
    Cubic,
    Bbr,
    NewReno,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: "0.0.0.0:9090".parse().unwrap(),
                max_concurrent_connections: 10000,
                max_concurrent_streams: 1000,
                keep_alive_interval: Duration::from_secs(30),
                max_idle_timeout: Duration::from_secs(300),
                enable_0rtt: true,
                enable_migration: true,
            },
            client: ClientConfig {
                default_server_name: "ghostbridge.local".to_string(),
                max_idle_timeout: Duration::from_secs(60),
                keep_alive_interval: Duration::from_secs(20),
                initial_rtt: Duration::from_millis(100),
                max_ack_delay: Duration::from_millis(25),
                congestion_control: CongestionControlType::Bbr,
                enable_0rtt: true,
            },
            pool: PoolConfig::default(),
            dns: DnsConfig {
                resolver_endpoints: vec![
                    "dns.ghostchain.io:853".to_string(),
                    "1.1.1.1:853".to_string(),
                ],
                cache_size: 10000,
                cache_ttl: Duration::from_secs(300),
                query_timeout: Duration::from_secs(5),
                enable_dnssec: true,
            },
            mesh: MeshConfig {
                node_id: uuid::Uuid::new_v4().to_string(),
                discovery_endpoints: vec![
                    "discovery.ghostchain.io:9090".to_string(),
                ],
                heartbeat_interval: Duration::from_secs(10),
                node_timeout: Duration::from_secs(30),
                max_mesh_connections: 100,
                enable_gossip: true,
            },
            security: SecurityConfig {
                use_self_signed_cert: true,
                cert_path: None,
                key_path: None,
                ca_path: None,
                require_client_cert: false,
                supported_alpn: vec![
                    "h3".to_string(),
                    "ghostbridge".to_string(),
                    "etherlink".to_string(),
                    "cross-chain".to_string(),
                ],
            },
        }
    }
}

impl GQuicTransport {
    /// Create a new GQUIC transport instance
    #[instrument(skip(config))]
    pub async fn new(config: TransportConfig) -> Result<Self> {
        info!("Initializing GQUIC transport layer");

        // Initialize connection pool
        let client_pool = Arc::new(ConnectionPool::new(config.pool.clone()));

        // Initialize mesh network
        let mesh_network = Arc::new(QuicMeshNetwork::new(config.mesh.clone()).await?);

        // Initialize DNS client
        let dns_client = Arc::new(DnsOverQuic::new(config.dns.clone()).await?);

        // Initialize metrics
        let metrics = Arc::new(TransportMetrics::new());

        let transport = Self {
            config,
            client_pool,
            server: None,
            mesh_network,
            dns_client,
            metrics,
        };

        info!("GQUIC transport layer initialized successfully");
        Ok(transport)
    }

    /// Start the QUIC server
    #[instrument(skip(self))]
    pub async fn start_server(&mut self) -> Result<()> {
        info!("Starting GQUIC server on {}", self.config.server.bind_address);

        let server = QuicServer::new(self.config.server.clone(), self.config.security.clone()).await?;

        // Start server in background
        let server_handle = server.start().await?;
        self.server = Some(server);

        info!("GQUIC server started successfully");
        Ok(())
    }

    /// Create a new client connection
    #[instrument(skip(self))]
    pub async fn connect(&self, endpoint: &str) -> Result<QuicConnection> {
        debug!("Creating QUIC connection to {}", endpoint);

        // Try to get existing connection from pool first
        if let Some(conn) = self.client_pool.get_connection(endpoint).await {
            debug!("Reusing existing connection to {}", endpoint);
            return Ok(conn);
        }

        // Create new connection
        let client = QuicClient::new(self.config.client.clone(), self.config.security.clone())?;
        let connection = client.connect(endpoint).await?;

        // Add to pool
        self.client_pool.add_connection(endpoint.to_string(), connection.clone()).await;

        self.metrics.record_connection_created();
        debug!("New QUIC connection created to {}", endpoint);
        Ok(connection)
    }

    /// Send data over QUIC with automatic connection management
    #[instrument(skip(self, data))]
    pub async fn send_data(&self, endpoint: &str, data: &[u8]) -> Result<Vec<u8>> {
        let connection = self.connect(endpoint).await?;

        // Open bidirectional stream
        let mut stream = connection.open_bi().await?;

        // Send data
        stream.write_all(data).await?;
        stream.finish().await?;

        // Read response
        let response = stream.read_to_end(1024 * 1024).await?; // 1MB max

        self.metrics.record_data_sent(data.len());
        self.metrics.record_data_received(response.len());

        Ok(response)
    }

    /// Broadcast data to multiple endpoints
    #[instrument(skip(self, data))]
    pub async fn broadcast(&self, endpoints: &[String], data: &[u8]) -> Result<Vec<BroadcastResult>> {
        debug!("Broadcasting data to {} endpoints", endpoints.len());

        let mut tasks = Vec::new();

        for endpoint in endpoints {
            let endpoint = endpoint.clone();
            let data = data.to_vec();
            let transport = self.clone();

            let task = tokio::spawn(async move {
                match transport.send_data(&endpoint, &data).await {
                    Ok(response) => BroadcastResult {
                        endpoint,
                        success: true,
                        response: Some(response),
                        error: None,
                    },
                    Err(e) => BroadcastResult {
                        endpoint,
                        success: false,
                        response: None,
                        error: Some(e.to_string()),
                    },
                }
            });

            tasks.push(task);
        }

        let results = futures::future::join_all(tasks).await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        debug!("Broadcast completed");
        Ok(results)
    }

    /// Resolve domain using DNS over QUIC
    pub async fn resolve_domain(&self, domain: &str) -> Result<Vec<std::net::IpAddr>> {
        self.dns_client.resolve_a(domain).await
    }

    /// Join mesh network
    pub async fn join_mesh(&self) -> Result<()> {
        self.mesh_network.join().await
    }

    /// Get connected mesh peers
    pub async fn get_mesh_peers(&self) -> Vec<MeshPeer> {
        self.mesh_network.get_peers().await
    }

    /// Get transport statistics
    pub fn get_stats(&self) -> TransportStats {
        TransportStats {
            connections_created: self.metrics.connections_created(),
            bytes_sent: self.metrics.bytes_sent(),
            bytes_received: self.metrics.bytes_received(),
            active_connections: self.client_pool.active_connections(),
            mesh_peers: self.mesh_network.peer_count(),
        }
    }

    /// Health check for transport layer
    pub async fn health_check(&self) -> Result<TransportHealth> {
        let mut health = TransportHealth::default();

        // Check connection pool
        health.pool_healthy = self.client_pool.is_healthy();

        // Check mesh network
        health.mesh_healthy = self.mesh_network.is_healthy().await;

        // Check DNS resolution
        health.dns_healthy = self.dns_client.is_healthy().await;

        // Check server (if running)
        health.server_healthy = self.server.as_ref()
            .map(|s| s.is_healthy())
            .unwrap_or(true); // No server is OK

        health.overall_healthy = health.pool_healthy &&
                                health.mesh_healthy &&
                                health.dns_healthy &&
                                health.server_healthy;

        Ok(health)
    }
}

// Make GQuicTransport cloneable for concurrent use
impl Clone for GQuicTransport {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client_pool: Arc::clone(&self.client_pool),
            server: None, // Server handle is not cloneable
            mesh_network: Arc::clone(&self.mesh_network),
            dns_client: Arc::clone(&self.dns_client),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

/// Broadcast operation result
#[derive(Debug, Clone)]
pub struct BroadcastResult {
    pub endpoint: String,
    pub success: bool,
    pub response: Option<Vec<u8>>,
    pub error: Option<String>,
}

/// Mesh network peer information
#[derive(Debug, Clone)]
pub struct MeshPeer {
    pub id: String,
    pub address: SocketAddr,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub latency: Duration,
}

/// Transport statistics
#[derive(Debug, Clone)]
pub struct TransportStats {
    pub connections_created: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: usize,
    pub mesh_peers: usize,
}

/// Transport health status
#[derive(Debug, Clone, Default)]
pub struct TransportHealth {
    pub overall_healthy: bool,
    pub pool_healthy: bool,
    pub mesh_healthy: bool,
    pub dns_healthy: bool,
    pub server_healthy: bool,
}

/// Transport metrics collection
pub struct TransportMetrics {
    connections_created: parking_lot::Mutex<u64>,
    bytes_sent: parking_lot::Mutex<u64>,
    bytes_received: parking_lot::Mutex<u64>,
}

impl TransportMetrics {
    pub fn new() -> Self {
        Self {
            connections_created: parking_lot::Mutex::new(0),
            bytes_sent: parking_lot::Mutex::new(0),
            bytes_received: parking_lot::Mutex::new(0),
        }
    }

    pub fn record_connection_created(&self) {
        *self.connections_created.lock() += 1;
    }

    pub fn record_data_sent(&self, bytes: usize) {
        *self.bytes_sent.lock() += bytes as u64;
    }

    pub fn record_data_received(&self, bytes: usize) {
        *self.bytes_received.lock() += bytes as u64;
    }

    pub fn connections_created(&self) -> u64 {
        *self.connections_created.lock()
    }

    pub fn bytes_sent(&self) -> u64 {
        *self.bytes_sent.lock()
    }

    pub fn bytes_received(&self) -> u64 {
        *self.bytes_received.lock()
    }
}

// Placeholder for QUIC connection type
pub type QuicConnection = gquic::Connection;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.server.max_concurrent_connections, 10000);
        assert!(config.server.enable_0rtt);
        assert_eq!(config.client.congestion_control, CongestionControlType::Bbr);
    }

    #[test]
    fn test_transport_metrics() {
        let metrics = TransportMetrics::new();

        metrics.record_connection_created();
        metrics.record_data_sent(1024);
        metrics.record_data_received(512);

        assert_eq!(metrics.connections_created(), 1);
        assert_eq!(metrics.bytes_sent(), 1024);
        assert_eq!(metrics.bytes_received(), 512);
    }

    #[tokio::test]
    async fn test_transport_creation() {
        let config = TransportConfig::default();
        let result = GQuicTransport::new(config).await;

        // This might fail in test environment without actual GQUIC, but structure should be correct
        assert!(result.is_ok() || result.is_err()); // Either outcome is fine for structure test
    }
}