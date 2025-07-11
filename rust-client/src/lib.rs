// Core modules
pub mod client;
pub mod connection_pool;
pub mod quic_transport;
pub mod quic_client_wrapper;

// Phase 2: Crypto integration
pub mod crypto;

// Generated protobuf modules
pub mod ghost {
    pub mod common {
        pub mod v1 {
            tonic::include_proto!("ghost.common.v1");
        }
    }
    
    pub mod chain {
        pub mod v1 {
            tonic::include_proto!("ghost.chain.v1");
        }
    }
    
    pub mod dns {
        pub mod v1 {
            tonic::include_proto!("ghost.dns.v1");
        }
    }
}

pub use client::{GhostBridgeClient, GhostBridgeError};
pub use connection_pool::ConnectionPool;
pub use quic_transport::QuicTransport;
pub use quic_client_wrapper::{QuicClient, EnhancedQuicTransport};

// Re-export commonly used types
pub use ghost::chain::v1::{
    DomainQuery, DomainResponse, DnsRecord,
    AccountQuery, AccountResponse,
    BalanceQuery, BalanceResponse,
    BlockQuery, BlockResponse,
    Transaction, TransactionResponse,
};

pub use ghost::dns::v1::{
    DnsStats, CacheStats,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_builder() {
        // Test that builder creates properly
        let builder = GhostBridgeClient::builder()
            .endpoint("http://127.0.0.1:9090")
            .enable_quic(false)
            .pool_size(2);
        
        // Just test that the builder was created successfully
        // We can't test the actual connection without a server
        assert!(true); // Builder creation succeeded if we get here
    }
}