# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule CytovilleServer.MixProject do
  use Mix.Project

  def project,
    do: [
      app: :cytoville_server,
      version: "0.1.0",
      elixir: "~> 1.19",
      start_permanent: Mix.env() == :prod,
      deps: []
    ]

  def application,
    do: [extra_applications: [:logger, :crypto, :ssl], mod: {Cytoville.Application, []}]
end
