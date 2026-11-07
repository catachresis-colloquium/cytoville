# SPDX-License-Identifier: AGPL-3.0-or-later
$ErrorActionPreference = 'Stop'
if (!$env:RUNNER_TEMP -or !$env:GITHUB_ENV) {
    throw 'This installer requires the GitHub Actions runner environment.'
}
$toolsDirectory = Join-Path $env:RUNNER_TEMP 'cytoville-native-tools'
New-Item -ItemType Directory -Force -Path $toolsDirectory | Out-Null
$archive = Join-Path $toolsDirectory 'zig-0.16.0.zip'
Invoke-WebRequest 'https://ziglang.org/download/0.16.0/zig-x86_64-windows-0.16.0.zip' -OutFile $archive
$expected = '68659eb5f1e4eb1437a722f1dd889c5a322c9954607f5edcf337bc3684a75a7e'
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'Zig archive checksum mismatch.'
}
Expand-Archive -LiteralPath $archive -DestinationPath $toolsDirectory
$zigExe = Join-Path $toolsDirectory 'zig-x86_64-windows-0.16.0/zig.exe'
"ZIG=$zigExe" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
rustup toolchain install 1.99.1 --profile minimal
if ($LASTEXITCODE -ne 0) { throw 'Rust installation failed.' }
rustup default 1.99.1
if ($LASTEXITCODE -ne 0) { throw 'Rust selection failed.' }
