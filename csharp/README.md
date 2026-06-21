# Hyperledger Iroha C# SDK

Preview `.NET 8` SDK for Hyperledger Iroha.

## Scope

This initial slice provides the foundation needed for a usable managed SDK:

- canonical account-address parsing and I105 rendering
- Norm v1 domain normalization helpers
- managed BLAKE2b-256/Iroha hash and Norito framing primitives used by the SDK's transaction path
- canonical Torii request signing headers
- a `LedgerClient` plus `TransactionBuilder` that can build, sign, and submit canonical asset/domain/asset-definition/NFT transfer transactions, asset mint/burn transactions, `SetAssetKeyValue`, `RemoveAssetKeyValue`, `SetDomainKeyValue`, `RemoveDomainKeyValue`, `SetAccountKeyValue`, `RemoveAccountKeyValue`, `SetAssetDefinitionKeyValue`, `RemoveAssetDefinitionKeyValue`, `SetNftKeyValue`, `RemoveNftKeyValue`, `SetTriggerKeyValue`, `RemoveTriggerKeyValue`, `MintTriggerRepetitions`, `BurnTriggerRepetitions`, and `ExecuteTrigger` transactions with deterministic hashes and pipeline-status polling
- typed Torii runtime and account-query models for capabilities, ABI, account pages, explorer account/domain/asset inventory pages and details, explorer QR snapshots, explorer asset-definition econometrics and holder snapshots, explorer block/transaction/instruction pages, details, latest snapshots, health, metrics, and instruction contract-view reads, typed contract metadata/code-bytes/instance/state reads, write-side contract deploy/instance-activate/call/multisig propose/approve helpers, verifying-key registry register/update helpers, read-only contract-view execution under `/v1/contracts/view`, typed verified-source job submit/status helpers, typed contract code-view reads under `/v1/contracts/code/{code_hash}/contract-view`, asset balances, transaction summaries, permissions, identifier policy listing, identifier resolution, reverse alias lookup, account and contract alias resolution, alias-index lookup, account onboarding, faucet puzzle and claim flows, multisig onboarding, UAID portfolio reads, and space-directory bindings and manifest inventory reads
- typed Torii VPN and SoraFS helpers for `/v1/vpn/profile`, signed VPN quote/session/receipt flows under `/v1/vpn/quotes`, `/v1/vpn/sessions`, and `/v1/vpn/receipts`, `/v1/sorafs/cid/{cid}`, `/v1/sorafs/denylist/catalog`, `/v1/sorafs/denylist/packs/{pack_id}`, and CID content reads under `/sorafs/cid/{cid}/...`
- low-level `ToriiClient.SubmitSignedQueryAsync(...)`, `OpenEventSseAsync(...)`, and parsed `StreamEventsAsync(...)` helpers plus a managed `SignedQueryBuilder` for the full current singular-query set (`FindExecutorDataModel`, `FindParameters`, `FindAliasesByAccountId`, `FindProofRecordById`, `FindContractManifestByCodeHash`, `FindAbiVersion`, `FindAssetById`, `FindAssetDefinitionById`, `FindTwitterBindingByHash`, `FindDomainEndorsements`, `FindDomainEndorsementPolicy`, `FindDomainCommittee`, `FindDaPinIntentByTicket`, `FindDaPinIntentByManifest`, `FindDaPinIntentByAlias`, `FindDaPinIntentByLaneEpochSequence`, `FindSorafsProviderOwner`, `FindDataspaceNameOwnerById`), a managed `SignedIterableQueryBuilder` for the current fast_dsl iterable subset (`FindDomains`, `FindAccounts`, `FindAssets`, `FindAssetDefinitions`, `FindRepoAgreements`, `FindNfts`, `FindRwas`, `FindTransactions`, `FindRoles`, `FindRoleIds`, `FindPeers`, `FindActiveTriggerIds`, `FindTriggers`, `FindAccountsWithAsset`, `FindPermissionsByAccountId`, `FindRolesByAccountId`, `FindBlocks`, `FindBlockHeaders`, `FindProofRecords`, and cursor `Continue(...)`), and typed `StreamPipelineEventsAsync(...)` / `StreamProofEventsAsync(...)` plus typed explorer block/transaction/instruction SSE projections
- a managed faucet PoW solver for `scrypt-leading-zero-bits-v1`, plus `ToriiClient` helpers that can fetch the current puzzle and prepare or submit a faucet claim for an account id
- `ToriiApiException` for non-success HTTP responses, preserving status code, request URI, and response body
- native Ethereum mainnet SCCP helpers for execution-provider chain-id
  validation, inbound receipt/source-event evidence, outbound Groth16 calldata,
  typed receipt RLP/MPT proof construction from `eth_getBlockReceipts`, and
  source verifier/source-adapter material hashes bound to Ethereum chain id `1`,
  the source bridge address, and deployed bridge code hash
