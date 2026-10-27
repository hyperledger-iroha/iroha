# Hyperledger Iroha SDK for .NET

The C# SDK is the typed .NET 10 client for Hyperledger Iroha 3. This first-release
surface exposes the V1 protocol directly and removes pre-release wrappers, fallback
codecs, compatibility modes, and deprecated transport APIs. Parser vocabulary shared
with other SDKs changes only through coordinated cross-SDK updates.

For operator and protocol documentation, see [docs.iroha.tech](https://docs.iroha.tech/).
This file focuses on getting a .NET application connected safely.

Contract-artifact reads and verified-source jobs use `ContractArtifactId(dataspaceId,
codeHash)` and canonical request credentials. The dataspace is an explicit `ulong`;
the hash is the lowercase marked hash of the complete `.to` artifact. Responses bind
both the configured network and requested artifact identity, and bytecode reads verify
the domain-separated complete-artifact hash. Singular manifest queries use
`FindContractManifestByArtifactId` with the same identity.
Singular queries use the current native query discriminants and Norito newtype
framing. Domain endorsement queries and domain transfer/metadata instructions require the fully qualified `domain.dataspace`
identity, for example `banka.universal`.
These operations require the ABI-33 Rust domain validator and exact canonical ASCII
labels, including admitted IDNA A-labels; raw Unicode and alternate spellings are
rejected. The native pinned UTS-46 owner also decides underscore and hyphen admission.
NFT instructions use the same fully qualified domain inside `name$domain.dataspace`.
Typed transfer, metadata and trigger instructions preserve the native enum struct
field and the `TriggerId`/`Json` newtype fields; transaction metadata uses that same
`Json` encoder.

## Requirements

- .NET SDK 10.0.402, as pinned by `global.json`; consumers require .NET 10 or newer
- A Torii endpoint
- The exact genesis-derived `NetworkId` for authenticated requests
- A canonical, domainless I105 account ID and its 32-byte Ed25519 seed for signing

Account construction, parsing, and every operation that admits account identities
require the packaged ABI-33 Rust bridge for the current runtime identifier. Privacy
and native SoraFS validation use the same bridge. Transport-only anonymous reads do
not construct account identities.

Identifier policies expose an immutable `ToriiRamFheProfile` through the existing
`GetIdentifierPoliciesAsync()` response. Its unsigned dimensions, encrypted-input
mode and exact initializer descriptor hash are validated during decoding; absent
profiles remain `null`. The profile describes public parameters and does not grant
execution-proof availability.

Binding-only IVM verifier labels are retired and rejected. Production proof-backed IVM invocation remains closed until the complete native execution relation and finalized State authority are implemented and qualified.

Privacy archive queries and validation run on the caller's ordinary stack. They
do not create enlarged-stack threads. Native result bounds, owned input snapshots
and native-buffer cleanup apply on that same call path.

Exact12 capability admission requires authenticated HTTPS Torii reads, the configured
`NetworkId`, and native validation of the canonical runtime manifest. Its top-level
network must match the authenticated client. Availability derives from the executable
compiled tuple and current per-protocol governance; release/deployment diagnostics
are not runtime prerequisites. The admission token revalidates the exact archive,
network and local tuple at construction. Offline decoded archives are inspection data and cannot grant admission.

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

    var capabilities = await client.Torii.GetNodeCapabilitiesAsync(cancellationToken);
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

## Query collections

Every Torii collection uses one query language and one page envelope
(`{"items": [...], "next_cursor": ..., "total": ...}`; see
`specs/torii/collection_queries.md`). `ToriiClient` exposes collections as
`ToriiCollection<T>` values with typed rows: `Domains`, `Accounts`, `AssetDefinitions`,
`Nfts`, `Rwas`, `RepoAgreements`, `AccountAssets(accountId)`,
`AssetHolders(assetDefinitionId)`, `Transactions`, `AccountTransactions(accountId)`,
`AccountPermissions(accountId)`, `SubscriptionPlans`, `Subscriptions` and
`UaidManifests(uaid)`. `ContractActivity`, `ContractEvents` and
`AccountHistory(accountId)` expose history rows as JSON objects with the same
filter/select/limit/cursor controls. Page envelopes require an explicit
`next_cursor` (null or a nonempty token) and reject retired or unknown fields.
Explorer collections use the same API: `ExplorerAccounts`, `ExplorerDomains`,
`ExplorerAssetDefinitions`, `ExplorerAssets`, `ExplorerNfts`, `ExplorerRwas`,
`ExplorerBlocks`, `ExplorerTransactions`, `ExplorerLatestTransactions`,
`ExplorerInstructions` and `ExplorerLatestInstructions`. Their rows retain the
strict Explorer DTO validation. These bounded feeds have fixed server order,
accept `Filter`, `Select` (through `.Rows`), `Limit` and `Cursor`, and reject
`Sort`, `IncludeTotal` and `Aggregate`. Torii defaults to 25 rows and caps
Explorer pages at 100. An empty page with a cursor still has more work; iterators
follow it. Detail and event stream methods remain separate.

Reads are public; when credentials are configured the request is signed, which only widens
visibility into restricted dataspaces. Row identity fields (`Id`; `AccountId`, `Asset`,
`Scope` and `Quantity` for balances; `EntrypointHash`, `BlockHeight` and `BlockIndex` for
transactions) are always present; every other field may be `null`.

```csharp
using Hyperledger.Iroha.Query;
using Hyperledger.Iroha.Torii;

// One page, filtered and sorted. Quantities are exact decimals (never doubles).
var query = new ListQuery
{
    Filter = Filter.Field("scope").Eq("global") & Filter.Field("quantity").Gte(10.5m),
    Sort = ["-quantity", "account_id"],
    Limit = 50,
};
Page<AssetHolderRow> page = await client.Torii.AssetHolders(assetDefinitionId)
    .GetPageAsync(query, cancellationToken);

// Every item: the iterator follows next_cursor until the last page.
await foreach (var holder in client.Torii.AssetHolders(assetDefinitionId)
    .EnumerateAsync(query, cancellationToken))
{
    Console.WriteLine($"{holder.AccountId}: {holder.Quantity}");
}
```

- Filters build with `Filter.Field(...)` and combine with `&`, `|` and `!`; parse the text
  form with `Filter.Parse("owned_by = \"alice\" and quantity >= 10.5")`, or pass text
  through unchanged with `ListQuery.FilterText`. `ToString()` renders the canonical text
  and `ToJson()` the canonical JSON form.
- Decimals are exact: pass `decimal`, `NumericV1` values or decimal strings. Torii rejects
  fractional JSON numbers, including inside structured literals, and so does the SDK.
- Object and array literals (`FilterLiteral.Json(...)`, for `metadata.<key>` values) exist
  only in the JSON form. Collection reads always send JSON, so they work there;
  `ListQuery.ToQueryString()` and event streams reject them with `invalid_filter`.
- Sort keys use the text spelling (``"-metadata.`ui-order`"``) in both forms; `Select`,
  `GroupBy` and filter JSON arguments use raw dotted paths (`"metadata.ui-order"`).
- Projections (`Select`) and aggregates (`Aggregate`) return partial or computed rows;
  read them as `JsonObject` through `.Rows`, for example
  `client.Torii.AssetHolders(id).Rows.GetPageAsync(aggregateQuery)`. Torii executes
  each collection query once over the caller-visible global state. For collections
  that support totals and `POST` aggregates, visible rows contribute exactly once
  even when they span several dataspace routes.
- `Cursor` resumes after a page (`query with { Cursor = page.NextCursor }`);
  `IncludeTotal = true` adds `Page<T>.Total`.

`Transactions` (`POST /v1/transactions/query`) and `AccountTransactions(accountId)` are
history collections. Rows come newest first by `block_height`, then `block_index`, and the
cursor holds block coordinates, so transactions committed while paging never shift later
pages. `Sort`, `IncludeTotal` and `Aggregate` would scan the whole history and are rejected
before dispatch (`invalid_sort`, `invalid_include_total`, `invalid_aggregate`). Each page has
a bounded scan budget, so a selective filter can return a short or even empty page that
still has a `NextCursor`; the iterators keep following it until it is `null`. Bounds on
`block_height` in the filter's top-level `and` also bound the scan, and the list fields
`asset_ids` and `asset_definition_ids` match element-wise (`=` and `in` keep rows where any
element matches; `!=` and `not in` rows where none does):

```csharp
var failed = new ListQuery
{
    Filter = Filter.Field("block_height").Gte(1200)
        & Filter.Field("asset_definition_ids").Eq(assetDefinitionId)
        & Filter.Field("result_ok").Eq(false),
};
await foreach (var transaction in client.Torii.Transactions.EnumerateAsync(failed, cancellationToken))
{
    Console.WriteLine($"{transaction.BlockHeight}/{transaction.BlockIndex}: {transaction.EntrypointHash}");
}
```

Invalid controls are rejected before any request with `ListQueryException`, and Torii
rejections arrive as `ToriiApiException`; both derive from `IrohaException` and carry the
Torii error code:

```csharp
try
{
    await client.Torii.Domains.GetPageAsync(new ListQuery { FilterText = userInput }, cancellationToken);
}
catch (IrohaException error) when (error.Code == "invalid_filter")
{
    Console.WriteLine($"{error.Message} (hint: {error.Details?.Hint})");
}
```

Event streams (`GET /v1/events/sse`) take the same text grammar, restricted to what event
subscriptions match: the fields `tx_status`, `tx_hash`, `tx_block_height`, `tx_lane_id`,
`tx_dataspace_id`, `block_status`, `block_height`, `proof_backend`, `proof_call_hash` and
`proof_envelope_hash`; `=` and `in` combined with `and` and `or`; `not` only over a status
equality; and `tx_block_height is null`. Torii rejects anything else with `invalid_filter`.
`StreamEventsAsync` decodes each payload into a `ToriiEvent` record:
`ToriiTransactionEvent` (`Status` is `Queued`, `Expired`, `Approved` or `Rejected`, with
`RejectionCode` and `RejectionReason` when rejected), `ToriiBlockEvent`,
`ToriiPipelineWarningEvent`, `ToriiWitnessEvent`, `ToriiProofVerificationEvent`,
`ToriiProofPrunedEvent`, `ToriiDataEvent` and `ToriiOtherEvent`. Events the SDK does not
model arrive as `ToriiUnknownEvent` with the raw payload instead of failing the stream, and a
terminal `stream_error` frame ends it with `ToriiStreamException`.
`StreamPipelineEventsAsync` and `StreamProofEventsAsync` keep one event family, and
`StreamServerSentEventsAsync` returns the raw frames.

```csharp
var filter = Filter.Field("tx_hash").Eq(transactionHashHex)
    & Filter.Field("tx_status").In("Approved", "Rejected");
await foreach (var pipelineEvent in client.Torii.StreamPipelineEventsAsync(filter, cancellationToken))
{
    if (pipelineEvent is ToriiTransactionEvent transaction)
    {
        Console.WriteLine($"{transaction.Hash}: {transaction.Status} {transaction.RejectionCode}");
    }
}
```

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

- Every SDK error with a stable code derives from `IrohaException` (`Code`, `StatusCode`,
  `Details`). Non-success HTTP responses throw `ToriiApiException`, which parses the
  Torii `{code, message, details}` envelope and the `x-iroha-reject-code` header
  (`RejectCode`) and keeps the request URI and bounded response body.
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
- contracts, runtime governance, verifying keys, privacy, VPN, and SoraFS routes.

For an existing public standalone ballot, use
`TransactionBuilder.UpdatePlainConviction(referendumId, newTotalBond, durationBlocks)`.
The direct V1 instruction contains only the referendum selector, owner, new total
bond and requested lock duration. The choice is immutable and read from finalized
state; a repeated cast is not an update.

`CallContractAsync` validates the canonical nine-field transaction payload against
the exact caller-trusted network, authority, invocation, metadata and fee before
returning a draft for local signing. Retired admission-intent layouts are rejected.

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

For focused validation, invoke the built xUnit v3 executable with its method
selector. Microsoft.Testing.Platform ignores the VSTest `dotnet test --filter`
and `--logger` switches. From the same `csharp/` directory after the build above:

```bash
dotnet tests/Hyperledger.Iroha.Sdk.Tests/bin/Release/net10.0/Hyperledger.Iroha.Sdk.Tests.dll -method '*Explorer*' -noLogo -noColor
```

The selector matches the fully qualified method name. Use `Debug` in the artifact
path when validating a Debug build.

## Pack

Package validation expects the native bridge stage to contain the supported runtime
artifacts. Build or obtain those artifacts, stage them with
`scripts/package_csharp_native_artifacts.py`, then run:

```bash
dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj -c Release --no-build --output artifacts/packages
CSHARP_SDK_PACKAGE_CONSUMER_RUNTIME_IDENTIFIER=osx-arm64 ../ci/check_csharp_sdk_package_consumer.sh
```

Pack the Release build validated above, then run the package-consumer gate with
the host's supported runtime identifier (`linux-x64`, `linux-arm64`, `osx-x64`,
`osx-arm64`, or `win-x64`). The gate installs the local NuGet package into a fresh
consumer through `PackageReference`, verifies its staged native artifact, and
runs the managed and native smoke checks. It rejects `ProjectReference` so a
source build cannot substitute for the package being released.

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
every public address constructor and parser additionally requires the ABI-33
Rust address owner for complete key and policy admission. Missing native
validation raises `NativeBridgeUnavailable`; structural checks cannot admit an
account by themselves. Canonical I105 parsing rejects surrounding Unicode
whitespace. Signature verification remains a separate operation.
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

