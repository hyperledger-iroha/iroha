# Hyperledger Iroha SDK for .NET

The C# SDK is the typed .NET 8 client for Hyperledger Iroha 3. This first-release
surface exposes the V1 protocol directly and removes pre-release wrappers, fallback
codecs, compatibility modes, and deprecated transport APIs. Parser vocabulary shared
with other SDKs changes only through coordinated cross-SDK updates.

For operator and protocol documentation, see [docs.iroha.tech](https://docs.iroha.tech/).
This file focuses on getting a .NET application connected safely.

## Requirements

- .NET SDK 8.0.419, as pinned by `global.json`
- A Torii endpoint
- The exact genesis-derived `NetworkId` for authenticated requests
- A canonical, domainless I105 account ID and its 32-byte Ed25519 seed for signing

Account construction, parsing, and every operation that admits account identities
require the packaged ABI-23 Rust bridge for the current runtime identifier. Privacy
and native SoraFS validation use the same bridge. Transport-only anonymous reads do
not construct account identities.

Exact12 capability admission requires authenticated HTTPS Torii reads, the configured
`NetworkId`, and native validation of the complete signed qualification. The SDK
retains that exact network on the manifest and admission token, checks the deployment
network against its genesis hash, and revalidates native evidence and network identity
at construction. Offline decoded archives are inspection data and cannot grant admission.

## Add the SDK

From a consuming project after the package is published:

```bash
dotnet add package Hyperledger.Iroha.Sdk --version 0.1.0
```

Inside this repository, use a project reference:

```xml
<ProjectReference Include="path/to/iroha/csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj" />
```

## Five-minute start

Anonymous, replay-safe reads need only the endpoint:

```csharp
using Hyperledger.Iroha;

using var client = new IrohaClient(new Uri("https://torii.example"));
var health = await client.Torii.GetHealthAsync(cancellationToken);
Console.WriteLine(health);
```

Authenticated routes use canonical request credentials and an exact `NetworkId`:

```csharp
using System.Security.Cryptography;
using Hyperledger.Iroha;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

var seed = Convert.FromHexString(seedHex);
try
{
    using var credentials = new CanonicalRequestCredentials(accountId, seed);
    using var client = new IrohaClient(
        new Uri("https://torii.example"),
        new ToriiClientOptions
        {
            NetworkId = NetworkId.Parse(networkIdFromGenesis),
            CanonicalRequestCredentials = credentials,
        });

    var accounts = await client.Torii.GetAccountsAsync(
        limit: 25,
        cancellationToken: cancellationToken);
}
finally
{
    CryptographicOperations.ZeroMemory(seed);
}
```

`CanonicalRequestCredentials` snapshots the seed, never exposes it through the public
API, and zeros its owned copy when disposed. `Ed25519KeyPair` follows the same ownership
model; its deliberately named `ExportPrivateKeySeed()` returns a caller-owned secret
that must also be zeroed.

## Prepared onboarding and faucet operations

Prepared operations use the closed `ToriiPreparedOperationBindingV1` contract:
`schema`, `semantic_hash_hex`, `kind`, `request_id`, and
`execution_expires_at_unix_ms`. The SDK authenticates the exact wire, signatures,
metadata and requested fee intent against the caller's network and onboarding
authority or faucet policy.

`ToriiClient.VerifyAccountOnboardingPreparedTransactionV1` and
`ToriiClient.VerifyAccountFaucetPreparedTransactionV1` verify retained envelopes
without HTTP or a current-time check. Supply the independently trusted request or
claim, binding, network, fee intent and authority/policy; onboarding also requires
the trusted receipt and its canonical body encoder. `SubmitPreparedAccountOnboardingAsync`
and `SubmitPreparedAccountFaucetAsync` repeat authentication and reject expired
operations immediately before HTTP dispatch. Signed transaction TTL must be positive
and fit within the authenticated deadline. Uncertain submissions are never replayed
automatically.

## Quote, sign, submit, and wait

The guided ledger flow freezes the transaction draft before awaiting the fee quote,
verifies that the quote preserves payer and gas invariants, signs that exact snapshot,
submits it once, and waits for authoritative global finality:

```csharp
using Hyperledger.Iroha.Transactions;

var transaction = client.Ledger
    .BuildTransaction(
        networkId,
        accountId,
        FeePaymentIntent.Authority(Array.Empty<FeeChargeLimit>()))
    .TransferAsset(assetDefinitionId, "1", destinationAccountId)
    .SetTimeToLiveMilliseconds(30_000)
    .SetNonce(1);

var submission = await client.Ledger.QuoteSignAndSubmitAsync(
    transaction,
    seed,
    new PipelineSubmitOptions
    {
        PollInterval = TimeSpan.FromMilliseconds(250),
        Timeout = TimeSpan.FromSeconds(30),
    },
    cancellationToken);

Console.WriteLine(submission.Transaction.TransactionHashHex);
```

Do not mutate and reuse a builder as shared concurrent state. The guided flow protects
an in-flight operation by taking its own snapshot, but a builder remains a simple
single-operation construction object.

## Errors, cancellation, and transport ownership

- Non-success HTTP responses throw `ToriiApiException` with status, request URI, and a
  bounded response body.
- Protocol-shape failures throw `JsonException` or `InvalidDataException` before a DTO
  reaches application code.
- Caller cancellation remains `OperationCanceledException`.
- Pipeline finality expiry throws `TimeoutException`, distinct from caller cancellation.
- Buffered JSON, text, error, and SoraFS reads have explicit memory limits. Use
  `OpenSoraFsCidContentAsync` to stream content larger than the buffered limit.

The normal `IrohaClient(Uri, ToriiClientOptions?)` constructor creates and owns a
no-redirect transport suitable for signed, nonce-bearing operations. This is the
recommended constructor.

The `HttpClient` overload is for anonymous reads only. The caller retains ownership of
that client. Supplying bearer or canonical credentials with an injected transport fails
at construction because the SDK cannot prove that external handlers, redirects, or
retry policies will not replay an authenticated request. A per-request onboarding token
must never be present in that client's default headers.

## Typed protocol surface

Use the domain methods on `ToriiClient`, `LedgerClient`, and the dedicated builders.
The public SDK does not expose arbitrary path + JSON request helpers or raw SSE response
handles. Typed methods centralize canonical paths, strict JSON rules, response bounds,
authentication, and validation.

Major first-release areas include:

- accounts, aliases, assets, domains, NFTs, roles, triggers, peers, and blocks;
- signed iterable queries and canonical transaction submission;
- fee quotes and sponsor programs;
- pipeline, data, proof, and explorer event streams;
- contracts, runtime governance, verifying keys, privacy, KAGEMUSHA V1, SCCP, VPN, and
  SoraFS routes.

## Run the sample

```bash
cd csharp
export IROHA_CSHARP_TORII_BASE_URL=https://taira.sora.org
export IROHA_CSHARP_NETWORK_ID='hash:...#....'
export IROHA_CSHARP_CANONICAL_ACCOUNT_ID='sora...'
export IROHA_CSHARP_PRIVATE_KEY_SEED_HEX='...'
dotnet run --project samples/Hyperledger.Iroha.Sdk.Sample
```

Treat seed environment variables as local-development inputs. In production,
use an application secret provider; that software-backed custody path is valid.
A hardware-backed signing boundary is an optional integration.

## Build and test

Run commands from `csharp/`:

```bash
dotnet restore Hyperledger.Iroha.Sdk.sln
dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore -warnaserror
dotnet test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj -c Release --no-build
dotnet test tests/Hyperledger.Iroha.Sdk.IntegrationTests/Hyperledger.Iroha.Sdk.IntegrationTests.csproj -c Release --no-build
```

The integration suite is environment-gated; its test project documents the required
variables. The executable sample lives in `samples/Hyperledger.Iroha.Sdk.Sample`.

## KAGEMUSHA wallet operation and recovery contract

Applications must persist a fresh, nonzero 32-byte operation ID before requesting
a payment request, payment, mint construction or redemption. Pass that same ID
and the same public inputs on every retry. The wallet reserves the exact ID with
native Core and rejects substituted reservation results; request creation also
requires the returned request ID to match. An interrupted call must not allocate
a replacement monetary operation.

Opening a new qualified wallet corroborates bootstrap with a second native
recovery snapshot and the live journal revision. Recovery of an existing wallet
never bootstraps missing state. It rejects identity changes, journal rollback,
same-revision equivocation and invalid epoch-generation transitions before
publishing the recovered snapshot. These checks require an authenticated native
provider; passing orchestration tests does not qualify a device or enable offline
money. See the [production-readiness record](../specs/kagemusha_v1_production_readiness.md).

## Pack

Package validation expects the native bridge stage to contain the supported runtime
artifacts. Build or obtain those artifacts, stage them with
`scripts/package_csharp_native_artifacts.py`, then run:

```bash
dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj \
  -c Release \
  --output artifacts/packages
```

Repository layout:

- `src/Hyperledger.Iroha.Sdk` — package source
- `tests/Hyperledger.Iroha.Sdk.Tests` — unit and protocol-contract tests
- `tests/Hyperledger.Iroha.Sdk.IntegrationTests` — live Torii smoke tests
- `samples/Hyperledger.Iroha.Sdk.Sample` — minimal executable example

## Kaigi V1

`Hyperledger.Iroha.Kaigi` owns the final managed Kaigi call configuration,
relay manifest, authorization/usage artifact bundles, and retained record JSON
projection. Use `TransactionInstruction.CreateKaigi`, `JoinKaigi`, `LeaveKaigi`,
`EndKaigi`, and `RecordKaigiUsage` to encode canonical Norito InstructionBoxes.
Private create requires a complete `KaigiAuthorizationArtifactsV1`; private
join, leave and end pass the same complete bundle type. Usage accepts a
`KaigiUsageArtifactsV1`. The node determines the call's privacy mode and verifies
the supplied proof against trusted state.

`KaigiAuthorizationScalarV1` retains exactly 32 little-endian bytes below the
Pasta Fp modulus, including zero. Commitment and nullifier wrappers each have
one field. Scalar JSON is an exact 32-element byte array. No hash marker,
reduction, alias tag, or issued-at timestamp is part of these values. Roster
roots retain their separate canonical Iroha Hash encoding.

`KaigiRecordV1.FromJson` reads the full retained model, including the original
`host`, private ownership sequences, current roster, nullifier history and usage
commitments. It rejects missing/unknown/duplicate fields, noncanonical scalars,
invalid sequence bounds, effective participant limits, lifecycle mismatches and
inconsistent roster ownership. Active private records reserve nullifier capacity
for every live leave and host end. Identity comparisons retain the complete
controller, including multisig policy, while ignoring network display prefixes. The redacted Torii
application call view does not provide this retained model. Decoding does not
verify the response's authenticity, roster root, proof, canonical account order,
or account-rekey authority; those remain with the trusted ledger and verifier.

Managed identity names currently require canonical ASCII Name/domain labels,
matching the Kotlin Kaigi encoder's fail-closed scope until the Rust pinned
NFC/UTS-46 owner is shared. Display text supports UTF-8. Account instruction
encoding covers all eleven published controller curve IDs and complete canonical
multisig policies with a u16 member count. Address decoding checks key envelopes;
every public address constructor and parser additionally requires the ABI-23
Rust address owner for complete key and policy admission. Missing native
validation raises `NativeBridgeUnavailable`; structural checks cannot admit an
account by themselves. Canonical I105 parsing rejects surrounding Unicode
whitespace. SCCP preserves extended single-key envelopes and full multisig
AccountId bytes within its 65,535-byte principal limit. Signature verification
remains a separate operation.
The C# SDK does not yet generate Kaigi proofs or expose a native Kaigi prover.

The five transparent instructions and complex private-create bytes are pinned
against the shared Rust-decoded and re-encoded fixture at
`python/iroha_python/tests/fixtures/kaigi_instruction_wire_v1.json`. All five
private actions also match the exact scalar fixture at
`tests/Hyperledger.Iroha.Sdk.Tests/Fixtures/kaigi_private_instruction_wire_v1.json`.
The retained
record fixture at `tests/Hyperledger.Iroha.Sdk.Tests/Fixtures/kaigi_record_v1.json`
was emitted and round-tripped by the final Rust data-model owner, with its
participation ledger checked against its roster; SHA-256 is
`0e54e88cd17476645d0bc88d307e49dc3237d0adda5c652357759ce66b65667d`.
These are model fixtures with synthetic artifacts, not proof-generation,
four-validator, hardware or release-qualification evidence.
