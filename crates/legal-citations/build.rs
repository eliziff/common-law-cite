use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=registry/aliases.json");
    let source: serde_json::Value = serde_json::from_slice(
        &fs::read("registry/aliases.json").expect("alias evidence"),
    ).expect("valid alias evidence");
    let runtime: serde_json::Map<String, serde_json::Value> =
        ["keyVersion", "targets", "blockedForms", "observed"].into_iter()
            .map(|field| (field.to_owned(), source[field].clone())).collect();
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("aliases.json"),
        serde_json::to_vec(&runtime).unwrap()).expect("runtime alias mappings");
}
