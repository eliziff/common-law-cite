#!/usr/bin/env bash
# Build the npm package into crates/legal-citations-wasm/js/pkg.
#
# Needs: rustup target wasm32-unknown-unknown, and wasm-bindgen-cli at the exact
# version pinned in Cargo.toml (cargo install wasm-bindgen-cli --version <v>).
# wasm-opt (binaryen) is used when on PATH.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

pinned="$(sed -n 's/^wasm-bindgen = "=\(.*\)"/\1/p' "$here/Cargo.toml")"
installed="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$pinned" != "$installed" ]; then
  echo "wasm-bindgen-cli $installed does not match the crate's wasm-bindgen $pinned" >&2
  echo "  cargo install wasm-bindgen-cli --version $pinned --locked" >&2
  exit 1
fi

(cd "$here/js" && cargo run --manifest-path "$root/Cargo.toml" -p legal-citations --features binding-types --bin export-types)

# Size-oriented profile for this build only; the workspace release profile
# stays tuned for native speed.
CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
CARGO_PROFILE_RELEASE_LTO=true \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
CARGO_PROFILE_RELEASE_PANIC=abort \
  cargo build --manifest-path "$root/Cargo.toml" -p legal-citations-wasm \
    --target wasm32-unknown-unknown --release

rm -rf "$here/js/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$here/js/pkg" \
  "$target_dir/wasm32-unknown-unknown/release/legal_citations_wasm.wasm"
rm -f "$here/js/pkg/.gitignore" "$here/js/pkg/package.json"

if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    -o "$here/js/pkg/legal_citations_wasm_bg.wasm" "$here/js/pkg/legal_citations_wasm_bg.wasm"
fi

cp "$root/LICENSE" "$here/js/LICENSE"
cp "$root/NOTICE" "$here/js/NOTICE"
cp "$root/README.md" "$here/js/README.md"
cp "$root/crates/legal-citations/registry/source-canlii-routes.json" "$here/js/source-canlii-routes.json"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
(cd "$here/js" && npm pkg set version="$version" >/dev/null)
ls -l "$here/js/pkg"
