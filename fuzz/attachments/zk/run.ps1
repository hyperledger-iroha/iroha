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

Write-Host "[0/7] Checking Torii health (server version)"
try {
  iroha @ConfigArg Version | Out-Null
} catch {
  Write-Error "Torii health check failed. Verify config and that the server is running."; exit 1
}

Write-Host "[1/7] VK register/update/deprecate (requires AUTHORITY and PRIVATE_KEY)"
if ($env:AUTHORITY -and $env:PRIVATE_KEY) {
  $tmp = New-Item -ItemType Directory -Path ([System.IO.Path]::GetTempPath()) -Name ("zk_" + [System.Guid]::NewGuid())
  try {
    # Register
    $reg = Get-Content -Raw -Path (Join-Path $ScriptDir 'vk_register.json') | ConvertFrom-Json
    $reg.authority = $env:AUTHORITY
    $reg.private_key = $env:PRIVATE_KEY
    $reg | ConvertTo-Json -Depth 8 | Set-Content -NoNewline -Path (Join-Path $tmp.FullName 'vk_register.eff.json')
    iroha @ConfigArg zk vk register --json (Join-Path $tmp.FullName 'vk_register.eff.json')
    # Update
    $upd = Get-Content -Raw -Path (Join-Path $ScriptDir 'vk_update.json') | ConvertFrom-Json
    $upd.authority = $env:AUTHORITY
    $upd.private_key = $env:PRIVATE_KEY
    $upd | ConvertTo-Json -Depth 8 | Set-Content -NoNewline -Path (Join-Path $tmp.FullName 'vk_update.eff.json')
    iroha @ConfigArg zk vk update --json (Join-Path $tmp.FullName 'vk_update.eff.json')
    # Deprecate
    $dep = Get-Content -Raw -Path (Join-Path $ScriptDir 'vk_deprecate.json') | ConvertFrom-Json
    $dep.authority = $env:AUTHORITY
    $dep.private_key = $env:PRIVATE_KEY
    $dep | ConvertTo-Json -Depth 8 | Set-Content -NoNewline -Path (Join-Path $tmp.FullName 'vk_deprecate.eff.json')
    iroha @ConfigArg zk vk deprecate --json (Join-Path $tmp.FullName 'vk_deprecate.eff.json')
    # Read
    iroha @ConfigArg zk vk get --backend 'halo2/ipa' --name 'vk_add'
  } finally {
    Remove-Item -Recurse -Force $tmp.FullName
  }
} else {
  Write-Host "  Skipping VK ops: set AUTHORITY and PRIVATE_KEY env vars to enable."
}

Write-Host "[2/7] Upload JSON attachment"
$attMetaJson = iroha @ConfigArg zk attachments upload --file (Join-Path $ScriptDir 'proof.json') --content-type application/json
$attMeta = $attMetaJson | ConvertFrom-Json
Write-Host ($attMetaJson | Out-String)
$attId = $attMeta.id

Write-Host "[3/7] Upload minimal ZK1 Norito envelope"
$zk1b64 = Get-Content -Raw -Path (Join-Path $ScriptDir 'zk1_min.b64')
[IO.File]::WriteAllBytes((Join-Path $ScriptDir 'zk1_min.bin'), [Convert]::FromBase64String($zk1b64))
iroha @ConfigArg zk attachments upload --file (Join-Path $ScriptDir 'zk1_min.bin') --content-type application/x-norito | Out-Null

Write-Host "[4/7] List attachments"
iroha @ConfigArg zk attachments list | Out-String | Write-Host

Write-Host "[5/7] List prover reports"
iroha @ConfigArg zk prover reports list | Out-String | Write-Host

Write-Host "[6/7] Get a single prover report (first if available)"
$firstRep = (iroha @ConfigArg zk prover reports list | ConvertFrom-Json)[0].id
if ($firstRep) {
  iroha @ConfigArg zk prover reports get --id $firstRep | Out-String | Write-Host
} else {
  Write-Host "  No reports yet"
}

Write-Host "[7/7] Vote tally helper"
iroha @ConfigArg zk vote tally --election-id $ElectionId | Out-String | Write-Host

Write-Host "Done."
