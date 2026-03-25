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

$AUTHORITY = if ($env:AUTHORITY) { $env:AUTHORITY } else { '6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9' }
$PRIVATE_KEY = if ($env:PRIVATE_KEY) { $env:PRIVATE_KEY } else { 'ed0120...' }
$BACKEND = if ($env:BACKEND) { $env:BACKEND } else { 'halo2/ipa' }
$FROM = if ($env:FROM) { $env:FROM } else { '6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9' }
$ASSET_DEFINITION_ID = if ($env:ASSET_DEFINITION_ID) { $env:ASSET_DEFINITION_ID } else { '62Fk4FPcMuLvW5QjDGNF2a4jAmjM' }
$ASSET_ID = if ($env:ASSET_ID) { $env:ASSET_ID } else { "$ASSET_DEFINITION_ID#$FROM" }
$TO = if ($env:TO) { $env:TO } else { '6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9' }
$AMOUNT = if ($env:AMOUNT) { [uint64]$env:AMOUNT } else { 1000 }
$NOTE_COMMITMENT_HEX = if ($env:NOTE_COMMITMENT_HEX) { $env:NOTE_COMMITMENT_HEX } else { '0000000000000000000000000000000000000000000000000000000000000000' }
$VK_TRANSFER_NAME = if ($env:VK_TRANSFER_NAME) { $env:VK_TRANSFER_NAME } else { 'vk_transfer' }
$VK_UNSHIELD_NAME = if ($env:VK_UNSHIELD_NAME) { $env:VK_UNSHIELD_NAME } else { 'vk_unshield' }
$PROOF_JSON = $env:PROOF_JSON
$RUN_UNSHIELD = if ($env:RUN_UNSHIELD) { $env:RUN_UNSHIELD } else { '0' }

Write-Host "[0/4] Checking Torii health (server version)"
try {
  iroha @ConfigArg Version | Out-Null
} catch {
  Write-Error "Torii health check failed. Verify config and that the server is running."; exit 1
}

Write-Host "[1/4] Register verifying keys (transfer, unshield)"
$regPath = Join-Path $ScriptDir 'vk_register.json'
if (-not (Test-Path $regPath)) {
  Write-Error "Missing $regPath; see corpora under fuzz/attachments/zk"; exit 1
}
$tmp = New-Item -ItemType Directory -Path ([System.IO.Path]::GetTempPath()) -Name ("zk_" + [System.Guid]::NewGuid())
try {
  # Register transfer VK
  $reg = Get-Content -Raw -Path $regPath | ConvertFrom-Json
  $reg.authority = $AUTHORITY
  $reg.private_key = $PRIVATE_KEY
  $reg.backend = $BACKEND
  $reg.name = $VK_TRANSFER_NAME
  $reg | ConvertTo-Json -Depth 8 | Set-Content -NoNewline -Path (Join-Path $tmp.FullName 'vk_transfer.json')
  iroha @ConfigArg zk vk register --json (Join-Path $tmp.FullName 'vk_transfer.json')

  # Register unshield VK
  $reg = Get-Content -Raw -Path $regPath | ConvertFrom-Json
  $reg.authority = $AUTHORITY
  $reg.private_key = $PRIVATE_KEY
  $reg.backend = $BACKEND
  $reg.name = $VK_UNSHIELD_NAME
  $reg | ConvertTo-Json -Depth 8 | Set-Content -NoNewline -Path (Join-Path $tmp.FullName 'vk_unshield.json')
  iroha @ConfigArg zk vk register --json (Join-Path $tmp.FullName 'vk_unshield.json')
} finally {
  Remove-Item -Recurse -Force $tmp.FullName
}

Write-Host "[2/4] Register Hybrid asset with VK bindings"
iroha @ConfigArg zk register-asset --asset $ASSET_ID `
  --allow-shield true --allow-unshield true `
  --vk-transfer ("{0}:{1}" -f $BACKEND, $VK_TRANSFER_NAME) `
  --vk-unshield ("{0}:{1}" -f $BACKEND, $VK_UNSHIELD_NAME)

Write-Host "[3/4] Shield: $FROM -> shielded ($ASSET_ID) amount=$AMOUNT"
iroha @ConfigArg zk shield --asset $ASSET_ID --from $FROM --amount $AMOUNT --note-commitment $NOTE_COMMITMENT_HEX

Write-Host "[4/4] Unshield: shielded -> $TO ($ASSET_ID) amount=$AMOUNT"
if ($RUN_UNSHIELD -eq '1' -and $PROOF_JSON) {
  Write-Host "  Attempting unshield using proof JSON: $PROOF_JSON"
  $null1 = '0000000000000000000000000000000000000000000000000000000000000000'
  $null2 = $null1
  $inputs = "$null1,$null2"
  try {
    iroha @ConfigArg zk unshield --asset $ASSET_ID --to $TO --amount $AMOUNT --inputs $inputs --proof-json $PROOF_JSON | Out-Null
  } catch {
    Write-Warning "  Unshield likely failed due to placeholder proof (expected). Provide a real proof to succeed."
  }
} else {
  Write-Host "  Skipping unshield. Set RUN_UNSHIELD=1 and PROOF_JSON=path to attempt."
}

Write-Host "Done."
