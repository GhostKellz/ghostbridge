use ghostbridge_client::{GhostBridgeClient, DomainQuery};
use tracing::{info, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting GhostBridge client example");

    // Create client with HTTP/2 and QUIC support
    let client = GhostBridgeClient::builder()
        .endpoint("http://127.0.0.1:9090")
        .enable_quic(true)
        .pool_size(4)
        .enable_compression(true)
        .build()
        .await?;

    info!("Client connected successfully");

    // Example 1: Resolve a single domain
    match client.resolve_domain("example.ghost", vec!["A".to_string(), "AAAA".to_string()]).await {
        Ok(response) => {
            info!("Domain resolved: {}", response.domain);
            for record in &response.records {
                info!("  {} record: {}", record.r#type, record.value);
            }
        }
        Err(e) => error!("Failed to resolve domain: {}", e),
    }

    // Example 2: Batch domain resolution
    let batch_queries = vec![
        DomainQuery {
            domain: "app1.ghost".to_string(),
            record_types: vec!["A".to_string()],
        },
        DomainQuery {
            domain: "app2.ghost".to_string(),
            record_types: vec!["A".to_string(), "TXT".to_string()],
        },
        DomainQuery {
            domain: "app3.ghost".to_string(),
            record_types: vec!["AAAA".to_string()],
        },
    ];

    match client.resolve_domains_batch(batch_queries).await {
        Ok(responses) => {
            info!("Batch resolution completed: {} domains", responses.len());
            for response in responses {
                info!("  {}: {} records", response.domain, response.records.len());
            }
        }
        Err(e) => error!("Batch resolution failed: {}", e),
    }

    // Example 3: Get account information
    match client.get_account("ghost1234567890").await {
        Ok(account) => {
            info!("Account info:");
            info!("  ID: {}", account.account_id);
            info!("  Balance: {}", account.balance);
            info!("  Owned domains: {}", account.owned_domains.len());
        }
        Err(e) => error!("Failed to get account: {}", e),
    }

    // Example 4: Get latest block
    match client.get_latest_block().await {
        Ok(block) => {
            info!("Latest block:");
            info!("  Height: {}", block.height);
            info!("  Hash: {}", block.hash);
            info!("  Transactions: {}", block.transactions.len());
        }
        Err(e) => error!("Failed to get latest block: {}", e),
    }

    // Example 5: Get DNS statistics
    match client.get_dns_stats().await {
        Ok(stats) => {
            info!("DNS Statistics:");
            info!("  Total queries: {}", stats.queries_total);
            info!("  Cache hits: {}", stats.cache_hits);
            info!("  Avg response time: {:.2}ms", stats.avg_response_time_ms);
        }
        Err(e) => error!("Failed to get DNS stats: {}", e),
    }

    // Example 6: Subscribe to block updates
    info!("Subscribing to block updates...");
    match client.subscribe_blocks().await {
        Ok(mut stream) => {
            // Listen for a few blocks
            let mut count = 0;
            while let Ok(Some(block)) = stream.message().await {
                info!("New block: height={}, hash={}", block.height, block.hash);
                count += 1;
                if count >= 3 {
                    break;
                }
            }
        }
        Err(e) => error!("Failed to subscribe to blocks: {}", e),
    }

    // Example 7: QUIC-specific request
    if client.resolve_domain_quic("fast.ghost", vec!["A".to_string()]).await.is_ok() {
        info!("QUIC request successful!");
    }

    info!("Example completed");
    Ok(())
}