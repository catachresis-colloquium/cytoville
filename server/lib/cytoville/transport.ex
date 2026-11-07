# SPDX-License-Identifier: AGPL-3.0-or-later
defmodule Cytoville.Transport do
  import Kernel, except: [send: 2]

  def listen(port, options) do
    case {System.get_env("CYTOVILLE_TLS_CERT"), System.get_env("CYTOVILLE_TLS_KEY")} do
      {nil, nil} ->
        wrap(:gen_tcp, :gen_tcp.listen(port, options))

      {cert, key} when is_binary(cert) and is_binary(key) ->
        wrap(
          :ssl,
          :ssl.listen(
            port,
            options ++
              [
                certfile: String.to_charlist(cert),
                keyfile: String.to_charlist(key),
                versions: [:"tlsv1.3", :"tlsv1.2"]
              ]
          )
        )

      _ ->
        {:error, :both_tls_cert_and_key_required}
    end
  end

  def accept({:ssl, s}), do: wrap(:ssl, :ssl.transport_accept(s))
  def accept({:gen_tcp, s}), do: wrap(:gen_tcp, :gen_tcp.accept(s))
  def handshake({:ssl, s}), do: wrap(:ssl, :ssl.handshake(s, 5_000))
  def handshake(socket), do: {:ok, socket}
  def sockname({:gen_tcp, s}), do: :inet.sockname(s)
  def sockname({:ssl, s}), do: :ssl.sockname(s)
  def controlling_process({m, s}, pid), do: m.controlling_process(s, pid)
  def recv({m, s}, length, timeout), do: m.recv(s, length, timeout)
  def send({m, s}, bytes), do: m.send(s, bytes)
  def close({m, s}), do: m.close(s)
  defp wrap(module, {:ok, socket}), do: {:ok, {module, socket}}
  defp wrap(_, error), do: error
end
