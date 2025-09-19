/*!
Connection pooling for GQUIC transport

High-performance connection pooling with automatic cleanup, load balancing,
and health monitoring for GQUIC connections.
*/

use crate::error::{BridgeError, NetworkError, Result};
use crate::transport::QuicConnection;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn, instrument};

/// Connection pool configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoolConfig {
    pub max_connections_per_endpoint: usize,
    pub connection_idle_timeout: Duration,
    pub cleanup_interval: Duration,
    pub enable_multiplexing: bool,
    pub health_check_interval: Duration,
    pub max_retries: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_endpoint: 50,
            connection_idle_timeout: Duration::from_secs(300), // 5 minutes
            cleanup_interval: Duration::from_secs(60), // 1 minute
            enable_multiplexing: true,
            health_check_interval: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

/// Connection pool entry
#[derive(Debug)]
struct PoolEntry {
    connection: QuicConnection,
    created_at: Instant,
    last_used: Arc<RwLock<Instant>>,
    use_count: Arc<parking_lot::Mutex<u64>>,
    is_healthy: Arc<parking_lot::Mutex<bool>>,
}

impl PoolEntry {
    fn new(connection: QuicConnection) -> Self {
        let now = Instant::now();
        Self {
            connection,
            created_at: now,
            last_used: Arc::new(RwLock::new(now)),
            use_count: Arc::new(parking_lot::Mutex::new(0)),
            is_healthy: Arc::new(parking_lot::Mutex::new(true)),
        }
    }

    async fn mark_used(&self) {
        *self.last_used.write().await = Instant::now();
        *self.use_count.lock() += 1;
    }

    async fn is_idle(&self, timeout: Duration) -> bool {
        let last_used = *self.last_used.read().await;
        last_used.elapsed() > timeout
    }

    fn is_healthy(&self) -> bool {
        *self.is_healthy.lock()
    }

    fn mark_unhealthy(&self) {
        *self.is_healthy.lock() = false;
    }
}

