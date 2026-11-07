# SPDX-License-Identifier: AGPL-3.0-or-later
ExUnit.start()

checkpoint_dir =
  Path.join(
    System.tmp_dir!(),
    "cytoville-test-#{System.pid()}-#{System.unique_integer([:positive])}"
  )

Application.put_env(:cytoville_server, :checkpoint_dir, checkpoint_dir)
ExUnit.after_suite(fn _ -> File.rm_rf!(checkpoint_dir) end)
