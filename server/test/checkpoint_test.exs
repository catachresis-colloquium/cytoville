# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.CheckpointTest do
  use ExUnit.Case, async: false
  import Bitwise, only: [band: 2]

  setup do
    previous = Application.fetch_env!(:cytoville_server, :checkpoint_dir)
    directory = Path.join(System.tmp_dir!(), "cytoville-checkpoint-test-#{System.unique_integer([:positive])}")
    Application.put_env(:cytoville_server, :checkpoint_dir, directory)
    on_exit(fn ->
      Application.put_env(:cytoville_server, :checkpoint_dir, previous)
      File.rm_rf!(directory)
    end)
    {:ok, directory: directory}
  end

  test "startup never changes an existing operator directory's permissions", %{directory: directory} do
    File.mkdir_p!(directory)
    File.chmod!(directory, 0o755)
    assert_raise RuntimeError, fn -> Cytoville.Checkpoint.initialize!() end
    assert band(File.stat!(directory).mode, 0o777) == 0o755
  end

  test "ordinary operations do not recreate a removed store", %{directory: directory} do
    Cytoville.Checkpoint.initialize!()
    assert band(File.stat!(directory).mode, 0o777) == 0o700
    File.rmdir!(directory)
    assert {:error, _} = Cytoville.Checkpoint.load("missing")
    refute File.exists?(directory)
  end
end