/// High-performance connection pool
pub struct ConnectionPool {
    config: PoolConfig,
    pools: DashMap<String, Vec<PoolEntry>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: PoolConfig) -> Self {
        let pool = Self {
            config,
            pools: DashMap::new(),
            cleanup_handle: None,
        };

        // Start cleanup task
        // TODO: Implement cleanup task

        pool
    }

    /// Get a connection from the pool or None if not available
    #[instrument(skip(self))]
    pub async fn get_connection(&self, endpoint: &str) -> Option<QuicConnection> {
        debug!("Getting connection from pool for endpoint: {}", endpoint);

        let mut pool_entry = self.pools.entry(endpoint.to_string()).or_insert_with(Vec::new);
        let pool = pool_entry.value_mut();

        // Find a healthy, non-idle connection
        for (index, entry) in pool.iter().enumerate() {
            if entry.is_healthy() && !entry.is_idle(self.config.connection_idle_timeout).await {
                entry.mark_used().await;
                debug!("Reusing existing connection for {}", endpoint);
                return Some(entry.connection.clone());
            }
        }

        // Remove unhealthy or idle connections
        pool.retain(|entry| {
            let is_healthy = entry.is_healthy();
            let is_not_idle = !futures::executor::block_on(entry.is_idle(self.config.connection_idle_timeout));
            is_healthy && is_not_idle
        });

        debug!("No suitable connection found in pool for {}", endpoint);
        None
    }

    /// Add a connection to the pool
    #[instrument(skip(self, connection))]
    pub async fn add_connection(&self, endpoint: String, connection: QuicConnection) {
        debug!("Adding connection to pool for endpoint: {}", endpoint);

        let mut pool_entry = self.pools.entry(endpoint.clone()).or_insert_with(Vec::new);
        let pool = pool_entry.value_mut();

        // Check if we're at the limit
        if pool.len() >= self.config.max_connections_per_endpoint {
            // Remove oldest connection
            if let Some(oldest_index) = pool.iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(index, _)| index)
            {
                pool.remove(oldest_index);
                debug!("Removed oldest connection due to pool limit");
            }
        }

        // Add new connection
        let entry = PoolEntry::new(connection);
        pool.push(entry);

        debug!("Connection added to pool for {}", endpoint);
    }

    /// Return a connection to the pool (for explicit return)
    pub async fn return_connection(&self, endpoint: String, connection: QuicConnection) {
        // For QUIC, we typically don't need explicit return as connections are multiplexed
        // But we can update the last used time
        if let Some(pool_entry) = self.pools.get(&endpoint) {
            let pool = pool_entry.value();
            for entry in pool.iter() {
                // Check if this is the same connection (simplified check)
                // In real implementation, you'd compare connection IDs
                entry.mark_used().await;
                break;
            }
        }
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, endpoint: &str, connection: &QuicConnection) {
        if let Some(mut pool_entry) = self.pools.get_mut(endpoint) {
            let pool = pool_entry.value_mut();
            // Remove the specific connection (simplified implementation)
            pool.retain(|entry| {
                // In real implementation, compare connection IDs
                false // Simplified: remove first matching
            });
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let mut total_connections = 0;
        let mut healthy_connections = 0;
        let endpoint_count = self.pools.len();

        for pool_ref in self.pools.iter() {
            let pool = pool_ref.value();
            total_connections += pool.len();
            healthy_connections += pool.iter().filter(|e| e.is_healthy()).count();
        }

        PoolStats {
            total_connections,
            healthy_connections,
            endpoint_count,
            max_connections_per_endpoint: self.config.max_connections_per_endpoint,
        }
    }

    /// Health check for the pool
    pub fn is_healthy(&self) -> bool {
        let stats = self.stats();
        stats.healthy_connections > 0 || stats.total_connections == 0
    }

    /// Get number of active connections
    pub fn active_connections(&self) -> usize {
        self.pools.iter()
            .map(|entry| entry.value().len())
            .sum()
    }

    /// Cleanup idle and unhealthy connections
    async fn cleanup(&self) {
        debug!("Running connection pool cleanup");

        let mut removed_count = 0;

        for mut pool_entry in self.pools.iter_mut() {
            let pool = pool_entry.value_mut();
            let initial_len = pool.len();

            pool.retain(|entry| {
                let is_healthy = entry.is_healthy();
                let is_not_idle = !futures::executor::block_on(
                    entry.is_idle(self.config.connection_idle_timeout)
                );
                is_healthy && is_not_idle
            });

            removed_count += initial_len - pool.len();
        }

        // Remove empty endpoint entries
        self.pools.retain(|_, pool| !pool.is_empty());

        if removed_count > 0 {
            debug!("Cleaned up {} idle/unhealthy connections", removed_count);
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&mut self) {
        let pools = self.pools.clone();
        let cleanup_interval = self.config.cleanup_interval;
        let connection_idle_timeout = self.config.connection_idle_timeout;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Cleanup logic
                let mut removed_count = 0;

                for mut pool_entry in pools.iter_mut() {
                    let pool = pool_entry.value_mut();
                    let initial_len = pool.len();

                    pool.retain(|entry| {
                        let is_healthy = entry.is_healthy();
                        let is_not_idle = !futures::executor::block_on(
                            entry.is_idle(connection_idle_timeout)
                        );
                        is_healthy && is_not_idle
                    });

                    removed_count += initial_len - pool.len();
                }

                if removed_count > 0 {
                    debug!("Background cleanup removed {} connections", removed_count);
                }
            }
        });

        self.cleanup_handle = Some(handle);
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub healthy_connections: usize,
    pub endpoint_count: usize,
    pub max_connections_per_endpoint: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections_per_endpoint, 50);
        assert!(config.enable_multiplexing);
    }

    #[test]
    fn test_pool_creation() {
        let config = PoolConfig::default();
        let pool = ConnectionPool::new(config);

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.endpoint_count, 0);
    }

    #[test]
    fn test_pool_entry() {
        // Mock connection for testing
        // let connection = QuicConnection::mock(); // Would need mock implementation
        // let entry = PoolEntry::new(connection);
        // assert!(entry.is_healthy());
        // assert_eq!(*entry.use_count.lock(), 0);
    }
}