## Petal Stream optical transport

`Hyperledger.Iroha.Petal` is the managed port of the Rust reference
`crates/iroha_petal`: an animated, camera-readable frame sequence ("streaming QR"
in the Sakura-storm look) that moves an opaque byte payload from a screen to a phone.
The stream carries bytes only; this SDK has no peer-message codec. A KAGEMUSHA
wallet V1 sender plays one `IPM1` message with its IPM1 kind as the stream `kind`
([wallet wire §6](../specs/kagemusha_wallet_wire_v1.md)). Frames use format 5
([Petal Stream spec](../specs/petal_stream.md)): each is a portrait card of
1024 × 1668 units, a square code with a band of katakana tiles above and below it
(`PetalLayout.CardHeight`, `SquareTop`). The square carries five independent
Reed–Solomon lanes of fountain-coded atoms — the 3,376 petals of the field, cut
into four sectors (`F0`–`F3`, `PetalLanes.Field(sector)`), and the ring dots (`D`,
which carries the stream beacon every fourth frame) — and on top of every field lane
a turn lane (`T0`–`T3`, `PetalTurns`): the first 360 lit petals of a sector are
turned by 0°, 45°, 90° or 135° about their centres, two more bits each, carrying
four more atoms from the repair range. Each band holds 306 tiles (`PetalLayout.KanaTiles`),
eight of them pilots with fixed symbols; the other 298 show one of sixteen katakana
(`PetalGlyphs`) and are light or dark, which carries a polarity lane (`P0`, `P1`) and a
glyph lane (`K0`, `K1`) per band (`PetalKana`): eight more atoms per band, bound to the
stream tag, the frame and the lane by a CRC-32C check. Any readable lane of any frame
adds progress. Reports name the lanes `D` and `0`–`3` (a turn lane by the digit of its
field lane) and `P0`, `K0`, `P1`, `K1`. Petals inside the `天` are drawn light and the
others pink, over a faint `天`; a light tile is filled light with its glyph in ink, a dark
tile is black with its glyph in pink. A payload is released only after its CRC-32C
matches the beacon.

