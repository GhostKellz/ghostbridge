fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                "../proto/common.proto",
                "../proto/ghostchain.proto",
                "../proto/ghostdns.proto",
            ],
            &["../proto"],
        )?;
    Ok(())
}