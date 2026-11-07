#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Real X11 input against Bevy, with save/checkpoint assertions. Run in Xvfb.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${DISPLAY:?Run under xvfb-run with a 1440x960 screen}"
unset WAYLAND_DISPLAY
out="${CYTOVILLE_ACCEPTANCE_OUTPUT:?Set a directory for screenshots and receipt}"
mkdir -p "$out"
out="$(realpath "$out")"
cargo build -p cytoville-bevy -p cytoville-runtime --locked
target="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
scratch="$(mktemp -d)"
export XDG_RUNTIME_DIR="$scratch/xdg"
mkdir -m 700 "$XDG_RUNTIME_DIR"
game_pid="" server_pid=""
cleanup() {
  for pid in "$game_pid" "$server_pid"; do
    if [[ -n "$pid" ]]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  done
  rm -rf -- "$scratch"
}
trap cleanup EXIT
wait_for() {
  local description=$1; shift
  for _ in $(seq 1 150); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep .2
  done
  echo "FAIL: $description" >&2
  return 1
}
launch() {
  "$target/debug/cytoville-bevy" --seed 7 --smoke-seconds 600 "$@" > "$scratch/game.log" 2>&1 &
  game_pid=$!
  if ! wait_for 'Bevy window' xdotool search --pid "$game_pid" --name '^CYTOVILLE'; then
    # Startup stderr has no private ticket/checkpoint contents. Surface it so
    # missing renderer or window-system prerequisites cannot become opaque.
    tail -60 "$scratch/game.log" >&2
    return 1
  fi
  window="$(xdotool search --pid "$game_pid" --name '^CYTOVILLE' | head -1)"
  xdotool windowfocus --sync "$window"
  sleep 2
}
stop_game() { kill "$game_pid"; wait "$game_pid" || true; game_pid=""; }
click() { xdotool mousemove --sync --window "$window" "$1" "$2"; xdotool mousedown 1; sleep .2; xdotool mouseup 1; sleep .5; }
key() { xdotool windowfocus --sync "$window"; xdotool key --clearmodifiers --delay 150 "$@"; sleep .5; }
shot() {
  if [[ -n "${CYTOVILLE_XWD_FRAMEBUFFER:-}" ]]; then
    magick-im7.q16 "xwd:$CYTOVILLE_XWD_FRAMEBUFFER" "$out/$1.png"
  else
    import -window "$window" "$out/$1.png"
  fi
}
save="$scratch/offline.json"
state() { jq -e "$1" "$save"; }
launch --save "$save"
shot 01-menu
click 448 183 # Amoeba
click 63 490 # Gentle
click 95 551 # Begin
shot 02-begin
click 866 290 # Pattern
wait_for 'selected specimen, difficulty and customisation' state '.game.actors[0].specimen == "amoeba" and .game.actors[0].atp == 102 and .game.looks[0].pattern == "spots"'
click 1040 290 # Rename
xdotool type --clearmodifiers --delay 80 Acceptance
key Return
wait_for 'rename committed through keyboard' state '.game.actors[0].name == "AmoebaAcceptance"'
shot 03-customisation
click 140 348 # University
wait_for 'travel cost' state '.game.actors[0].atp == 90'
before="$(jq -c .game "$save")"
click 1085 294 # Locked tier two
[[ "$(jq -c .game "$save")" == "$before" ]] || { echo 'FAIL: disabled course changed game state'; exit 1; }
click 1085 232 # Basal metabolism
wait_for 'course cost and progress' state '.game.actors[0] | .atp == 78 and .biomass == 100 and .courseProgress.bc1 == 1'
shot 04-study
click 938 548 # Next course page
shot 05-course-pagination
key space
wait_for 'cycle and native rival advanced' state '.game.turn == 2 and .ai_tick > 0 and (.trace.events | length) > 0'
shot 06-cycle-and-encounter
click 1100 229 # Repair UV encounter
wait_for 'encounter resolved' state '.game.actors[0] | .atp == 78 and .stress == 5'
key F5
before="$(jq -c . "$save")"
stop_game
launch --save "$save"
key F9
key F5 # Serialize the loaded in-memory runtime before comparing it.
shot 07-restored
[[ "$(jq -c . "$save")" == "$before" ]] || { echo 'FAIL: loading changed saved runtime'; exit 1; }
# Prove the restored UI accepts commands, rather than merely retaining a file.
click 140 348
wait_for 'restored game remains playable' state '.game.actors[0].atp == 66'
stop_game

# The graphical client now uses the real Elixir host over verified loopback TLS.
openssl req -x509 -newkey rsa:2048 -sha256 -nodes -days 1 \
  -subj /CN=localhost -addext subjectAltName=DNS:localhost \
  -keyout "$scratch/key.pem" -out "$scratch/cert.pem" 2> "$scratch/certificate.log"
chmod 600 "$scratch/key.pem"
export CYTOVILLE_WORKER="$target/debug/cytoville-server"
export CYTOVILLE_PORT=0 CYTOVILLE_READY_FILE="$scratch/port"
export CYTOVILLE_CHECKPOINT_DIR="$scratch/checkpoints"
export CYTOVILLE_TLS_CERT="$scratch/cert.pem" CYTOVILLE_TLS_KEY="$scratch/key.pem" CYTOVILLE_CA_CERT="$scratch/cert.pem"
(cd server && exec mix run --no-halt) > "$scratch/server.log" 2>&1 &
server_pid=$!
wait_for 'Elixir TLS listener' test -s "$scratch/port"
launch --server "tls://localhost:$(cat "$scratch/port")" --host --native-rival \
  --session graphical-acceptance --connection-file "$scratch/ticket.json"
checkpoint="$scratch/checkpoints/$(printf graphical-acceptance | sha256sum | cut -d' ' -f1).json"
key space
wait_for 'graphical TLS command reached authoritative native runtime' jq -e \
  '.data.runtime | .game.turn == 2 and .ai_tick > 0 and (.trace.events | length) > 0' "$checkpoint"
before="$(jq -c .data.runtime.game "$checkpoint")"
stop_game
launch --resume-file "$scratch/ticket.json" --connection-file "$scratch/resumed-ticket.json"
[[ "$(jq -c .data.runtime.game "$checkpoint")" == "$before" ]] || { echo 'FAIL: resume changed authoritative game'; exit 1; }
# Resume has no invitation key on screen. Never publish tickets, keys or checkpoints.
shot 08-tls-resumed
revision="$(jq .data.runtime.game.revision "$checkpoint")"
click 1100 274 # Resolve the resumed Euglena encounter.
wait_for 'resumed graphical client can still command the host' jq -e \
  ".data.runtime.game.revision > $revision" "$checkpoint"
stop_game
printf '%s\n' 'PASS: real Bevy input, offline save/reload, native rival, loopback TLS command and resume.' \
  'Scope: 1440x960 X11; not a frame-rate, art-quality or accessibility certification.' | tee "$out/receipt.txt"
