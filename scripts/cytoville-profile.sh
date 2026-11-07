#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Bounded, offline redraw sample using actual X11 input. No public listener.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${DISPLAY:?Run in an X11 display or xvfb-run}"
: "${CYTOVILLE_PROFILE_OUTPUT:?Set a directory for timing receipts}"
mkdir -p "$CYTOVILLE_PROFILE_OUTPUT"
out=$(realpath "$CYTOVILLE_PROFILE_OUTPUT")
if [[ -n "${CYTOVILLE_PROFILE_BINARY:-}" ]]; then
  binary=$(realpath "$CYTOVILLE_PROFILE_BINARY")
else
  cargo build -p cytoville-bevy --release --locked
  target=$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)
  binary="$target/release/cytoville-bevy"
fi
scratch=$(mktemp -d)
game_pid=''
cleanup() {
  if [[ -n "$game_pid" ]]; then kill "$game_pid" 2>/dev/null || true; wait "$game_pid" 2>/dev/null || true; fi
  rm -rf -- "$scratch"
}
trap cleanup EXIT
unset WAYLAND_DISPLAY
"$binary" --play --seed 7 --save "$scratch/offline.json" --smoke-seconds 60 --profile "$out/timings.json" > "$out/renderer.log" 2>&1 &
game_pid=$!
window=''
for _ in $(seq 1 150); do
  if window=$(xdotool search --pid "$game_pid" --name '^CYTOVILLE' | head -1); then break; fi
  kill -0 "$game_pid" 2>/dev/null || { tail -40 "$out/renderer.log"; exit 1; }
  sleep .2
done
[[ -n "$window" ]] || { echo 'FAIL: no Bevy window'; exit 1; }
sleep 5
xdotool windowfocus --sync "$window"
for _ in $(seq 1 20); do xdotool key --clearmodifiers --delay 80 F5; sleep .35; done
wait "$game_pid"
game_pid=''
jq -e '.screen_rebuild_cpu_ms.count >= 20 and .frame_interval_ms.count > 0' "$out/timings.json"
printf '%s\n' 'Receipt covers CPU screen construction and frame intervals; inspect renderer.log for adapter and build context.'
