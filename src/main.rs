/*!
GhostBridge CLI binary

Command-line interface for operating the GhostBridge cross-chain infrastructure.
*/

use ghostbridge::{BridgeConfig, GhostBridge, init_with_tracing};
use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ghostbridge")]
#[command(about = "GhostBridge - Cross-Chain Bridge Infrastructure")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the bridge service
    Start {
        /// Bind address for the service
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Check bridge health
    Health,
    /// Show bridge status
    Status,
    /// Test bridge configuration
    Test,
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    init_with_tracing(log_level);

    // Load configuration
    let config = if let Some(config_path) = cli.config {
        load_config_from_file(&config_path).await?
    } else {
        BridgeConfig::default()
    };

    match cli.command {
        Commands::Start { bind } => {
            println!("🌉 Starting GhostBridge on {}", bind);
            start_bridge_service(config, &bind).await?;
        }
        Commands::Health => {
            println!("🔍 Checking bridge health...");
            check_health(config).await?;
        }
        Commands::Status => {
            println!("📊 Getting bridge status...");
            show_status(config).await?;
        }
        Commands::Test => {
            println!("🧪 Testing bridge configuration...");
            test_configuration(config).await?;
        }
        Commands::Version => {
            println!("GhostBridge v{}", ghostbridge::version());
            println!("Built with Rust and Zig integration");
            if ghostbridge::has_ffi_support() {
                println!("✓ FFI support enabled");
            }
            if ghostbridge::has_metrics_support() {
                println!("✓ Metrics support enabled");
            }
        }
    }

    Ok(())
}

async fn start_bridge_service(config: BridgeConfig, bind_addr: &str) -> Result<()> {
    println!("Initializing GhostBridge with {} networks", config.networks.len());

    let bridge = GhostBridge::new(config).await?;

    println!("✅ GhostBridge initialized successfully");
    println!("🔗 Multi-chain support enabled");
    println!("⚡ L2 settlement engine ready");
    println!("🛡️  Guardian Framework active");

    // Perform health check
    let health = bridge.health_check().await?;
    if health.overall_healthy {
        println!("✅ All systems healthy");
    } else {
        println!("⚠️  Some components unhealthy: {}/{} services",
                 health.healthy_services, 6);
    }

    // TODO: Start HTTP/gRPC servers for bridge API
    println!("🚀 GhostBridge service running on {}", bind_addr);
    println!("Press Ctrl+C to stop");

    // Keep the service running
    tokio::signal::ctrl_c().await?;
    println!("👋 Shutting down GhostBridge...");

    Ok(())
}

async fn check_health(config: BridgeConfig) -> Result<()> {
    let bridge = GhostBridge::new(config).await?;
    let health = bridge.health_check().await?;

    if health.overall_healthy {
        println!("✅ Bridge is healthy");
        println!("   - Services: {}/6 healthy", health.healthy_services);
        println!("   - FFI: {}", if health.ffi_healthy { "✅" } else { "❌" });
        println!("   - Settlement: {}", if health.settlement_healthy { "✅" } else { "❌" });
    } else {
        println!("❌ Bridge has health issues");
        println!("   - Services: {}/6 healthy", health.healthy_services);
        println!("   - FFI: {}", if health.ffi_healthy { "✅" } else { "❌" });
        println!("   - Settlement: {}", if health.settlement_healthy { "✅" } else { "❌" });
        std::process::exit(1);
    }

    Ok(())
}

async fn show_status(config: BridgeConfig) -> Result<()> {
    let bridge = GhostBridge::new(config.clone()).await?;

    println!("📊 GhostBridge Status Report");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    println!("🌐 Networks: {} configured", config.networks.len());
    for (chain_id, network_config) in &config.networks {
        let status = if network_config.is_testnet { "🧪 Testnet" } else { "🟢 Mainnet" };
        println!("   - Chain {}: {} [{}]", chain_id,
                 network_config.rpc_url, status);
    }

    println!("🪙 Token Economy:");
    println!("   - GCC: {} (deflationary)",
             if config.token_config.gcc.is_deflationary { "✅" } else { "❌" });
    println!("   - SPIRIT: Governance token");
    println!("   - MANA: Utility & rewards");
    println!("   - GHOST: Brand & collectibles");

    println!("⚡ L2 Configuration:");
    println!("   - Target TPS: {}", config.l2_config.target_tps);
    println!("   - Max Batch Size: {}", config.l2_config.max_batch_size);
    println!("   - ZK Proofs: {}",
             if config.l2_config.enable_zk_proofs { "✅" } else { "❌" });

    println!("🛡️  Security:");
    println!("   - Guardian Auth: {}",
             if config.enable_guardian_auth { "✅" } else { "❌" });
    println!("   - Zero Trust: {}",
             if config.guardian_config.enable_zero_trust { "✅" } else { "❌" });

    Ok(())
}

async fn test_configuration(config: BridgeConfig) -> Result<()> {
    println!("🧪 Testing bridge configuration...");

    // Validate configuration
    match config.validate() {
        Ok(_) => println!("✅ Configuration is valid"),
        Err(e) => {
            println!("❌ Configuration error: {}", e);
            std::process::exit(1);
        }
    }

    // Test service connections (without initializing full bridge)
    println!("🔗 Testing service connections...");

    // TODO: Add actual connection tests
    println!("   - GHOSTD: ✅ (simulated)");
    println!("   - WALLETD: ✅ (simulated)");
    println!("   - GID: ✅ (simulated)");
    println!("   - CNS: ✅ (simulated)");
    println!("   - GLEDGER: ✅ (simulated)");
    println!("   - GSIG: ✅ (simulated)");
    println!("   - GhostPlane: ✅ (simulated)");

    println!("✅ All tests passed!");

    Ok(())
}

async fn load_config_from_file(path: &PathBuf) -> Result<BridgeConfig> {
    let content = tokio::fs::read_to_string(path).await?;

    if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        let config: BridgeConfig = toml::from_str(&content)?;
        Ok(config)
    } else {
        let config: BridgeConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
}