- `IrohaClient`, `LedgerClient`, and `ToriiClient` entry points, with raw JSON helpers still available for uncovered endpoints
- fixture-backed unit tests against the repo's canonical address vectors

Broader iterable families beyond the current fast_dsl subset, richer typed event coverage beyond the current pipeline/proof/explorer SSE projections, broader contract admin/lifecycle helpers beyond deploy/activate/call/multisig plus verified-source job helpers, Connect, Nexus, and the remaining parity work are still planned.

`CreateVpnQuoteAsync(...)`, `CreateVpnSessionAsync(...)`,
`SubmitVpnReceiptAsync(...)`, and `DeleteVpnSessionAsync(...)` call signed Torii
routes, so set `ToriiClientOptions.CanonicalRequestCredentials` before using
those helpers. Session creation requires the quote id, committed
`OpenVpnLeaseEscrow` transaction hash, and the same metering public key that was
bound into the quote. Session deletion and operator receipt submission return
canonical receipt DTOs with earned/refund XOR fields and any `SettleVpnLease`
instruction skeleton Torii produced.

For SoraFS content, use `OpenSoraFsCidContentAsync(...)` when you want the raw
HTTP response/stream, or `GetSoraFsCidContentAsync(...)` when buffering the
payload into memory is acceptable.

## Offline Cash Lifecycle

Use `OfflineCashLifecycleController` around the app's offline wallet for load
actions. It syncs pending audit receipts before issuing more cash, while local
device-to-device exchange should validate cached setup and avoid fresh
capability fetches.

```csharp
using Hyperledger.Iroha.Offline;

var snapshot = new OfflineCashConfigurationSnapshot(
    ChainId: "00000042",
    AssetDefinitionId: "pkr#sbp",
    OfflinePaymentsEnabled: true,
    IssuerPublicKeyBase64: cachedIssuerPublicKeyBase64,
    NativeBridgeAbiVersion: 7,
    CreatedAtMs: cachedAtMs,
    ExpiresAtMs: expiresAtMs);
snapshot.RequireUsableForOfflineExchange(
    nowMs: currentTimeMs,
    requiredNativeBridgeAbiVersion: 7);

var controller = new OfflineCashLifecycleController(
    offlineWallet,
    auditReceiptSynchronizer);
await controller.LoadAsync("pkr#sbp", "500");

var transports = new OfflineCashTransportCapabilities(
    QrStreaming: true,
    Nfc: appHasHceEntitlement && deviceSupportsNfc
        ? OfflineCashNfcCapability.Available
        : OfflineCashNfcCapability.Unavailable("missing HCE"),
    Nearby: true);
```

UI layers must hide NFC when `SupportedTransportKinds()` omits `nfc`; non-NFC
devices and app builds without HCE should expose QR or Nearby only.

## Verifying Key Registry

`ToriiClient.RegisterVerifyingKeyAsync(...)` and
`UpdateVerifyingKeyAsync(...)` post Torii's `/v1/zk/vk/register` and
`/v1/zk/vk/update` payloads. The client validates production verifier backends,
required signing fields, height ranges, and inline verifier-key commitments
before the HTTP request is sent:

```csharp
await torii.RegisterVerifyingKeyAsync(new ToriiVerifyingKeyRegisterRequest
{
    Authority = "alice",
    PrivateKey = "ed25519:...",
    Backend = "halo2/ipa",
    Name = "vk_main",
    Version = 1,
    CircuitId = "halo2/ipa::transfer_v1",
    PublicInputsSchemaHashHex = new string('a', 64),
    GasScheduleId = "halo2_default",
    VerifyingKeyBytes = new byte[] { 1, 2, 3 },
    Status = "Active",
});
```

## Native Kagemusha Recursive Spend

The optional `Hyperledger.Iroha.Offline.KagemushaRecursiveSpendNative` wrapper
calls the ABI-6 `connect_norito_bridge` recursive spend surface. `IsAvailable()`
requires native bridge ABI 6 or later plus `init`, `append`, both transition-profile
helpers, the append-boundary helper, both lineage-witness helpers, `verify`,
and `redeem` before reporting recursive spend support. Each entry point accepts
raw Norito request archives and returns raw Norito archive bytes; the C# SDK does
not reimplement prover internals.
`TransitionProfileInit(...)` and `TransitionProfileAppend(...)` return the
canonical Reserved-lineage accumulator transition profile as raw Norito
archives for fixture generation and circuit preflight.
`LineageAppendBoundary(...)` derives the compact append-boundary Norito archive
from a full append transition profile with native opening preflight material;
the C# SDK treats the result as opaque verifier material.
Native Kagemusha bridge outputs are rejected if they are empty, null, or larger
than 64 MiB before the wrapper copies them into managed memory.
The append-boundary digest uses the public
`RecursiveSpendLineageAppendBoundaryDomainV1` domain, plus
`RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1` and
`RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1` for chain/asset
and final-root/current-note binding.

