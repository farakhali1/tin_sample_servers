use std::{io, path::PathBuf, process::Command};
use tonic_prost_build::configure;

fn resolve_protoc() -> Result<PathBuf, io::Error> {
    // 1) Explicit PROTOC if it points at a real binary.
    if let Some(p) = std::env::var_os("PROTOC") {
        if !p.is_empty() {
            let path = PathBuf::from(&p);
            if path.exists() {
                return Ok(path);
            }
        }
    }

    // 2) System protoc on PATH (fast; avoids rebuilding protobuf-src).
    if let Ok(output) = Command::new("protoc").arg("--version").output() {
        if output.status.success() {
            return Ok(PathBuf::from("protoc"));
        }
    }
    for candidate in ["/usr/bin/protoc", "/usr/local/bin/protoc"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    // 3) Vendored protobuf-src build.
    #[cfg(not(windows))]
    {
        let vendored = protobuf_src::protoc();
        if vendored.exists() {
            return Ok(vendored);
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "vendored protoc missing at {} (stale target after a directory rename?). \
                 Run `cargo clean -p protobuf-src -p protos`, or install protobuf-compiler, \
                 or set PROTOC to a working protoc binary.",
                vendored.display()
            ),
        ));
    }

    #[cfg(windows)]
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "PROTOC is unset; install protobuf-compiler and set PROTOC to the protoc binary",
    ))
}

fn main() -> Result<(), io::Error> {
    let protoc = resolve_protoc()?;

    // SAFETY: build scripts are single-threaded.
    unsafe {
        std::env::set_var("PROTOC", &protoc);
    }
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:warning=protos using protoc={}", protoc.display());

    let proto_base_path = PathBuf::from(".");
    let proto_files = [
        "auth.proto",
        "block_engine.proto",
        "bundle.proto",
        "packet.proto",
        "shared.proto",
    ];
    let mut protos = Vec::new();
    for proto_file in &proto_files {
        let proto = proto_base_path.join(proto_file);
        println!("cargo:rerun-if-changed={}", proto.display());
        protos.push(proto);
    }

    configure()
        .bytes(".packet.Packet.data")
        .build_client(false)
        .build_server(true)
        .server_mod_attribute(".", "#[allow(clippy::default_trait_access)]")
        .server_mod_attribute(".", "#[allow(clippy::mixed_attributes_style)]")
        .compile_protos(&protos, &[proto_base_path])
}
