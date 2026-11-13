#!/bin/sh
set -eu

workspace=$(cd "$PROJECT_DIR/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$workspace/src-tauri/target/ios-native-core"
export TAURI_ENV_PLATFORM=ios
export TAURI_ENV_TARGET_TRIPLE="$LISTENER_RUST_TARGET"

set -- rustc --locked --manifest-path "$workspace/src-tauri/Cargo.toml" \
    --lib --crate-type staticlib --target "$LISTENER_RUST_TARGET" --features native-ios
if [ "$CONFIGURATION" = Release ]; then
    set -- "$@" --release
fi

cargo "$@"
