#!/usr/bin/env pwsh
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Require-Cmd($name) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
    Write-Error "Missing dependency: $name"; exit 1
  }
}

Require-Cmd iroha

$ConfigArg = @()
if ($env:CLI_CONFIG) { $ConfigArg = @('--config', $env:CLI_CONFIG) }

$FROM = if ($env:FROM) { $env:FROM } else { 'soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ' }
$ASSET_DEFINITION_ID = if ($env:ASSET_DEFINITION_ID) { $env:ASSET_DEFINITION_ID } else { '62Fk4FPcMuLvW5QjDGNF2a4jAmjM' }
$ASSET_ID = if ($env:ASSET_ID) { $env:ASSET_ID } else { "$ASSET_DEFINITION_ID#$FROM" }
$AMOUNT = if ($env:AMOUNT) { [uint64]$env:AMOUNT } else { 1 }
$NOTE_COMMITMENT_HEX = if ($env:NOTE_COMMITMENT_HEX) { $env:NOTE_COMMITMENT_HEX } else { '0000000000000000000000000000000000000000000000000000000000000000' }

Write-Host "[zk-smoke] server version"
iroha @ConfigArg Version | Out-Null

Write-Host "[zk-smoke] register-asset (Hybrid, allow shield/unshield)"
iroha @ConfigArg zk register-asset --asset $ASSET_ID --allow-shield true --allow-unshield true | Out-Null

Write-Host "[zk-smoke] shield $ASSET_ID from $FROM amount=$AMOUNT"
iroha @ConfigArg zk shield --asset $ASSET_ID --from $FROM --amount $AMOUNT --note-commitment $NOTE_COMMITMENT_HEX | Out-Null

Write-Host "[zk-smoke] OK"
