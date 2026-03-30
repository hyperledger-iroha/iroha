# Hyperledger Iroha C# SDK

Preview `.NET 8` SDK for Hyperledger Iroha.

## Scope

This initial slice provides the foundation needed for a usable managed SDK:

- canonical account-address parsing and I105 rendering
- Norm v1 domain normalization helpers
- managed BLAKE2b-256/Iroha hash and Norito framing primitives used by the SDK's transaction path
- canonical Torii request signing headers
- a `LedgerClient` plus `TransactionBuilder` that can build, sign, and submit canonical asset/domain/asset-definition/NFT transfer transactions, asset mint/burn transactions, `SetAssetKeyValue`, `RemoveAssetKeyValue`, `SetDomainKeyValue`, `RemoveDomainKeyValue`, `SetAccountKeyValue`, `RemoveAccountKeyValue`, `SetAssetDefinitionKeyValue`, `RemoveAssetDefinitionKeyValue`, `SetNftKeyValue`, `RemoveNftKeyValue`, `SetTriggerKeyValue`, `RemoveTriggerKeyValue`, `MintTriggerRepetitions`, `BurnTriggerRepetitions`, and `ExecuteTrigger` transactions with deterministic hashes and pipeline-status polling
- typed Torii runtime and account-query models for capabilities, ABI, account pages, explorer account/domain/asset inventory pages and details, explorer QR snapshots, explorer asset-definition econometrics and holder snapshots, explorer block/transaction/instruction pages, details, latest snapshots, health, metrics, and instruction contract-view reads, typed contract metadata/code-bytes/instance/state reads, write-side contract deploy/instance-activate/call/multisig propose/approve helpers, read-only contract-view execution under `/v1/contracts/view`, typed verified-source job submit/status helpers, typed contract code-view reads under `/v1/contracts/code/{code_hash}/contract-view`, asset balances, transaction summaries, permissions, identifier policy listing, identifier resolution, reverse alias lookup, account and contract alias resolution, alias-index lookup, account onboarding, faucet puzzle and claim flows, multisig onboarding, UAID portfolio reads, and space-directory bindings and manifest inventory reads
- typed Torii VPN and SoraFS helpers for `/v1/vpn/profile`, signed `/v1/vpn/sessions`, `/v1/sorafs/cid/{cid}`, `/v1/sorafs/denylist/catalog`, `/v1/sorafs/denylist/packs/{pack_id}`, and CID content reads under `/sorafs/cid/{cid}/...`
- low-level `ToriiClient.SubmitSignedQueryAsync(...)`, `OpenEventSseAsync(...)`, and parsed `StreamEventsAsync(...)` helpers plus a managed `SignedQueryBuilder` for the full current singular-query set (`FindExecutorDataModel`, `FindParameters`, `FindAliasesByAccountId`, `FindProofRecordById`, `FindContractManifestByCodeHash`, `FindAbiVersion`, `FindAssetById`, `FindAssetDefinitionById`, `FindTwitterBindingByHash`, `FindDomainEndorsements`, `FindDomainEndorsementPolicy`, `FindDomainCommittee`, `FindDaPinIntentByTicket`, `FindDaPinIntentByManifest`, `FindDaPinIntentByAlias`, `FindDaPinIntentByLaneEpochSequence`, `FindSorafsProviderOwner`, `FindDataspaceNameOwnerById`), a managed `SignedIterableQueryBuilder` for the current fast_dsl iterable subset (`FindDomains`, `FindAccounts`, `FindAssets`, `FindAssetDefinitions`, `FindRepoAgreements`, `FindNfts`, `FindRwas`, `FindTransactions`, `FindRoles`, `FindRoleIds`, `FindPeers`, `FindActiveTriggerIds`, `FindTriggers`, `FindAccountsWithAsset`, `FindPermissionsByAccountId`, `FindRolesByAccountId`, `FindBlocks`, `FindBlockHeaders`, `FindProofRecords`, `FindOfflineAllowances`, `FindOfflineAllowanceByCertificateId`, `FindOfflineToOnlineTransfers`, `FindOfflineToOnlineTransferById`, `FindOfflineCounterSummaries`, `FindOfflineVerdictRevocations`, and cursor `Continue(...)`), and typed `StreamPipelineEventsAsync(...)` / `StreamProofEventsAsync(...)` plus typed explorer block/transaction/instruction SSE projections
- a managed faucet PoW solver for `scrypt-leading-zero-bits-v1`, plus `ToriiClient` helpers that can fetch the current puzzle and prepare or submit a faucet claim for an account id
- `ToriiApiException` for non-success HTTP responses, preserving status code, request URI, and response body
- `IrohaClient`, `LedgerClient`, and `ToriiClient` entry points, with raw JSON helpers still available for uncovered endpoints
- fixture-backed unit tests against the repo's canonical address vectors