Transaction builders expose the same Kagemusha instruction surface without
asking wallet code to reframe native archives. Use
`TransactionInstruction.KagemushaInstructionArchive(...)` or
`KagemushaInstructionArchiveInstruction` for a typed `KagemushaTransfer` or
`RedeemKagemushaRecursive` instruction archive,
`TransactionBuilder.KagemushaInstructionArchive(...)` to add a single archived
instruction, and `TransactionBuilder.KagemushaRecursiveRedeem(...)` to
derive the redeem instruction from a native recursive redeem request before
signing. These builders require valid Norito archives, reject empty, malformed,
tampered, or wrong-type instruction archives, and keep recursive redeem
derivation inside the native bridge.

Use `PreferredMode(...)` to select `recursive_spend_v1` when the complete
ABI-6-or-later native surface is available, otherwise fall back to
`checked_prefold_v1`.
The ABI-7 `recursive_compact_v1` compact-token symbols remain source-stable and
probe `kagemusha-recursive-compact-v1` separately from ABI 6 recursive spend.
Use `BuildPallasOpenEnvelopesArchive(...)` for the current-hop record bundle
and `BuildPreviousProofOpenEnvelopesArchive(...)` for the previous recursive
proof bundle; gate both builders with
`IsPallasOpenEnvelopeBuilderAvailable()`. The returned opaque Pallas opening
archives are native-owned Norito bytes and should be passed through unchanged.
Use `ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(...)`
with record-bundle, Pallas open-envelope, and recursive compact key-artifact
archives, and `VerifyRecursiveCompactPaymentToken(...)` with compact-token and
recursive compact verifier-key archives; gate them with
`IsRecursiveCompactPaymentTokenProverAvailable()` and
`IsRecursiveCompactPaymentTokenVerifierAvailable()`. The recursive-spend
compact projection verifier is exposed separately as
`VerifyRecursiveSpendCompactPaymentTokenProjection(...)`; gate it with
`IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable()`. It accepts
raw Norito compact-token and verifier-record archives, rejects empty,
malformed, oversized, or invalid-height inputs before P/Invoke dispatch, and
returns the native boolean receiver result. ABI 7 now carries one-hop LEN=4 and
package-backed multi-hop compact-token proof paths when the native bundle
includes packaged compact proving-key archives and matching verifier-slice
material. Production defaults still stay on ABI 6 Reserved-lineage recursive
spend until that artifact set is shipped and signed for release. Empty,
malformed, missing, or oversized local archives fail as `ArgumentException`;
bridge error code `-312` remains classified as legacy reserved ABI-7 state. The
ABI-7 launch boundary remains explicit: the one-hop LEN=4 compact-token proof
path uses a packaged compact one-hop proving-key, while release evidence
continues to track the proof-composition reservation, generic compact-token
reservation, and multi-hop verifier-batch reservation. Missing native symbols
still surface as `InvalidOperationException`. For
Reserved-lineage branching, use `CanRedeemWitnessless(...)`,
`RequiresLineageWitnessForRedeem(...)`, `PreferredAppendOutputCircuitId(...)`,
`CanProveAppendOutputCircuitId(...)`, and
`CanSelectAppendOutputCircuitId(...)` instead of duplicating circuit-id rules in
app code. `IsSupportedPreviousProofCircuitId(...)` and
`RequiresPreviousLineageVerifierRecordForAppend(...)` tell app code when to
reject an unknown previous proof circuit and when to include
`previous_lineage_verifier_record`. `RecursiveSpendLineageWitnesslessMaxHopsV1`
is `64`, and `RecursiveSpendLineageTransitionCircuitWiredV1` is `true`:
witnessless Reserved-lineage online redemption is available for lineage bundles
inside the 64-hop cap. `CanAppendWitnesslessLineage(...)` returns `true` for
previous hop counts `1..63`, and `PreferredAppendOutputCircuitId(...)` selects
Reserved-lineage append inside that range.
`previous_recursive_proof_open_envelopes_archive` is opaque native prover
material: C# wallet code must pass it through Norito unchanged and must not
construct, rewrite, or mutate it. The native bridge validates `vk_commitment`,
`public_inputs_schema_hash`, and `domain_tag` against the exact previous bundle
before proving or returning output bytes.
Native append streams the previous recursive proof bytes and per-hop accumulator
material into native-owned accumulator digests (`recursive_proof_chain_digest`,
lineage/aggregation transcript, fixed-window schedule/shared-manifest/table-base,
verifier-witness batch, transition-profile, append-opening-preflight,
append-boundary, scalar-projection, and previous/resulting accumulator digests);
SDK code must not derive, supply, or patch accumulator state.
Verify request archives must pass the same public-binding preflight before the
native bridge returns a recursive spend verify result: Reserved-lineage bundles
require a matching active `lineage_verifier_record`, semantic bundles must omit
it, and unsupported proof attachments are rejected as malformed requests rather
than soft invalid proof results.
Production init requests and Reserved-lineage append-output requests must also
include packaged lineage key artifacts in the raw Norito request:
`lineage_verifier_key` and `lineage_proving_key_archive`. Missing artifacts are
rejected before runtime key generation.
Use `LineageKeyArtifactsForInit(...)` and `LineageKeyArtifactsForAppend(...)`
to package and validate these verifier/proving key artifacts before building
the raw request.
Semantic append is bounded by the separate `CompactTokenMaxHops` constant;
witnessless Reserved-lineage append and redeem use
`RecursiveSpendLineageWitnesslessMaxHopsV1`.
Reserved-lineage append output is valid only when the previous bundle is
already Reserved-lineage; semantic previous bundles keep using semantic append
plus a record-backed lineage witness.

