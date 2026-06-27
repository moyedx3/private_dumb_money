fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(
            &["proto/service.proto", "proto/compact_formats.proto"],
            &["proto"],
        )?;
    Ok(())
}
