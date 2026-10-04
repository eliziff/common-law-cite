# Citation source contracts

This private npm package contains public TypeScript wire declarations and the
canonical CanLII route registry. It has no JavaScript or WASM citation runtime;
Beaver executes citation operations through its native addon.

The checked-in declarations are the existing public `ts-rs` contracts previously
shipped with the WASM binding. They describe the Rust types in this crate. Only
when those wire types change, regenerate them explicitly:

```sh
cargo run --manifest-path common-law-cite/Cargo.toml -p legal-citations --features binding-types --bin export-types
```

The generator writes here, independent of its invocation directory. Ordinary
TypeScript installation, checking and tests use these files directly. Actual
WASM packaging copies these declarations into its own package without a separate
Rust type-export build.
