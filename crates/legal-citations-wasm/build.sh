#!/usr/bin/env bash
# Build the npm package into crates/legal-citations-wasm/js/pkg.
#
# Needs: rustup target wasm32-unknown-unknown, and wasm-bindgen-cli at the exact
# version pinned in Cargo.toml (cargo install wasm-bindgen-cli --version <v>).
set -euo pipefail
profile=debug
build_args=()
if [[ "${1:-}" == --release ]]; then
  profile=release
  build_args+=(--release)
  shift
fi
if (( $# )); then echo "Usage: build.sh [--release]" >&2; exit 2; fi
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"
target_dir="$(cargo metadata --locked --offline --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",c=>s+=c).on("end",()=>process.stdout.write(JSON.parse(s).target_directory.replace(/\\/g,"/")))')"

pinned="$(sed -n 's/^wasm-bindgen = "=\(.*\)"/\1/p' "$here/Cargo.toml")"
installed="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$pinned" != "$installed" ]; then
  echo "wasm-bindgen-cli $installed does not match the crate's wasm-bindgen $pinned" >&2
  echo "  cargo install wasm-bindgen-cli --version $pinned --locked" >&2
  exit 1
fi

# Development is incremental; optimized delivery is explicit. Both reuse Cargo's target.
cargo build --locked --manifest-path "$root/Cargo.toml" -p legal-citations-wasm \
  --target wasm32-unknown-unknown --jobs 1 "${build_args[@]}"

wasm="$target_dir/wasm32-unknown-unknown/$profile/legal_citations_wasm.wasm"
# Bind the selected profile even when another profile's package is newer.
rm -rf "$here/js/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$here/js/pkg" "$wasm"
rm -f "$here/js/pkg/.gitignore" "$here/js/pkg/package.json"
rm -rf "$here/js/bindings"

cp -R "$root/crates/legal-citations/bindings" "$here/js/bindings"
cp "$root/LICENSE" "$here/js/LICENSE"
cp "$root/NOTICE" "$here/js/NOTICE"
cp "$root/README.md" "$here/js/README.md"
cp "$root/crates/legal-citations/registry/source-canlii-routes.json" "$here/js/source-canlii-routes.json"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
(cd "$here/js" && npm pkg set version="$version" >/dev/null)
ls -l "$here/js/pkg"