```csharp
// Sender: draw frames at the default 12 fps through an IPetalCanvas adapter over the
// platform canvas (SkiaSharp, MAUI ICanvas, WPF DrawingContext, ...). The whole card is
// drawn, `side` wide and side · 1668 / 1024 tall, from its top-left corner.
var player = new PetalFramePlayer(new PetalStreamEncoder(payload, kind: 2));
player.Draw(canvasAdapter, clock.Elapsed, side: viewWidthInPixels);

// Receiver: feed each camera Y plane (CameraX plane 0, CVPixelBuffer luma).
var analyzer = new PetalCameraAnalyzer();
analyzer.PayloadCompleted += (_, done) => Handle(done.Meta.Kind, done.ToArray());
analyzer.AnalyzeYPlane(yPlane, width, height, rowStride, pixelStride, timestampMs);
```

`PetalDecoder.Decode` reports `UnsupportedImage`, `NoFinders` or
`NoOrientation` through `PetalDecodeResult` and never throws on image content.
Every petal is one bit: its level, normalised by the light and dark levels measured
at the finders, is compared with a two-means threshold of its sector and colour
class. When a plain pose leaves a field lane unread, the decoder searches the pose
for radial lens distortion (`PetalPose.Distortion`), then reads the sectors still
unread from local levels that rescale every petal by its 32-unit neighbourhood,
which cancels shadows, glare, banding and veiling light. `PetalScanSession` follows
the code from frame to frame and also averages the field evidence of up to four
captures of each of six shown frames to recover missing field lanes. Independently
decoded headers must agree, and recovered lane headers must name that frame.
`PetalScanOutcome.Merged` and `PetalScanStats.Merges` and `MergedLanes` report this
recovery separately from individual captures (`LaneD`, `FieldLanes`); reset and
stream timeouts discard held evidence. `PetalDecoder.FieldEvidence` and
`MergeFields` expose the same low-level read.

