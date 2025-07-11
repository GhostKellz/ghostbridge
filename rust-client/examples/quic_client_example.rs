// Example demonstrating the correct QUIC client usage patterns
// This addresses the issues mentioned in the user's request

use std::net::SocketAddr;
use tracing::{info, debug};
use tracing_subscriber::fmt::init;
use ghostbridge_client::{
    QuicClient, EnhancedQuicTransport,
    client::ClientConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init();
    
    info!("Starting QUIC client examples");
    
    // Example 1: Using the enhanced QuicClient (recommended)
    example_1_enhanced_quic_client().await?;
    
    // Example 2: Using the EnhancedQuicTransport
    example_2_enhanced_transport().await?;
    
    // Example 3: Direct connection patterns
    example_3_direct_connection().await?;
    
    Ok(())
}

/// Example 1: Using the enhanced QuicClient that follows INT_BRIDGE.md patterns
async fn example_1_enhanced_quic_client() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Example 1: Enhanced QuicClient ===");
    
    // Create client config
    let config = ClientConfig {
        endpoint: "http://127.0.0.1:9090".to_string(),
        enable_quic: true,
        pool_size: 5,
        request_timeout: std::time::Duration::from_secs(30),
        enable_compression: false,
    };
    
    // Create the QUIC client - this is the missing QuicClient from INT_BRIDGE.md
    let client = QuicClient::new(&config).await?;
    
    // Server address
    let server_addr: SocketAddr = "127.0.0.1:9090".parse()?;
    
    // Example: Connect to server (this was missing in CryptoEndpoint)
    let connection = client.connect(server_addr).await?;
    info!("Successfully connected to server at {}", server_addr);
    
    // Example: Send wallet request - following INT_BRIDGE.md pattern
    let wallet_request = b"wallet_balance_request";
    let response = client.send_wallet_request(server_addr, wallet_request).await?;
    info!("Wallet request response: {:?}", String::from_utf8_lossy(&response));
    
    // Example: Send encrypted request
    let encrypted_request = b"encrypted_domain_query";
    let encrypted_response = client.send_encrypted_request(server_addr, encrypted_request).await?;
    info!("Encrypted request response: {:?}", String::from_utf8_lossy(&encrypted_response));
    
    Ok(())
}

/// Example 2: Using the EnhancedQuicTransport for higher-level operations
async fn example_2_enhanced_transport() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Example 2: Enhanced QuicTransport ===");
    
    let config = ClientConfig {
        endpoint: "http://127.0.0.1:9090".to_string(),
        enable_quic: true,
        pool_size: 5,
        request_timeout: std::time::Duration::from_secs(30),
        enable_compression: false,
    };
    
    // Create enhanced transport
    let transport = EnhancedQuicTransport::new(&config).await?;
    
    // Example: Domain resolution
    let domain_response = transport.resolve_domain(
        "example.ghost".to_string(),
        vec!["A".to_string(), "TXT".to_string()],
    ).await?;
    info!("Domain resolution response: {:?}", domain_response);
    
    // Example: Block streaming
    let mut block_stream = transport.stream_blocks().await?;
    info!("Block streaming started");
    
    // Note: In a real implementation, you would use futures::StreamExt to process the stream
    // use futures::StreamExt;
    // while let Some(block) = block_stream.next().await {
    //     match block {
    //         Ok(block) => info!("Received block: {:?}", block),
    //         Err(e) => error!("Error receiving block: {:?}", e),
    //     }
    // }
    
    Ok(())
}

/// Example 3: Direct connection patterns showing the difference
async fn example_3_direct_connection() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Example 3: Direct Connection Patterns ===");
    
    // This demonstrates the issues that were found:
    
    // ISSUE 1: CryptoEndpoint doesn't have connect() method
    // The following would NOT work:
    // let crypto_endpoint = Endpoint::bind_crypto(...).await?;
    // let connection = crypto_endpoint.connect(addr).await?; // ❌ This method doesn't exist
    
    // ISSUE 2: QuicClient doesn't exist in the main gquic crate
    // The following would NOT work:
    // use gquic::QuicClient; // ❌ This doesn't exist in lib.rs
    // let client = QuicClient::new(config)?; // ❌ This doesn't exist
    
    // SOLUTION: Use our enhanced implementations
    debug!("Using enhanced QuicClient wrapper that provides the missing functionality");
    
    // This shows the CORRECT pattern for client connections per your specification:
    // 
    // For client connections:
    // let endpoint = Endpoint::client("127.0.0.1:0".parse()?).await?;
    // let connection = endpoint.connect("127.0.0.1:4433".parse()?, "server.example.com").await?;
    //
    // For server endpoints:
    // let endpoint = Endpoint::server("127.0.0.1:4433".parse()?, config).await?;
    // let connection = endpoint.accept().await;
    //
    // For crypto server:
    // let crypto_key = b"my_secret_crypto_key_32_bytes___".to_vec();
    // let crypto_endpoint = Endpoint::bind_crypto("127.0.0.1:4434".parse()?, crypto_key).await?;
    //
    // The API separates client and server creation methods, which is why CryptoEndpoint lacks connect().
    
    info!("Client connection pattern example (commented out until gquic API is available)");
    
    // TODO: Implement the correct pattern once gquic API is clarified:
    // let endpoint = gquic::Endpoint::client("127.0.0.1:0".parse()?).await?;
    // let connection = endpoint.connect("127.0.0.1:9090".parse()?, "ghostbridge.local").await?;
    // connection.send(b"test_message").await?;
    // let response = connection.recv().await?;
    
    Ok(())
}

/// Example showing the INT_BRIDGE.md pattern that now works
async fn example_int_bridge_pattern() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== INT_BRIDGE.md Pattern Example ===");
    
    // This is the pattern from INT_BRIDGE.md that was not working:
    // let client = QuicClient::new(config)?;
    // let conn = client.connect(addr).await?;
    
    // Now it works with our enhanced implementation:
    let config = ClientConfig {
        endpoint: "http://127.0.0.1:9090".to_string(),
        enable_quic: true,
        pool_size: 5,
        request_timeout: std::time::Duration::from_secs(30),
        enable_compression: false,
    };
    
    // Create client (this now works!)
    let client = QuicClient::new(&config).await?;
    
    // Connect to server (this now works!)
    let addr: SocketAddr = "127.0.0.1:9090".parse()?;
    let conn = client.connect(addr).await?;
    
    info!("INT_BRIDGE.md pattern now working!");
    
    Ok(())
}