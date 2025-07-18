fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                "../proto/common.proto",
                "../proto/ghostchain.proto",
                "../proto/ghostdns.proto",
                "../proto/eth_bridge.proto",
                "../proto/stellar_bridge.proto",
                "../proto/cross_chain.proto",
            ],
            &["../proto"],
        )?;
    Ok(())
}