`PetalTurns.ReadTurns` reads the turn lanes of the field lanes a capture decoded:
it samples every carrier petal in its own outward frame under a smooth correction
of the pose, fits the blur under which the petals and their lit neighbours explain
the image, and gives each carrier the turn whose template matches best once its
neighbours are subtracted; a turn lane passes only when its Reed–Solomon codeword
decodes and its CRC-32C check matches the header of the field lane it rides on. A
camera too soft to resolve the petals' shapes reads no turn lane and loses nothing
else. The session tries each turn lane once per shown frame (`PetalScanOutcome.Turned`,
`PetalScanStats.TurnTries` and `TurnLanes`) and, after tries that read nothing,
passes over the next captures that would try one, at most `TurnBackoff` of them.
`PetalTurns.ObservedTurns` reports every carrier's decided turn for diagnostics.
`PetalFrameCells.Turns`, `SetTurns` and `TurnWord`, `PetalLaneData.Turns` and the
draw list's turned, notched petals (`IPetalCanvas.FillPetal`: the ellipse minus its
notch, antialiased) expose the turns to senders.

`PetalKana.ReadKana` reads the kana lanes of a capture whose square decoded, under the
`(tag, frame)` header its lanes name: the square's pose, extended beyond the square,
places every tile; each band's pilots are found by a coarse search whose reach grows
with the distance from the square, pick the blur of the glyph templates, correct every
tile by an affine shift fitted to their offsets, and calibrate the levels that tell
light tiles from dark ones; each data tile then takes the polarity its level shows and
the glyph whose template correlates best. A lane passes when its Reed–Solomon codeword
decodes (with an erasure schedule) into data whose check matches the square's frame,
the frame after it or the frame before it (a rolling shutter or a stale display can
show a band from a neighbouring frame). A phone held upright sees both bands; a camera
that frames only the square reads the square as before. The session reads each band
once per shown frame (`PetalScanOutcome.Kana`, `PetalScanStats.KanaTries` and
`KanaLanes`) with its own back-off (`KanaBackoff`). The bands go first: a capture whose
bands read a kana lane tries the turn lanes too only when its work and the session's last
turn try fit a 30 fps camera's frame time (`CaptureBudgetMilliseconds`); otherwise the
turn lanes of that shown frame wait for its next capture, because on a slow phone core a
turn try costs five tracked decodes and a band try one. The session times its work with
`Stopwatch`, and the constructor's `workCost` (a `Func<PetalWork, double, double>`) puts
a model's costs in place of the measured times; `PetalScanStats.SpareTurns` counts the
captures that had the time. `PetalKana.ObserveBand` reports every tile's decided polarity and
glyph for diagnostics. `PetalFrameCells.Kana`, `SetKana`, `KanaPolarityWord` and
`KanaGlyphWord`, `PetalLaneData.Kana`, `PetalRenderer.Render` (the whole card,
`PetalRenderer.CardHeightPx(size)` pixels tall) and the draw list's tiles
(`PetalDrawList.Tiles`; `IPetalCanvas.FillRoundedRectangle` for a light tile's fill and
`IPetalCanvas.StrokePolyline` for the glyph strokes of `PetalDrawList.KanaGlyphStrokes`,
round caps and joins, `KanaStrokeWidth` wide) expose the bands to senders.
`PetalScanSession`, `PetalStreamAssembler` and `PetalRenderer`/`PetalDrawList`
expose the lower layers, and `PetalPng` writes inspection images. Memory and
work are bounded by `PetalDecodeOptions.MaxPixels` (12 MP) and
`PetalAssemblerLimits` (64 KiB payloads, 512 pending atoms). Encoder output and the
software renderer are bit-identical to Rust, turn and kana lanes included, and the
decoder, the turn reader and the band reader repeat the reference's IEEE double
operations in the same order. The shared fixtures `fixtures/petal/petal_stream_v5.json`
and `petal_captures_v5.json` pin both. Run the Petal tests through the same built
xUnit v3 runner:

