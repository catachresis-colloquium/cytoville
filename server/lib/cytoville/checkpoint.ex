# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Checkpoint do
  @moduledoc "Private, atomic session checkpoints, including native memory and resume credentials."
  defp path(id) do
    root = Application.fetch_env!(:cytoville_server, :checkpoint_dir)
    File.mkdir_p!(root)
    File.chmod!(root, 0o700)
    Path.join(root, Base.encode16(:crypto.hash(:sha256, id), case: :lower) <> ".json")
  end

  def save(state) do
    destination = path(state.id)
    temporary = destination <> ".tmp"

    record = %{
      "id" => state.id,
      "data" => state.data,
      "invite" => state.invite,
      "tokens" => Map.new(state.tokens, fn {k, v} -> {Integer.to_string(k), v} end),
      "joined" => state.joined,
      "expires" =>
        System.system_time(:millisecond) +
          Application.fetch_env!(:cytoville_server, :session_ttl_ms)
    }

    {:ok, file} = :file.open(String.to_charlist(temporary), [:write, :binary, :raw])

    try do
      File.chmod!(temporary, 0o600)
      :ok = :file.write(file, :json.encode(record))
      :ok = :file.sync(file)
    after
      :file.close(file)
    end

    File.rename!(temporary, destination)
    state
  end

  def load(id) do
    with {:ok, bytes} <- File.read(path(id)),
         record <- :json.decode(bytes),
         true <- record["id"] == id and record["expires"] > System.system_time(:millisecond) do
      {:ok, record}
    else
      _ -> {:error, "Unknown or expired session"}
    end
  rescue
    _ -> {:error, "Invalid session checkpoint"}
  end

  def delete(id), do: File.rm(path(id))
end
