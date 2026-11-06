# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Session do
  use GenServer, restart: :temporary

  def start_link({id, config}),
    do: GenServer.start_link(__MODULE__, config, name: {:via, Registry, {Cytoville.Registry, id}})

  def call(pid, message), do: GenServer.call(pid, message, 15_000)

  def lookup(id) do
    case Registry.lookup(Cytoville.Registry, id) do
      [{pid, _}] -> {:ok, pid}
      _ -> {:error, "Unknown session"}
    end
  end

  @impl true
  def init(config) do
    path = Application.fetch_env!(:cytoville_server, :worker)
    if not File.regular?(path), do: raise("Build cytoville-server and set CYTOVILLE_WORKER")

    port =
      Port.open({:spawn_executable, String.to_charlist(path)}, [
        :binary,
        {:line, 8_388_608},
        :exit_status,
        args: []
      ])

    request = Map.merge(config, %{"op" => "new", "multiplayer" => true})

    case worker(port, request) do
      {:ok, data} ->
        timer = Process.send_after(self(), :expire, ttl())

        {:ok,
         %{
           port: port,
           data: data,
           invite: token(),
           tokens: %{0 => token()},
           joined: false,
           timer: timer
         }}

      {:error, reason} ->
        {:stop, reason}
    end
  end

  @impl true
  def handle_call(:created, _from, state) do
    {:reply,
     {:ok,
      %{
        "seat" => 0,
        "resumeKey" => state.tokens[0],
        "inviteKey" => state.invite,
        "data" => state.data
      }}, state}
  end

  def handle_call({:join, invite}, _from, state) do
    cond do
      state.joined ->
        {:reply, {:error, "Both seats are occupied; use your resume key"}, state}

      invite != state.invite ->
        {:reply, {:error, "Invalid invitation"}, state}

      true ->
        key = token()
        next = touch(%{state | joined: true, invite: nil, tokens: Map.put(state.tokens, 1, key)})
        {:reply, {:ok, %{"seat" => 1, "resumeKey" => key, "data" => state.data}}, next}
    end
  end

  def handle_call({:resume, key}, _from, state) do
    case Enum.find(state.tokens, fn {_seat, value} -> value == key end) do
      {seat, _} ->
        {:reply, {:ok, %{"seat" => seat, "resumeKey" => key, "data" => state.data}}, touch(state)}

      nil ->
        {:reply, {:error, "Invalid resume key"}, state}
    end
  end

  def handle_call({:state, key}, _from, state) do
    if authorized?(state, key),
      do: {:reply, {:ok, state.data}, touch(state)},
      else: {:reply, {:error, "Unauthorized"}, state}
  end

  def handle_call({:command, key, revision, command}, _from, state) do
    case Enum.find(state.tokens, fn {_seat, value} -> value == key end) do
      nil ->
        {:reply, {:error, "Unauthorized"}, state}

      {seat, _} ->
        if not state.joined do
          {:reply, {:error, "Wait for the second cell to join"}, state}
        else
          # The connection cannot supply its seat. Rust checks the active seat,
          # revision, action costs and encounters; Elixir serializes ownership.
          request = %{
            "op" => "command",
            "seat" => seat,
            "expected_revision" => revision,
            "command" => command
          }

          case worker(state.port, request) do
            {:ok, data} -> {:reply, {:ok, data}, touch(%{state | data: data})}
            {:error, reason} -> {:reply, {:error, reason}, state}
          end
        end
    end
  end

  @impl true
  def handle_info(:expire, state), do: {:stop, :normal, state}

  def handle_info({port, {:exit_status, status}}, %{port: port} = state),
    do: {:stop, {:worker_exit, status}, state}

  @impl true
  def terminate(_reason, state) do
    if Port.info(state.port), do: Port.close(state.port)
    :ok
  end

  defp authorized?(state, key), do: key != nil and key in Map.values(state.tokens)
  defp token, do: Base.url_encode64(:crypto.strong_rand_bytes(24), padding: false)
  defp ttl, do: Application.fetch_env!(:cytoville_server, :session_ttl_ms)

  defp touch(state) do
    Process.cancel_timer(state.timer)
    %{state | timer: Process.send_after(self(), :expire, ttl())}
  end

  defp worker(port, request) do
    Port.command(port, [:json.encode(request), "\n"])

    receive do
      {^port, {:data, {:eol, line}}} ->
        case :json.decode(line) do
          %{"ok" => true, "data" => data} -> {:ok, data}
          %{"error" => reason} -> {:error, reason}
        end

      {^port, {:data, {:noeol, _}}} ->
        exit(:oversized_worker_response)

      {^port, {:exit_status, code}} ->
        exit({:worker_exit, code})
    after
      10_000 -> exit(:worker_timeout)
    end
  end
end