```bash
dotnet tests/Hyperledger.Iroha.Sdk.Tests/bin/Release/net10.0/Hyperledger.Iroha.Sdk.Tests.dll -method '*Petal*' -noLogo -noColor
```

## Local confidential wallet proofs

`Hyperledger.Iroha.Privacy.ConfidentialProver` owns a clearing native spend key,
accepts an exact `NetworkId` and `ConfidentialAssetId`, and selects the canonical
relation and proving key. Use `using` or `Dispose()`. `ProveTransferAsync` and
`ProveRedemptionAsync` prepare a bounded native job synchronously, consume and clear
the supplied note/tree owners, and run proving on the ordinary thread pool.
Disposing the prover rejects future jobs while an accepted job can finish.
Validation failures before accepting bounded inputs leave those caller owners
available for explicit disposal. Original caller arrays remain caller-owned.

Supply one or two actual `ConfidentialInputNote` values. Choose
`ConfidentialTreeEvidence.Commitments(root, leaves)` or
`ConfidentialTreeEvidence.Paths(root, paths)`; the latter needs exactly one
16-level path per real input, with no dummy path. The native result is
self-verified and checked against the requested root, relation and cardinalities.

Persist the private change amount and rho securely **before** proving consumes a
`ConfidentialChangeNote`. After its leaf index and root are independently
authenticated, reconstruct the change owner and call `ToInput(leafIndex)`.
This uses Core's default change diversifier; reusing a nondefault input diversifier
would produce another owner. `ConfidentialNotes` supplies native-backed default
diversifier, owner, commitment, root and path helpers without managed cryptography.

