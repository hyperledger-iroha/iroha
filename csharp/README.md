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

The managed HTTP, address, transaction, query, and Norito surfaces do not require a
native library. Privacy and native SoraFS validation features use the packaged native
bridge for the current runtime identifier.

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
- contracts, runtime governance, verifying keys, privacy, Kagemusha, SCCP, VPN, and
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

Treat seed environment variables as local-development inputs. Prefer an application
secret provider or hardware-backed signing boundary in production.

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
