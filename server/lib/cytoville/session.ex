# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Session do
  use GenServer, restart: :temporary

  def start_link({id, config}),
    do:
      GenServer.start_link(__MODULE__, {id, config},
        name: {:via, Registry, {Cytoville.Registry, id}}
      )

  def call(pid, message), do: GenServer.call(pid, message, 15_000)

  def lookup(id, credential) do
    case Registry.lookup(Cytoville.Registry, id) do
      [{pid, _}] ->
        {:ok, pid}

      _ ->
        with {:ok, record} <- Cytoville.Checkpoint.load(id),
             true <- recoverable?(record, credential) do
          case DynamicSupervisor.start_child(
                 Cytoville.Sessions,
                 {__MODULE__, {id, {:restore, record}}}
               ) do
            {:ok, pid} -> {:ok, pid}
            {:error, {:already_started, pid}} -> {:ok, pid}
            _ -> {:error, "Session recovery failed"}
          end
        else
          false -> {:error, "Invalid recovery credential"}
          error -> error
        end
    end
  end

  @impl true
  def init({id, config}) do
    path = Application.fetch_env!(:cytoville_server, :worker)
    if not File.regular?(path), do: raise("Build cytoville-server and set CYTOVILLE_WORKER")

    port =
      Port.open({:spawn_executable, String.to_charlist(path)}, [
        :binary,
        {:line, 8_388_608},
        :exit_status,
        args: []
      ])

    request =
      case config do
        {:restore, record} -> %{"op" => "load", "save" => record["data"]["runtime"]}
        config -> config |> Map.put("op", "new") |> Map.put_new("multiplayer", true)
      end

    case worker(port, request) do
      {:ok, data} ->
        lease = make_ref()

        remaining =
          case config do
            {:restore, record} -> max(1, record["expires"] - System.system_time(:millisecond))
            _ -> ttl()
          end

        timer = Process.send_after(self(), {:expire, lease}, remaining)

        fresh = %{
          id: id,
          lease: lease,
          port: port,
          data: data,
          invite: token(),
          tokens: %{0 => token()},
          joined: false,
          timer: timer,
          checkpointed_at: System.monotonic_time(:millisecond)
        }

        state =
          case config do
            {:restore, record} ->
              %{
                fresh
                | invite: record["invite"],
                  tokens: Map.new(record["tokens"], fn {k, v} -> {String.to_integer(k), v} end),
                  joined: record["joined"]
              }

            _ ->
              fresh
          end

        case config do
          {:restore, _} -> {:ok, state}
          _ -> {:ok, Cytoville.Checkpoint.save(state)}
        end

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
        "inviteKey" =>
          if(state.data["runtime"]["game"]["multiplayer"], do: state.invite, else: nil),
        "data" => state.data
      }}, state}
  end

  def handle_call({:join, invite}, _from, state) do
    cond do
      state.joined or not state.data["runtime"]["game"]["multiplayer"] ->
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
      do: {:reply, {:ok, client_data(state.data)}, touch(state, false)},
      else: {:reply, {:error, "Unauthorized"}, state}
  end

  def handle_call({:command, key, revision, command}, _from, state) do
    case Enum.find(state.tokens, fn {_seat, value} -> value == key end) do
      nil ->
        {:reply, {:error, "Unauthorized"}, state}

      {seat, _} ->
        if not state.joined and state.data["runtime"]["game"]["multiplayer"] do
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
            {:ok, data} -> {:reply, {:ok, client_data(data)}, touch(%{state | data: data})}
            {:error, reason} -> {:reply, {:error, reason}, state}
          end
        end
    end
  end

  @impl true
  def handle_info({:expire, lease}, %{lease: lease} = state) do
    Cytoville.Checkpoint.delete(state.id)
    {:stop, :normal, state}
  end

  def handle_info({:expire, _old_lease}, state), do: {:noreply, state}

  def handle_info({port, {:exit_status, status}}, %{port: port} = state),
    do: {:stop, {:worker_exit, status}, state}

  @impl true
  def terminate(_reason, state) do
    if Port.info(state.port), do: Port.close(state.port)
    :ok
  end

  defp authorized?(state, key), do: key != nil and key in Map.values(state.tokens)

  # Checkpoints retain complete runtime content. Network updates carry only
  # mutable state and the content fingerprint supplied by the Rust worker.
  defp client_data(data),
    do: Map.update!(data, "runtime", &Map.delete(&1, "content"))
  defp recoverable?(record, {:resume, key}), do: key in Map.values(record["tokens"])

  defp recoverable?(record, {:join, key}),
    do: not record["joined"] and key == record["invite"] and key != nil

  defp token, do: Base.url_encode64(:crypto.strong_rand_bytes(24), padding: false)
  defp ttl, do: Application.fetch_env!(:cytoville_server, :session_ttl_ms)

  defp touch(state, durable \\ true) do
    Process.cancel_timer(state.timer)
    lease = make_ref()

    next = %{
      state
      | lease: lease,
        timer: Process.send_after(self(), {:expire, lease}, ttl())
    }

    now = System.monotonic_time(:millisecond)

    if durable or now - state.checkpointed_at >= min(30_000, max(1, div(ttl(), 4))) do
      Cytoville.Checkpoint.save(%{next | checkpointed_at: now})
    else
      next
    end
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
