//! Gera o código Rust a partir dos arquivos `.proto`.
//!
//! Usa o `protox` (compilador de protobuf em Rust puro), então não é preciso
//! instalar o `protoc` para compilar o projeto.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = ["proto/omegadesk/v1/session.proto"];
    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    let descriptors = protox::compile(protos, ["proto"])?;
    prost_build::Config::new()
        .bytes(["."])
        .compile_fds(descriptors)?;
    Ok(())
}
