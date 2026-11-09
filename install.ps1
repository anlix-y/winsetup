# winsetup: download the latest release and run it.
#
# GUI:
#   irm https://raw.githubusercontent.com/anlix-y/winsetup/main/install.ps1 | iex
# Apply a config right away (console mode, no GUI):
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/anlix-y/winsetup/main/install.ps1))) https://example.com/config.toml
#
# Messages are in English on purpose: Windows PowerShell 5.1 may garble non-ASCII text of a script run via iex.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # the progress bar makes Invoke-WebRequest many times slower
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$url = 'https://github.com/anlix-y/winsetup/releases/latest/download/winsetup.exe'
$exe = Join-Path $env:TEMP 'winsetup.exe'

Write-Host 'Downloading winsetup...'
Invoke-WebRequest -Uri $url -OutFile $exe -UseBasicParsing
# SHA256 check disabled (by decision): the exe is run without verifying its hash.

Write-Host 'Starting winsetup...'
& $exe @args
