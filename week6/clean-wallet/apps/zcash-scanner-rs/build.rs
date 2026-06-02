// tonic-prost-build로 lightwalletd .proto → Rust gRPC 클라이언트 생성.
// 시스템 protoc 의존을 없애려고 protoc-bin-vendored 사용 (Docker 빌드에도 유리).
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // SAFETY: build 스크립트는 단일 스레드 — bundled protoc의 표준 패턴.
    unsafe {
        std::env::set_var("PROTOC", protoc.as_os_str());
    }

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&["proto/service.proto"], &["proto"])?;
    Ok(())
}
