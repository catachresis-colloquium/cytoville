# SPDX-License-Identifier: AGPL-3.0-or-later
import Config

config :cytoville_server,
  checkpoint_dir: System.get_env("CYTOVILLE_CHECKPOINT_DIR", Path.expand("../data", __DIR__)),
  port:
    if(config_env() == :test,
      do: 0,
      else: String.to_integer(System.get_env("CYTOVILLE_PORT", "4280"))
    ),
  bind: {127, 0, 0, 1},
  worker:
    System.get_env(
      "CYTOVILLE_WORKER",
      Path.expand("../../target/debug/cytoville-server", __DIR__)
    ),
  session_ttl_ms: 1_800_000
