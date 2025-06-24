fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile(
            &[
                "../proto/common.proto",
                "../proto/ghostchain.proto",
                "../proto/ghostdns.proto",
            ],
            &["../proto"],
        )?;
    Ok(())
}