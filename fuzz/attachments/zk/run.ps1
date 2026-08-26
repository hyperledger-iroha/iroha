#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Require-Cmd($name) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
    Write-Error "Missing dependency: $name"; exit 1
  }
}

Require-Cmd iroha

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ConfigArg = @()
if ($env:CLI_CONFIG) { $ConfigArg = @('--config', $env:CLI_CONFIG) }
$ElectionId = if ($env:ELECTION_ID) { $env:ELECTION_ID } else { 'demo-election-1' }

Write-Host "[0/5] Checking Torii health (server version)"
try {
  iroha @ConfigArg Version | Out-Null
} catch {
  Write-Error "Torii health check failed. Verify config and that the server is running."; exit 1
}

Write-Host "[1/5] VK register/update with the configured signer"
iroha @ConfigArg zk vk register --json (Join-Path $ScriptDir 'vk_register.json')
iroha @ConfigArg zk vk update --json (Join-Path $ScriptDir 'vk_update.json')
iroha @ConfigArg zk vk get --backend 'halo2/ipa' --name 'vk_add'

Write-Host "[2/5] Upload JSON attachment"
$attMetaJson = iroha @ConfigArg zk attachments upload --file (Join-Path $ScriptDir 'proof.json') --content-type application/json
Write-Host ($attMetaJson | Out-String)

Write-Host "[3/5] Upload minimal ZK1 Norito envelope"
$zk1b64 = Get-Content -Raw -Path (Join-Path $ScriptDir 'zk1_min.b64')
[IO.File]::WriteAllBytes((Join-Path $ScriptDir 'zk1_min.bin'), [Convert]::FromBase64String($zk1b64))
iroha @ConfigArg zk attachments upload --file (Join-Path $ScriptDir 'zk1_min.bin') --content-type application/x-norito | Out-Null

Write-Host "[4/5] List attachments"
iroha @ConfigArg zk attachments list | Out-String | Write-Host

Write-Host "[5/5] Vote tally helper"
iroha @ConfigArg zk vote tally --election-id $ElectionId | Out-String | Write-Host

Write-Host "Done."