Run the two-proof disposable example with the current ABI-33 runtime library
available to the .NET loader. `ConfidentialProverException.Code == -101` means
the bridge is missing or lacks the required wallet contract; install the matching
current native artifact for your runtime identifier before retrying:

```sh
dotnet run --project samples/ConfidentialRedemption
```

It uses OS randomness, proves a partial redemption, restores its change opening,
then proves full redemption after disposing the parent prover. These are local
proof artifacts. Roots must come from authenticated protocol state, and a proof
does not submit a transaction or establish ledger authorization. Host execution
does not qualify all five NuGet runtime assets.

`ResolveIdentifierAsync` requires an encrypted input envelope and a typed
`ToriiRamLfeOutputOpening` supplied by the policy's independent opening authority.
The request model has no plaintext input or local encryption fallback. DTO
validation does not authenticate an opening or make a backend available; Torii
enforces signatures, context and its current encryption availability boundary.

The Exact12 fixture codec validates the dedicated proof-free X509 request, original
signed occurrence, ActionStatement projection and materialized statement. Other
protocols retain their own existing route. The retired inline X509 submission is
rejected. `TransactionBuilder.PrepareX509Transaction(admission, requestNorito)` freezes an
otherwise empty builder and the current canonical request in a native opaque owner.
The returned `PrivacyX509PreparedTransactionV1` exposes copied ordered-intent/action
digests, ActionStatement and tagged materialized PrivacyStatement. Its `Sign(seed,
credentialProof)` completes exactly the retained Proof/ActionProof occurrence;
there is no alternative payload parameter. Use `using`/`Dispose` to release custody.

Capability readiness comes only from the actual compiled implementation. Protocol
policy is optional: `PolicyCanonicalBytes` retains bounded current limits and any
scheduled ordinary update. No activation, lifecycle or registration enables use;
retired activation layouts are rejected. Protocol and consensus policy updates may
increase or decrease limits within hard bounds; no-op or stale schedules are invalid.

The sealed SDK admission retains authenticated HTTPS origin and exact network.
The C ABI validates canonical manifests and repeats selected native profile,
governed context, authority, occurrence and proof-container checks; a raw C archive
does not authenticate a Torii service. Native buffers use `iroha_privacy_free_buffer`.
`PrivacyCapabilityUnavailableException` retains the exact selected refusal, including
`JointSecurityReductionIncomplete`; direct native calls retain the same refusal
(status -24). No local fixture, mock driver, or wire codec grants availability or
State authority. TODO: validate this producer with the rebuilt native bridge and
complete holder-prover, authenticated transport, and full-operation integration
once the joint security construction is complete.