Broader iterable families beyond the current fast_dsl subset, richer typed event coverage beyond the current pipeline/proof/explorer SSE projections, broader contract admin/lifecycle helpers beyond deploy/activate/call/multisig plus verified-source job helpers, Connect, offline, Nexus, and the remaining parity work are still planned.

`CreateVpnSessionAsync(...)` and `DeleteVpnSessionAsync(...)` call signed Torii
routes, so set `ToriiClientOptions.CanonicalRequestCredentials` before using
those helpers.

For SoraFS content, use `OpenSoraFsCidContentAsync(...)` when you want the raw
HTTP response/stream, or `GetSoraFsCidContentAsync(...)` when buffering the
payload into memory is acceptable.

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
- `/v1/explorer/accounts`, `/v1/explorer/domains`, `/v1/explorer/asset-definitions`, `/v1/explorer/assets`, `/v1/explorer/nfts`, and `/v1/explorer/rwas` with first-item detail reads when present
- `/v1/contracts/instances/{ns}` using `universal` by default, with `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view` when a code hash is available from that namespace or the override env var below
- `/v1/identifier-policies`
- `/v1/vpn/profile`
- `/v1/sorafs/denylist/catalog`
- `/v1/sorafs/denylist/packs/{pack_id}` when the catalog is non-empty
- `/v1/aliases/by_account`
- `/v1/accounts/faucet/puzzle`
- `/v1/space-directory/uaids/{uaid}`
- `/v1/space-directory/uaids/{uaid}/manifests`

Optional live-smoke environment variables:

