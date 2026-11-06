#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Retained path for inherited suite compatibility. Cytoville has no owned-string
# FFI: exercise its real pointer checks, generated layout and linked C API.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
command -v zig >/dev/null || { echo 'SETUP: Zig is required' >&2; exit 2; }
command -v cc >/dev/null || { echo 'SETUP: C compiler is required' >&2; exit 2; }
test -f "$repo/external/evil-weevil/src/interface/ffi/src/main.zig" || {
  echo 'SETUP: initialise the pinned Evil Weevil submodule' >&2; exit 2;
}
zig build --build-file "$repo/src/interface/ffi/build.zig" test install --cache-dir "$work/cache" --prefix "$work"
cc -std=c11 -Wall -Wextra -Werror "$repo/src/interface/ffi/test/api_test.c" "$work/lib/libcytoville_native.a" -o "$work/api-test"
"$work/api-test"