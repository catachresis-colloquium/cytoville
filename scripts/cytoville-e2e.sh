#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p cytoville-runtime -p cytoville-ums --locked
target="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
scratch="$(mktemp -d)"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true; fi
  rm -rf -- "$scratch"
}
trap cleanup EXIT
"$target/debug/cytoville-ums" init "$scratch/source.json"
jq '.content.SPECIMENS.euglena.name = "UMS end-to-end cell" | .content.SPECIMENS.euglena.blurb = ("x" * 5242880)' "$scratch/source.json" > "$scratch/edited.json"
"$target/debug/cytoville-ums" compile "$scratch/edited.json" "$PWD" "$scratch/package.json"
export CYTOVILLE_WORKER="$target/debug/cytoville-server" CYTOVILLE_PACKAGE="$scratch/package.json"
export CYTOVILLE_PORT=0 CYTOVILLE_READY_FILE="$scratch/port"
export CYTOVILLE_CHECKPOINT_DIR="$scratch/checkpoints"
if [[ "${CYTOVILLE_E2E_TLS:-0}" == 1 ]]; then
  openssl req -x509 -newkey rsa:2048 -sha256 -nodes -days 1 \
    -subj /CN=localhost -addext subjectAltName=DNS:localhost \
    -keyout "$scratch/key.pem" -out "$scratch/cert.pem" 2> "$scratch/certificate.log"
  chmod 600 "$scratch/key.pem"
  export CYTOVILLE_TLS_CERT="$scratch/cert.pem" CYTOVILLE_TLS_KEY="$scratch/key.pem" CYTOVILLE_CA_CERT="$scratch/cert.pem"
fi
(cd server && exec mix run --no-halt) > "$scratch/server.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 300); do
  [[ -s "$scratch/port" ]] && break
  if ! kill -0 "$server_pid" 2>/dev/null; then cat "$scratch/server.log"; exit 1; fi
  sleep 0.1
done
if [[ ! -s "$scratch/port" ]]; then cat "$scratch/server.log"; exit 1; fi
export CYTOVILLE_E2E_SERVER="127.0.0.1:$(cat "$scratch/port")"
if [[ "${CYTOVILLE_E2E_TLS:-0}" == 1 ]]; then
  export CYTOVILLE_E2E_SERVER="tls://localhost:$(cat "$scratch/port")"
fi
cargo test -p cytoville-bevy --test end_to_end --locked ums_to_elixir -- --ignored --nocapture
if [[ "${CYTOVILLE_E2E_TLS:-0}" == 1 ]]; then
  env -u CYTOVILLE_CA_CERT cargo test -p cytoville-bevy --test end_to_end --locked rejects_untrusted_tls -- --ignored --nocapture
fi
