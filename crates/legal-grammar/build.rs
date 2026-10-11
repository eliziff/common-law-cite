use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::{env, fs, path::PathBuf};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for name in ["grammar-corpus.json", "us-extractors.json"] {
        let source = format!("data/{name}");
        println!("cargo:rerun-if-changed={source}");
        let mut compressed = GzEncoder::new(Vec::new(), Compression::best());
        compressed
            .write_all(&fs::read(source).expect("grammar source"))
            .expect("compress grammar");
        fs::write(
            output.join(format!("{name}.gz")),
            compressed.finish().expect("finish grammar"),
        )
        .expect("embedded grammar");
    }
}
