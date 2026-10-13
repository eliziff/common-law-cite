use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::{env, fs, path::PathBuf};
mod build_registry;

fn compress(path: PathBuf, value: &serde_json::Value) {
    let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
    compressed.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    fs::write(path, compressed.finish().unwrap()).unwrap();
}

fn main() {
    build_registry::compile(&PathBuf::from(env::var_os("OUT_DIR").unwrap()));
    println!("cargo:rerun-if-changed=registry/aliases.json");
    let source: serde_json::Value =
        serde_json::from_slice(&fs::read("registry/aliases.json").expect("alias evidence"))
            .expect("valid alias evidence");
    let runtime: serde_json::Map<String, serde_json::Value> =
        ["keyVersion", "targets", "blockedForms", "observed"]
            .into_iter()
            .map(|field| (field.to_owned(), source[field].clone()))
            .collect();
    let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
    compressed
        .write_all(&serde_json::to_vec(&runtime).unwrap())
        .expect("compress alias mappings");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("aliases.json.gz"),
        compressed.finish().expect("finish alias mappings"),
    )
    .expect("runtime alias mappings");
}