- `IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE` to override the default contract-instance namespace (`universal`)
- `IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH` to override the code hash used for `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view`
- `IROHA_CSHARP_SMOKE_SORAFS_CID` and optional `IROHA_CSHARP_SMOKE_SORAFS_PATH` to also probe `/v1/sorafs/cid/{cid}` plus `/sorafs/cid/{cid}/...`
- `IROHA_CSHARP_CANONICAL_ACCOUNT_ID` plus `IROHA_CSHARP_PRIVATE_KEY_SEED_HEX` to also create and delete a signed VPN session

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
- `TransactionBuilder.SetAssetKeyValue(...)`
- `TransactionBuilder.RemoveAssetKeyValue(...)`
- `TransactionBuilder.SetDomainKeyValue(...)`
- `TransactionBuilder.RemoveDomainKeyValue(...)`
- `TransactionBuilder.SetAccountKeyValue(...)`
- `TransactionBuilder.RemoveAccountKeyValue(...)`
- `TransactionBuilder.SetAssetDefinitionKeyValue(...)`
- `TransactionBuilder.RemoveAssetDefinitionKeyValue(...)`
- `SignedQueryBuilder.FindExecutorDataModel(...)`
- `SignedQueryBuilder.FindParameters(...)`
- `SignedQueryBuilder.FindAbiVersion(...)`
- `SignedQueryBuilder.FindAliasesByAccountId(...)`
- `SignedQueryBuilder.FindProofRecordById(...)`
- `SignedQueryBuilder.FindAssetById(...)`
- `SignedQueryBuilder.FindAssetDefinitionById(...)`
- `SignedQueryBuilder.FindContractManifestByCodeHash(...)`
- `SignedQueryBuilder.FindTwitterBindingByHash(...)`
- `SignedQueryBuilder.FindDomainEndorsements(...)`
- `SignedQueryBuilder.FindDomainEndorsementPolicy(...)`
- `SignedQueryBuilder.FindDomainCommittee(...)`
- `SignedQueryBuilder.FindDaPinIntentByTicket(...)`
- `SignedQueryBuilder.FindDaPinIntentByManifest(...)`
- `SignedQueryBuilder.FindDaPinIntentByAlias(...)`
- `SignedQueryBuilder.FindDaPinIntentByLaneEpochSequence(...)`
- `SignedQueryBuilder.FindSorafsProviderOwner(...)`
- `SignedQueryBuilder.FindDataspaceNameOwnerById(...)`
- `SignedIterableQueryBuilder.FindDomains(...)`
- `SignedIterableQueryBuilder.FindAccounts(...)`
- `SignedIterableQueryBuilder.FindAssets(...)`
- `SignedIterableQueryBuilder.FindAssetDefinitions(...)`
- `SignedIterableQueryBuilder.FindRepoAgreements(...)`
- `SignedIterableQueryBuilder.FindNfts(...)`
- `SignedIterableQueryBuilder.FindRwas(...)`
- `SignedIterableQueryBuilder.FindTransactions(...)`
- `SignedIterableQueryBuilder.FindRoles(...)`
- `SignedIterableQueryBuilder.FindRoleIds(...)`
- `SignedIterableQueryBuilder.FindPeers(...)`
- `SignedIterableQueryBuilder.FindActiveTriggerIds(...)`
- `SignedIterableQueryBuilder.FindTriggers(...)`
- `SignedIterableQueryBuilder.FindAccountsWithAsset(...)`
- `SignedIterableQueryBuilder.FindPermissionsByAccountId(...)`
- `SignedIterableQueryBuilder.FindRolesByAccountId(...)`
- `SignedIterableQueryBuilder.FindBlocks(...)`
- `SignedIterableQueryBuilder.FindBlockHeaders(...)`
- `SignedIterableQueryBuilder.FindProofRecords(...)`
- `SignedIterableQueryBuilder.FindOfflineAllowances(...)`
- `SignedIterableQueryBuilder.FindOfflineAllowanceByCertificateId(...)`
- `SignedIterableQueryBuilder.FindOfflineToOnlineTransfers(...)`
- `SignedIterableQueryBuilder.FindOfflineToOnlineTransferById(...)`
- `SignedIterableQueryBuilder.FindOfflineCounterSummaries(...)`
- `SignedIterableQueryBuilder.FindOfflineVerdictRevocations(...)`
- `SignedIterableQueryBuilder.Continue(...)`
- `LedgerClient.SubmitAsync(...)`
- `LedgerClient.SubmitAndWaitAsync(...)`
- `ToriiClient.GetPipelineTransactionStatusAsync(...)`
- `ToriiClient.SubmitSignedQueryAsync(...)`
- `ToriiClient.OpenEventSseAsync(...)`
- `ToriiClient.StreamEventsAsync(...)`
- `ToriiClient.StreamPipelineEventsAsync(...)`
- `ToriiClient.StreamProofEventsAsync(...)`
- `ToriiClient.OpenExplorerBlocksSseAsync(...)`
- `ToriiClient.StreamExplorerBlocksAsync(...)`
- `ToriiClient.GetExplorerBlocksAsync(...)`
- `ToriiClient.GetExplorerBlockAsync(...)`
- `ToriiClient.OpenExplorerTransactionsSseAsync(...)`
- `ToriiClient.StreamExplorerTransactionsAsync(...)`
- `ToriiClient.GetExplorerTransactionsAsync(...)`
- `ToriiClient.GetExplorerLatestTransactionsAsync(...)`
- `ToriiClient.GetExplorerTransactionAsync(...)`
- `ToriiClient.OpenExplorerInstructionsSseAsync(...)`
- `ToriiClient.StreamExplorerInstructionsAsync(...)`
- `ToriiClient.GetExplorerInstructionsAsync(...)`
- `ToriiClient.GetExplorerLatestInstructionsAsync(...)`
- `ToriiClient.GetExplorerInstructionAsync(...)`
- `ToriiClient.GetExplorerHealthAsync(...)`
- `ToriiClient.GetExplorerMetricsAsync(...)`

The managed transaction encoder is deterministic and now covers the current asset quantity plus domain, asset, account, and asset-definition metadata slice. The Torii client can also parse generic SSE frames, project the common pipeline, proof, and explorer block/transaction/instruction streams into typed models, read the core explorer JSON endpoints including latest/health/metrics snapshots with typed DTOs, and build/sign the current singular set plus the first fast_dsl iterable-query subset, but broader iterable families, richer instruction families beyond that slice, broader typed event families, and the broader parity surfaces are still open work.

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