## Native Privacy Bridge

`Hyperledger.Iroha.Privacy.PrivacyNative` exposes the privacy FFI surface as
generic raw Norito archives: `CapabilitiesV1()`, `BuildProofV1(requestArchive)`,
and `VerifyProofV1(requestArchive)`. Approved typed proof-builder aliases for
admitted production privacy entrypoints, including
`BuildZkAceAuthorizationProofV1(requestArchive)`, dispatch through the same
production archive paths and remain fail-closed while the privacy rows are
gated. Planned catalog entrypoints stay unexported until their production gates
pass. Native availability requires ABI 6 or later, the privacy
capability/build/verify symbols, and successful Norito probe outputs whose
operation-specific result schema bytes match the called entry point.

All privacy request and response payloads must stay as raw Norito archives. C#
validates archive magic, length, CRC, the 64 MiB native size cap, and the
operation-specific result schema before returning bytes to callers. Capability
metadata reports `privacy-production-gate-v1`, keeps `ProductionReady = false`,
and remains fail-closed with missing production gates and no audit references
until real proving, verification, chain admission, witness privacy checks,
deterministic testing, negative/adversarial testing, replay/nullifier rejection
testing, parser/verifier fuzzing, performance gates, and external audit signoff
are complete.

C# also exposes the deterministic privacy FFI status/error-code contract for
diagnostics and cross-language parity: `StatusError`, `ErrorNullPointer`,
`ErrorMalformedNorito`, `ErrorUnsupportedAlgorithm`,
`ErrorProductionDisabled`, and `ErrorInvalidRequest`. The stable wire values
are `status_error = 1`, `null_pointer = 1`, `malformed_norito = 2`,
`unsupported_algorithm = 3`, `production_disabled = 4`, and
`invalid_request = 5`; treat them as sanitized status metadata, not proof success.

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
- `/v1/contracts/instances/{ns}` when the deployment exposes contract metadata, using `universal` by default, with `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view` when a code hash is available from that namespace or the override env var below
- `/v1/identifier-policies`
- `/v1/vpn/profile`
- `/v1/sorafs/denylist/catalog` when the deployment exposes the denylist surface
- `/v1/sorafs/denylist/packs/{pack_id}` when the catalog is available and non-empty
- `/v1/aliases/by_account`
- `/v1/accounts/faucet/puzzle`
- `/v1/space-directory/uaids/{uaid}`
- `/v1/space-directory/uaids/{uaid}/manifests`

Optional live-smoke environment variables:

- `IROHA_CSHARP_SMOKE_CONTRACT_NAMESPACE` to override the default contract-instance namespace (`universal`)
- `IROHA_CSHARP_SMOKE_CONTRACT_CODE_HASH` to override the code hash used for `/v1/contracts/code/{code_hash}`, `/v1/contracts/code-bytes/{code_hash}`, and `/v1/contracts/code/{code_hash}/contract-view`
- `IROHA_CSHARP_SMOKE_SORAFS_CID` and optional `IROHA_CSHARP_SMOKE_SORAFS_PATH` to also probe `/v1/sorafs/cid/{cid}` plus `/sorafs/cid/{cid}/...`
- `IROHA_CSHARP_CANONICAL_ACCOUNT_ID` plus `IROHA_CSHARP_PRIVATE_KEY_SEED_HEX` to also create a signed VPN quote and verify that Torii returns an `OpenVpnLeaseEscrow` instruction skeleton

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
