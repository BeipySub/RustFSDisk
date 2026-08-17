#!/usr/bin/env sh
set -eu

repo_dir="${RUSTFS_TRANSFER__DEV_REPO_DIR:-/home/edge/RustFSDisk-dev}"
binary="$repo_dir/target/debug/rustfs-transfer-rescan"
rescan_source="$repo_dir/crates/edge-backend/src/bin/rescan.rs"

if [ -x "$binary" ] && { [ ! -e "$rescan_source" ] || [ "$binary" -nt "$rescan_source" ]; }; then
    exec "$binary" "$@"
fi

if [ -n "${RUSTFS_TRANSFER__DEV_CARGO:-}" ] && [ -x "$RUSTFS_TRANSFER__DEV_CARGO" ]; then
    cargo_bin="$RUSTFS_TRANSFER__DEV_CARGO"
elif [ -x /home/edge/.cargo/bin/cargo ]; then
    cargo_bin=/home/edge/.cargo/bin/cargo
elif command -v cargo >/dev/null 2>&1; then
    cargo_bin="$(command -v cargo)"
else
    if [ -x "$binary" ]; then
        exec "$binary" "$@"
    fi
    echo "rustfs-transfer dev rescan helper cannot find cargo or $binary" >&2
    exit 127
fi

cd "$repo_dir"
exec "$cargo_bin" run -q -p rustfs-transfer-edge --bin rustfs-transfer-rescan -- "$@"
