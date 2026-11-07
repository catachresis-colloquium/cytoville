# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Listener do
  use GenServer
  def start_link(_), do: GenServer.start_link(__MODULE__, [], name: __MODULE__)
  def port, do: GenServer.call(__MODULE__, :port)
  @impl true
  def init(_) do
    opts = [
      :binary,
      packet: :line,
      packet_size: 65_536,
      buffer: 65_536,
      active: false,
      reuseaddr: true,
      ip: Application.fetch_env!(:cytoville_server, :bind)
    ]

    {:ok, socket} = :gen_tcp.listen(Application.fetch_env!(:cytoville_server, :port), opts)
    {:ok, {_ip, port}} = :inet.sockname(socket)

    if path = System.get_env("CYTOVILLE_READY_FILE"),
      do: File.write!(path, Integer.to_string(port))

    Task.start_link(fn -> accept(socket) end)
    {:ok, %{socket: socket, port: port}}
  end

  @impl true
  def handle_call(:port, _from, state), do: {:reply, state.port, state}

  defp accept(listener) do
    case :gen_tcp.accept(listener) do
      {:ok, socket} ->
        {:ok, pid} =
          Task.Supervisor.start_child(Cytoville.Connections, fn ->
            receive do
              {:socket, ^socket} -> loop(socket, nil, 0, System.monotonic_time(:second))
            end
          end)

        :ok = :gen_tcp.controlling_process(socket, pid)
        send(pid, {:socket, socket})
        accept(listener)

      {:error, :closed} ->
        :ok
    end
  end

  defp loop(socket, auth, count, epoch) do
    case :gen_tcp.recv(socket, 0, 60_000) do
      {:ok, line} ->
        now = System.monotonic_time(:second)
        next_count = if now == epoch, do: count + 1, else: 1

        if next_count > 30 do
          :gen_tcp.close(socket)
        else
          {reply, next_auth} = decode_and_route(line, auth)
          :gen_tcp.send(socket, [:json.encode(reply), "\n"])
          loop(socket, next_auth, next_count, now)
        end

      {:error, _} ->
        :gen_tcp.close(socket)
    end
  end

  defp decode_and_route(line, auth) do
    try do
      route(:json.decode(line), auth)
    rescue
      _ -> {%{"ok" => false, "error" => "Invalid request"}, auth}
    catch
      :exit, _ -> {%{"ok" => false, "error" => "Session worker unavailable"}, nil}
    end
  end

  defp route(%{"op" => "create", "session" => id} = request, nil) when is_binary(id) do
    if Regex.match?(~r/\A[a-zA-Z0-9_-]{1,48}\z/, id) and
         not match?({:ok, _}, Cytoville.Checkpoint.load(id)) and
         DynamicSupervisor.count_children(Cytoville.Sessions).active < 64 do
      config = Map.take(request, ["specimen", "scenario", "difficulty", "seed", "multiplayer"])

      case DynamicSupervisor.start_child(Cytoville.Sessions, {Cytoville.Session, {id, config}}) do
        {:ok, pid} -> authenticate(Cytoville.Session.call(pid, :created), pid)
        _ -> {%{"ok" => false, "error" => "Session unavailable or invalid configuration"}, nil}
      end
    else
      {%{"ok" => false, "error" => "Invalid session name or server full"}, nil}
    end
  end

  defp route(%{"op" => op, "session" => id, "key" => key}, nil)
       when op in ["join", "resume"] and is_binary(id) and is_binary(key) do
    case Cytoville.Session.lookup(id) do
      {:ok, pid} ->
        authenticate(
          Cytoville.Session.call(pid, {if(op == "join", do: :join, else: :resume), key}),
          pid
        )

      {:error, error} ->
        {%{"ok" => false, "error" => error}, nil}
    end
  end

  defp route(%{"op" => "state"}, {pid, key} = auth),
    do: {reply(Cytoville.Session.call(pid, {:state, key})), auth}

  defp route(
         %{"op" => "command", "expected_revision" => revision, "command" => command},
         {pid, key} = auth
       )
       when is_integer(revision) and revision >= 0 and is_map(command) do
    {reply(Cytoville.Session.call(pid, {:command, key, revision, command})), auth}
  end

  defp route(_, auth),
    do: {%{"ok" => false, "error" => "Unsupported or unauthenticated request"}, auth}

  defp authenticate({:ok, data}, pid),
    do: {%{"ok" => true, "data" => data}, {pid, data["resumeKey"]}}

  defp authenticate({:error, reason}, _), do: {%{"ok" => false, "error" => reason}, nil}
  defp reply({:ok, data}), do: %{"ok" => true, "data" => data}
  defp reply({:error, reason}), do: %{"ok" => false, "error" => reason}
end
