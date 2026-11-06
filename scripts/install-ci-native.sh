#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail
tool_dir="${RUNNER_TEMP:?}/cytoville-tools"
mkdir -p "$tool_dir"
curl --proto '=https' --fail --location https://ziglang.org/download/0.16.0/zig-x86_64-linux-0.16.0.tar.xz -o "$tool_dir/zig.tar.xz"
echo "70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00  $tool_dir/zig.tar.xz" | sha256sum --check --strict
tar -xJf "$tool_dir/zig.tar.xz" -C "$tool_dir"
echo "$tool_dir/zig-x86_64-linux-0.16.0" >> "${GITHUB_PATH:?}"
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libasound2-dev libudev-dev libx11-dev libxrandr-dev libxi-dev libxkbcommon-dev libwayland-dev chezscheme
git init "$tool_dir/Idris2"
git -C "$tool_dir/Idris2" remote add origin https://github.com/idris-lang/Idris2
git -C "$tool_dir/Idris2" fetch --depth=1 origin 15a3e4e70843f7a34100f6470c04b791330788df
git -C "$tool_dir/Idris2" checkout --detach FETCH_HEAD
# Upstream Idris2 bootstrap requires its own Makefile; no game Makefile is used.
make -C "$tool_dir/Idris2" bootstrap SCHEME=scheme
make -C "$tool_dir/Idris2" install PREFIX="$tool_dir/idris2"
echo "$tool_dir/idris2/bin" >> "$GITHUB_PATH"
