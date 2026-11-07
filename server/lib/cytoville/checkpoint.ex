# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Checkpoint do
  import Bitwise, only: [band: 2]
  @moduledoc "Private, atomic session checkpoints, including native memory and resume credentials."

  defp root, do: Application.fetch_env!(:cytoville_server, :checkpoint_dir)

  def initialize! do
    directory = root()
    File.mkdir_p!(Path.dirname(directory))

    case File.mkdir(directory) do
      :ok ->
        File.chmod!(directory, 0o700)

      {:error, :eexist} ->
        stat = File.lstat!(directory)

        unless stat.type == :directory and band(stat.mode, 0o777) == 0o700 do
          raise "Checkpoint directory must already be a private directory (0700)"
        end

        unless Enum.all?(File.ls!(directory), &Regex.match?(~r/\A[0-9a-f]{64}\.json(?:\.tmp)?\z/, &1)) do
          raise "Checkpoint directory must be dedicated to Cytoville session records"
        end

      {:error, reason} ->
        raise File.Error, reason: reason, action: "create checkpoint directory", path: directory
    end

    :ok
  end

  defp path(id),
    do: Path.join(root(), Base.encode16(:crypto.hash(:sha256, id), case: :lower) <> ".json")

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
         true <- record["id"] == id and is_integer(record["expires"]) do
      if record["expires"] > System.system_time(:millisecond) do
        {:ok, record}
      else
        delete(id)
        {:error, "Unknown or expired session"}
      end
    else
      _ -> {:error, "Unknown or expired session"}
    end
  rescue
    _ -> {:error, "Invalid session checkpoint"}
  end

  def prune_expired do
    for file <- Path.wildcard(Path.join(root(), "*.json")) do
      if expired_file?(file), do: File.rm(file)
    end

    # No session writer exists during application startup. Interrupted writes
    # contain resume credentials too, and are never a recoverable checkpoint.
    for file <- Path.wildcard(Path.join(root(), "*.json.tmp")), do: File.rm(file)
    :ok
  end

  defp expired_file?(file) do
    with {:ok, bytes} <- File.read(file),
         %{"expires" => expires} when is_integer(expires) <- :json.decode(bytes) do
      expires <= System.system_time(:millisecond)
    else
      _ -> true
    end
  rescue
    _ -> true
  end

  def delete(id), do: File.rm(path(id))
end
