# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Application do
  use Application

  def start(_type, _args) do
    Cytoville.Checkpoint.prune_expired()

    children = [
      {Registry, keys: :unique, name: Cytoville.Registry},
      {DynamicSupervisor, strategy: :one_for_one, name: Cytoville.Sessions, max_children: 64},
      {Task.Supervisor, name: Cytoville.Connections},
      Cytoville.Listener
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: Cytoville.Supervisor)
  end
end
