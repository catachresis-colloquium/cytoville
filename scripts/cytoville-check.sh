#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
git submodule update --init --recursive
idris2 --build src/interface/abi.ipkg
src/interface/build/exec/cytoville-abi-gen
git diff --exit-code -- src/interface/generated
native_dir="${CARGO_TARGET_DIR:-$PWD/target}/native-check"
zig build --build-file src/interface/ffi/build.zig test install --cache-dir "$native_dir/cache" --prefix "$native_dir"
cc -std=c11 -Wall -Wextra -Werror src/interface/ffi/test/api_test.c "$native_dir/lib/libcytoville_native.a" -o "$native_dir/api-test"
"$native_dir/api-test"
cargo fmt --all --check
cargo test --locked
cargo test -p cytoville-bevy --locked
cargo build -p cytoville-runtime --locked
worker="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/debug/cytoville-server"
(cd server && mix format --check-formatted && CYTOVILLE_WORKER="$worker" mix test)
bash scripts/cytoville-e2e.sh
