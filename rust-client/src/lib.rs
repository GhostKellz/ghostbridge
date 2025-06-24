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

mod client;
mod connection_pool;
mod quic_transport;

pub use client::{GhostBridgeClient, GhostBridgeError};
pub use connection_pool::ConnectionPool;
pub use quic_transport::QuicTransport;

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
    async fn test_client_creation() {
        let client = GhostBridgeClient::builder()
            .endpoint("http://127.0.0.1:9090")
            .build();
        
        assert!(client.is_ok());
    }
}