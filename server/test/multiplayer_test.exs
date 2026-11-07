# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.MultiplayerTest do
  use ExUnit.Case, async: false

  defp socket do
    {:ok, s} =
      :gen_tcp.connect({127, 0, 0, 1}, Cytoville.Listener.port(), [
        :binary,
        packet: :line,
        packet_size: 8_388_608,
        buffer: 8_388_608,
        active: false
      ])

    s
  end

  defp request(s, body) do
    :ok = :gen_tcp.send(s, [:json.encode(body), "\n"])
    {:ok, line} = :gen_tcp.recv(s, 0, 15_000)
    assert String.ends_with?(line, "\n")
    :json.decode(line)
  end

  test "native rival memory and causal trace survive session loss" do
    host = socket()
    id = "native-#{System.unique_integer([:positive])}"

    created =
      request(host, %{
        "op" => "create",
        "session" => id,
        "specimen" => "euglena",
        "scenario" => "fresh-daughter",
        "difficulty" => "standard",
        "seed" => 7,
        "multiplayer" => false
      })

    assert created["ok"]

    next =
      request(host, %{
        "op" => "command",
        "expected_revision" => 0,
        "command" => %{"type" => "end_turn"}
      })

    assert next["ok"]
    assert next["data"]["nativeDecisions"] > 0
    refute Enum.empty?(next["data"]["runtime"]["trace"]["events"])
    :ok = Application.stop(:cytoville_server)
    {:ok, _} = Application.ensure_all_started(:cytoville_server)

    resumed = socket()

    restored =
      request(resumed, %{"op" => "resume", "session" => id, "key" => created["data"]["resumeKey"]})

    assert restored["ok"]
    assert restored["data"]["data"]["runtime"] == next["data"]["runtime"]
    rejected = socket()

    refute request(rejected, %{
             "op" => "join",
             "session" => id,
             "key" => created["data"]["inviteKey"]
           })["ok"]

    Enum.each([host, resumed, rejected], &:gen_tcp.close/1)
  end

  test "two real clients share Rust truth, enforce seats, reject replay, and reconnect" do
    host = socket()
    id = "test-#{System.unique_integer([:positive])}"

    created =
      request(host, %{
        "op" => "create",
        "session" => id,
        "specimen" => "euglena",
        "scenario" => "fresh-daughter",
        "difficulty" => "standard",
        "seed" => 7
      })

    assert created["ok"]
    assert map_size(created["data"]["data"]["content"]["SPECIMENS"]) == 7
    guest = socket()
    refute request(guest, %{"op" => "join", "session" => id, "key" => "wrong"})["ok"]

    joined =
      request(guest, %{"op" => "join", "session" => id, "key" => created["data"]["inviteKey"]})

    assert joined["ok"]
    assert joined["data"]["seat"] == 1

    end_turn = %{
      "op" => "command",
      "expected_revision" => 0,
      "command" => %{"type" => "end_turn"}
    }

    refute request(guest, end_turn)["ok"]
    advanced = request(host, end_turn)
    assert advanced["ok"]
    assert advanced["data"]["game"]["active"] == 1
    refute request(host, end_turn)["ok"]
    state = request(guest, %{"op" => "state"})
    assert state["data"]["game"] == advanced["data"]["game"]
    :gen_tcp.close(guest)
    resumed = socket()

    assert request(resumed, %{
             "op" => "resume",
             "session" => id,
             "key" => joined["data"]["resumeKey"]
           })["ok"]

    next = request(resumed, %{end_turn | "expected_revision" => 1})
    assert next["ok"]
    assert next["data"]["game"]["turn"] == 2
    assert request(host, %{"op" => "state"})["data"]["game"] == next["data"]["game"]
    {:ok, pid} = Cytoville.Session.lookup(id)
    monitor = Process.monitor(pid)
    Process.exit(pid, :kill)
    assert_receive {:DOWN, ^monitor, :process, ^pid, :killed}
    recovered = socket()

    recovery =
      request(recovered, %{
        "op" => "resume",
        "session" => id,
        "key" => joined["data"]["resumeKey"]
      })

    assert recovery["ok"]
    assert recovery["data"]["data"]["runtime"] == next["data"]["runtime"]
    :gen_tcp.close(recovered)
    :gen_tcp.close(host)
    :gen_tcp.close(resumed)
  end
end
