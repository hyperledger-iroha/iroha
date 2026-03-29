# Hyperledger Iroha C# SDK

Preview `.NET 8` SDK for Hyperledger Iroha.

## Scope

This initial slice provides the foundation needed for a usable managed SDK:

- canonical account-address parsing and I105 rendering
- Norm v1 domain normalization helpers
- managed BLAKE2b-256/Iroha hash and Norito framing primitives used by the SDK's transaction path
- canonical Torii request signing headers
- a `LedgerClient` plus `TransactionBuilder` that can build, sign, and submit canonical asset `Transfer`, `Mint`, and `Burn` transactions with deterministic hashes and pipeline-status polling
- typed Torii runtime and account-query models for capabilities, ABI, account pages, explorer QR snapshots, asset balances, transaction summaries, permissions, identifier policy listing, identifier resolution, reverse alias lookup, account and contract alias resolution, alias-index lookup, account onboarding, faucet puzzle and claim flows, multisig onboarding, UAID portfolio reads, and space-directory bindings and manifest inventory reads
- a managed faucet PoW solver for `scrypt-leading-zero-bits-v1`, plus `ToriiClient` helpers that can fetch the current puzzle and prepare or submit a faucet claim for an account id
- `ToriiApiException` for non-success HTTP responses, preserving status code, request URI, and response body
- `IrohaClient`, `LedgerClient`, and `ToriiClient` entry points, with raw JSON helpers still available for uncovered endpoints
- fixture-backed unit tests against the repo's canonical address vectors

Broader signed query coverage, streaming, Connect, offline, Nexus, and SoraFS parity is still planned.

## Live Testnet Smoke

The integration project is opt-in and can target the public Taira testnet:

```bash
export PATH="$HOME/.dotnet:$PATH"
export IROHA_CSHARP_RUN_LIVE_TESTS=1
export IROHA_CSHARP_TORII_BASE_URL=https://taira.sora.org
cd csharp
dotnet test tests/Hyperledger.Iroha.Sdk.IntegrationTests/Hyperledger.Iroha.Sdk.IntegrationTests.csproj -c Release
```

The live smoke currently probes unauthenticated read endpoints:

- `/v1/node/capabilities`
- `/v1/runtime/abi/active`
- `/v1/accounts`
- `/v1/explorer/accounts/{account_id}/qr`
- `/v1/identifier-policies`
- `/v1/aliases/by_account`
- `/v1/accounts/faucet/puzzle`
- `/v1/space-directory/uaids/{uaid}`
- `/v1/space-directory/uaids/{uaid}/manifests`

## Sample

```csharp
using Hyperledger.Iroha;
using Hyperledger.Iroha.Torii;

using var client = new IrohaClient(new Uri("https://taira.sora.org"));
try
{
    var capabilities = await client.Torii.GetNodeCapabilitiesAsync();
    var accounts = await client.Torii.GetAccountsAsync(limit: 5);
    var aliases = await client.Torii.LookupAliasesByAccountAsync(accounts.Items[0].Id);
    var faucetPuzzle = await client.Torii.GetAccountFaucetPuzzleAsync();

    Console.WriteLine($"ABI version: {capabilities.AbiVersion}");
    Console.WriteLine($"First page size: {accounts.Items.Count}");
    Console.WriteLine($"Alias count for first account: {aliases?.Total ?? 0}");
    Console.WriteLine($"Faucet puzzle difficulty: {faucetPuzzle.DifficultyBits}");

    // Offline transaction building is available through client.Ledger.
    // var seedHex = Environment.GetEnvironmentVariable("IROHA_CSHARP_PRIVATE_KEY_SEED_HEX");
    // if (!string.IsNullOrWhiteSpace(seedHex))
    // {
    //     var signed = client.Ledger
    //         .BuildTransaction("00000042", accounts.Items[0].Id)
    //         .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "1.0000", accounts.Items[0].Id)
    //         .SetCreationTime(DateTimeOffset.UtcNow)
    //         .SetTimeToLiveMilliseconds(5_000)
    //         .SetNonce(1)
    //         .BuildSigned(Convert.FromHexString(seedHex));
    //
    //     Console.WriteLine($"Signed tx hash: {signed.TransactionHashHex}");
    //     // await client.Ledger.SubmitAsync(signed);
    //     // var status = await client.Ledger.WaitForAsync(signed.TransactionHashHex);
    // }
}
catch (ToriiApiException exception)
{
    Console.WriteLine($"Torii status: {(int?)exception.StatusCode}");
    Console.WriteLine(exception.ResponseBody);
}
```

## Layout

- `src/Hyperledger.Iroha.Sdk/` - package source
- `tests/Hyperledger.Iroha.Sdk.Tests/` - unit and fixture-parity tests
- `tests/Hyperledger.Iroha.Sdk.IntegrationTests/` - live-network integration test lane
- `samples/Hyperledger.Iroha.Sdk.Sample/` - minimal sample app

## Current Ledger Coverage

- `TransactionBuilder.TransferAsset(...)`
- `TransactionBuilder.MintAsset(...)`
- `TransactionBuilder.BurnAsset(...)`
- `LedgerClient.SubmitAsync(...)`
- `LedgerClient.SubmitAndWaitAsync(...)`
- `ToriiClient.GetPipelineTransactionStatusAsync(...)`

The managed transaction encoder is deterministic and covered by byte-level golden tests, but it currently stops at asset transfer, mint, and burn. Signed `/query`, richer instruction families, streaming, and the broader parity surfaces are still open work.

## Build

```bash
export PATH="$HOME/.dotnet:$PATH"
cd csharp
dotnet restore Hyperledger.Iroha.Sdk.sln
dotnet build Hyperledger.Iroha.Sdk.sln -c Release
```

## Test

```bash
export PATH="$HOME/.dotnet:$PATH"
cd csharp
dotnet test Hyperledger.Iroha.Sdk.sln -c Release
```

## Pack

```bash
export PATH="$HOME/.dotnet:$PATH"
cd csharp
dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj -c Release
```
