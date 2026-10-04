# Iroha Kotlin SDK

The canonical JVM/Android SDK for Kotlin and Java applications using Iroha 3.

Android KAGEMUSHA uses an app-owned, persistent, nonexportable P-256 approval key
in Android Keystore, backed by TEE or StrongBox. Prefer StrongBox when available;
reject software-only approval keys. Wallet code runs in the app/native runtime.
Embedded applets, OMAPI/eSE monetary services, OEM support or provisioning, and
device-hardware installation are excluded Android paths. Retained OEM/device
service prototypes are unsupported for Android; remaining prototype-only
admission is an ordinary-provider integration defect to fix.

The P-256 approval key is separate from the Ed25519 wallet account signatory S.
Preserve the existing account identity, encrypted key records and activation/
recovery data. Trusted platform storage may admit the existing S key to the
private Native account owner; Native-only key generation, a new vault or account
key rotation is not required. Authentic app/key attestation, separately verified
Play Integrity, exact owner/key/operation binding, Native financial proofs,
durable state/time/replay/recovery and accepted signed release authority remain
mandatory. An ordinary approval key proves no monetary counter, nonforking
journal or trusted hardware clock, and this source guidance grants no device
qualification.

Java-source runtime consumer tests live in `core-jvm/src/test/java` and exercise
this implementation directly. Both Kotlin and Java compilation enforce JDK 8
APIs using the JDK 21 toolchain. Run the Norito consumer suite with:

```sh
./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.norito.*' --console=plain
```

Account and public-key admission requires the ABI-25 `connect_norito_bridge`
native library, including `nativeValidateAccountAddressCanonical`. Address
construction and parsing use Rust to validate every key and complete multisig
policy, then require identical canonical bytes. The V1 identity catalog includes
all eleven algorithms without a process-global curve selector. Missing native
admission returns `ERR_NATIVE_BRIDGE_UNAVAILABLE`; account literals must be exact
I105 strings without surrounding whitespace. For host tests, set
`IROHA_NATIVE_LIBRARY_PATH` to the absolute directory containing the freshly
built bridge. Android packages the bridge through the generated native artifact
pipeline described in `CLAUDE.md`.

Android Native builds require two canonical absolute public originals outside
source and Cargo target directories:

- `-PirohaHardwareBootstrapCompiledBinding=` selects `hardware-compiled-binding.norito`
  from the maintained Native hardware preparation helper.
- `-PirohaOrdinaryContextCompiledBinding=` selects `common-sdk-compiled-root.bin`.
  The sole source/policy producer in `png2-deploy` derives its 76 bytes from the
  independently admitted SDK signer role, actual Iroha source inventory and header ABI.

The recipe rechecks both originals before and after ABI builds and stripping,
holds their descriptors across the hermetic Cargo child, and retains complete
bytes plus SHA256/size in provenance. Actual artifact admission matches the
ordinary original to the independently admitted role/source/ABI. Managed unit
compilation does not load these Native inputs. Neither input grants account,
financial or device authority; Native startup still requires the installed
package, genuine signed context and selected hardware release.

`ValidatorStakingNoritoV1` decodes first-release authority generations, epoch
authorizations, signed all-edge beacon DKG records, committee transitions,
typed registration, bond, withdrawal and slash plans, bounded reward claims with an explicit optional fee-custody
payment, and peer rebinding. Its Rust-authored fixture is
`fixtures/validator_staking/norito_v1.tsv`; the consumer tests also reject
truncated records, malformed peer bindings, invalid withdrawal hash widths or markers,
retired reward-plan layouts, invalid fee custody and
noncanonical quantity decimals. Decoding preserves exact
Norito bytes but does not verify signatures, custody, or committee activation.
Unsigned 64-bit fields retain their complete wire bits in `Long`, including
staking-plan expiry heights; compare them as unsigned values. Collection and
byte-array access returns copies, preserving the original decoded record.

`UpdatePlainConvictionInstruction` exposes the public standalone ballot's
choice-free conviction update to Kotlin and Java callers. It emits the registered
`iroha.instruction.v1::governance::UpdatePlainConviction` Norito frame with exactly
`referendum_id`, `owner`, `amount`, and `duration_blocks`. Construction and
transaction encoding reject direction fields, noncanonical selectors, account
addresses, quantities, durations, and malformed frames. The focused
`UpdatePlainConviction*` Kotlin/Java-source tests compiled on 2026-09-24, but
execution still requires a same-source ABI-25 native bridge for account
admission. This SDK slice does not establish Rust fixture parity or complete
private standalone elections.

## Quickstart: Torii collections and event filters

Every Torii collection (`specs/torii/collection_queries.md`) is read with one
query language and returns one page envelope. `HttpClientTransport` exposes the
collections: `domains`, `accounts`, `assetDefinitions`, `nfts`, `rwas`,
`repoAgreements`, `accountAssets(accountId)`, `assetHolders(definitionId)`,
`transactions` and `accountTransactions(accountId)`. Each one has `page(query)` (one page),
`iterate(query)` (a lazy iterator that follows `next_cursor`; `close()` cancels
the request in flight), `pages(query)` and `fetchAll(query)` (every page,
asynchronously; cancelling the future stops paging). Requests use
`POST <collection>/query`.

```kotlin
import java.net.URI
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.ClientConfig
import org.hyperledger.iroha.sdk.client.HttpClientTransport
import org.hyperledger.iroha.sdk.client.ToriiApiException
import org.hyperledger.iroha.sdk.client.stream.EventFields
import org.hyperledger.iroha.sdk.client.stream.ToriiEvent
import org.hyperledger.iroha.sdk.client.stream.ToriiEventListener
import org.hyperledger.iroha.sdk.client.stream.TransactionEventStatus
import org.hyperledger.iroha.sdk.query.field
import org.hyperledger.iroha.sdk.query.listQuery

val client = HttpClientTransport.createDefault(
    ClientConfig.builder().setBaseUri(URI("https://taira.sora.org")).build(),
)

// One page: filter, sort and page size.
val query = listQuery {
    filter((field("owned_by") eq alice) and (field("alias_binding.bound_at_ms") gt 0))
    sort("-alias_binding.bound_at_ms,id")
    limit(50)
}
val page = client.assetDefinitions.page(query).join()
page.items.forEach { println("${it.id} ${it.alias}") }

// Everything, following next_cursor; quantities are exact BigDecimal values.
client.accountAssets(alice).iterate().use { balances ->
    for (balance in balances) println("${balance.asset} ${balance.quantity}")
}

// Errors carry Torii's envelope: status, code, message and details.
try {
    client.nfts.page(listQuery { filter("owned_by == \"x\" && quantity > 1") }).join()
} catch (failure: CompletionException) {
    val error = failure.cause as ToriiApiException
    println("${error.status} ${error.code} ${error.message} field=${error.field} hint=${error.hint}")
}

// Event streams use the same grammar over event fields; payloads decode to typed events.
client.newEventStreamClient().subscribe(
    (EventFields.TX_HASH eq txHash) and EventFields.TX_STATUS.isIn("Approved", "Rejected"),
    object : ToriiEventListener {
        override fun onEvent(event: ToriiEvent) {
            if (event is ToriiEvent.Transaction && event.status == TransactionEventStatus.REJECTED) {
                println("${event.hash} ${event.rejectionCode} ${event.rejectionReason}")
            }
        }
    },
)
```

Java uses the same API: `Filter.field("owned_by").eq(alice).and(...)`,
`ListQuery.builder().filter(filter).sort("-quantity,id").limit(50).build()`,
`client.domains().page(query)`, and try-with-resources around
`client.domains().iterate(query)`.

- `toString()` of a `Filter` is the canonical text (`owned_by = "alice" and
  quantity >= "10.5"`) and `toJson()` the JSON form; `Filter.parse(text)`
  validates text locally and reports the line and column like Torii. A string
  passed to `filter(...)` is sent unchanged.
- Literals are exact: integers that fit `u64`/`i64` are numbers, `BigDecimal`
  and wider `BigInteger` values become decimal strings. There are no `double`
  overloads.
- Typed rows keep the complete JSON in `row.json`; only the identity fields
  (`id`; `account_id`, `asset`, `scope` and `quantity` for balances) are
  non-null. Use `collection.json()` for `select` projections and aggregates
  (`AggregateSpec`), whose items are plain JSON objects. Aggregates are
  computed where the rows live: a read whose visible rows span several
  dataspace routes is rejected with `invalid_aggregate`; page through the rows
  instead.
- Object and array literals (only valid against `metadata.<key>`) exist only
  in the JSON form. `POST` bodies always carry tree filters as JSON; GET
  parameters (`toQueryPairs()`) and event-stream filters reject them.
- Failures complete the futures (or throw from the iterator) with
  `ToriiApiException`; query errors use `invalid_filter`, `invalid_sort`,
  `invalid_select`, `invalid_aggregate`, `invalid_limit`, `invalid_cursor`,
  `invalid_include_total` and `invalid_query`. Malformed responses raise
  `ToriiProtocolException` (`invalid_response`). Client-side validation raises
  `ListQueryException` with the same `code` before anything is sent.
- Transaction history (`transactions`, `accountTransactions(id)`) is read
  newest first by (`block_height`, `block_index`) and rejects `sort`,
  `include_total` and `aggregate` (`invalid_sort`, `invalid_include_total`,
  `invalid_aggregate`). Each page scans a bounded slice of history, so a page
  can hold fewer than `limit` rows, or none, and still have a `next_cursor`;
  `iterate` and `fetchAll` keep following it until it is `null`. Bounds on
  `block_height` in the filter's top-level `and` also bound the scan
  (`(field("block_height") gte 1200) and (field("result_ok") eq true)` reads
  only that range). `asset_ids` and `asset_definition_ids` are lists matched
  element-wise: `=`/`in` match when any element matches, `!=`/`not in` when
  none does.
- Collection reads are public. Set `ClientConfig.Builder.setCanonicalAuth(...)`
  (with a `LocalSigningContext`) to sign them, which widens visibility into
  restricted dataspaces, or sign one collection with `signedBy(auth)`.
  Signed requests need HTTPS; for a local devnet,
  `setAllowPlaintextLoopback(true)` admits plain `http` to loopback hosts only.

## Local confidential proofs

`ConfidentialProver` owns the spend key in native Core and selects the canonical
transfer/full-redemption/change relation and key automatically. Supply typed
`NetworkId`, a canonical asset-definition address, one or two actual input
notes, and an authenticated tree root. `ConfidentialTreeEvidence.Paths` needs
one 16-level path per actual input; a one-note spend needs no dummy path even
when the tree is full. `ConfidentialNoteCommitment.derive(asset, amount, rho,
ownerTag)` derives a commitment without retaining a spend-key opening.

Run proving on your application's background executor; the methods block:

```kotlin
ConfidentialProver.create(networkId, assetDefinitionId, spendKey).use { prover ->
    val proof = prover.proveUnshield(
        ConfidentialTreeEvidence.Paths(root, listOf(inputPath)),
        listOf(ConfidentialInputNote(amount, rho, diversifier, leafIndex)),
        publicAmount = amount,
    )
    // Public local proof material; choose an implemented protocol admission path separately.
}
```

Proving consumes and closes the note/tree owners on success and failure.
`close()` rejects future jobs; work already accepted by the native owner can
finish. Caller-owned arrays, immutable amounts and other JVM copies remain the
application's responsibility. The proof does not submit a transaction, authorize
value movement, or establish network activation.

Persist the private change opening securely before proving consumes its
`ConfidentialChangeNote` owner. Once the change commitment has an authenticated
leaf index, reconstruct the saved opening and convert it to a later input:

```kotlin
val input = ConfidentialChangeNote(savedAmount, savedRho).use { restoredChange ->
    restoredChange.toInput(authenticatedLeafIndex)
}
// Supply input with authenticated tree evidence to a later proving operation,
// which consumes it; close input yourself if you abandon that operation.
```

Conversion uses Core's default change diversifier, which may differ from the
original input note's diversifier. It copies the opening into an independent
owner; closing the restored change leaves the new input intact. Conversion does
not authenticate the supplied index or establish membership. Do not use a
placeholder leaf index before the commitment has been located and authenticated.

With a same-source rebuilt bridge in `IROHA_NATIVE_LIBRARY_PATH`, run
`./gradlew :core-jvm:confidentialRedemptionExample --console=plain` for a disposable
wallet using operating-system randomness. Native JNI checks are
`./gradlew :core-jvm:test --tests '*ConfidentialProverNativeTests' --console=plain`;
missing exports fail. JVM host evidence does not qualify an Android device or
replace the native AAR packaging/provenance checks below.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM models, codecs, cryptography, clients, and KAGEMUSHA V1 wire support |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager, shared JNI bridge for ML-DSA-65 / KAGEMUSHA flows |
| `org.hyperledger.iroha.sdk:kagemusha-wallet-android` | AAR | KAGEMUSHA wallet integration built on `client-android`; use this artifact for Android KAGEMUSHA |

### Consumer usage

```kotlin
// build.gradle.kts (consumer project)
repositories {
    mavenLocal()
}

// Pure JVM — business logic modules, JUnit tests, server-side
implementation("org.hyperledger.iroha.sdk:core-jvm:0.1.0")

// Android wallet without KAGEMUSHA payments
implementation("org.hyperledger.iroha.sdk:client-android:0.1.0")

// Android wallet with KAGEMUSHA payments
implementation("org.hyperledger.iroha.sdk:kagemusha-wallet-android:0.1.0")
```

`KagemushaWalletV1` is the Android-free aggregate-balance orchestrator. Android
monetary integration requires the ordinary Native/Core/State/Guard provider to
authenticate financial proofs, rollback-resistant state, durable inbox/outbox,
trusted-time admission and atomic recovery. A hardware-backed P-256 app approval
key supplies operation-bound approval; it cannot attest those monetary guarantees. Incoming
payments are acknowledged only after durable staging; duplicate delivery returns the provider's
same durable ACK. Sends and redemptions require the native provider to fold the staged credits
needed to cover the amount; unrelated backlog must not delay an already-covered spend.
`foldPendingCredit()` folds exactly one authenticated mint or peer credit, while
`drainPendingCredits()` repeatedly folds a stable snapshot without a cumulative count limit.
The drain releases the lane after each credit for queued foreground work. Concurrent epoch rotation interrupts the drain; start a new
pass for the new epoch's watermark. Continuous background scheduling remains an integration
requirement. Android uses the ordinary app-owned TEE/StrongBox approval-key path;
StrongBox is preferred and software-only approval custody is refused. The
retained `KagemushaAndroidWalletV1.openProduction(...)` OEM adapter and SDK-to-OEM
operation-16 path are unsupported Android prototypes. Applets, OMAPI/eSE,
OEM support/provisioning and device-hardware installation are excluded. Missing
ordinary-provider integration remains an implementation blocker, not a hardware
inability of stock KeyMint or StrongBox devices.
Staging advances native inbox bookkeeping, not the monetary-state journal. Core's typed mint
reservation/inbox implementation and ordinary-provider integration remain under
validation. A completed MintFold is a separate proved transition, not a staging result.
Managed KAGEMUSHA X25519 types enforce only the canonical 32-byte nonzero wire shape. They do
not perform scalar multiplication or low-order probing; the shared native core authenticates
canonical X25519 elements during object and complete three-message exchange validation before monetary use.
The retained hardware-provider epoch-rotation behavior below is unsupported
Android prototype history and is not an account-key rotation prerequisite.
Logical sequence and durable journal revision are per hardware epoch. Authenticated rotation carries
the full balance and replay root into the exact successor epoch, replaces the device-policy binding,
and resets both counters to zero. `rotateHardwareEpoch()` does not first drain the inbox, so it
remains callable with saturated counters and pending receipts. The native provider must arrange
rollover before counter exhaustion; the managed wallet does not schedule automatic rotation.

### Attestation command

The `tools` application verifies collected Android key evidence using the pure
`core-jvm` attestation verifier. `client-android` owns device/key provisioning.
Run `./gradlew :tools:test :tools:installDist --console=plain`, then
`tools/build/install/iroha-attestation/bin/iroha-attestation --help`.
The repository launcher `scripts/android_keystore_attestation.sh` builds and
invokes that same command. Trust roots, challenge, alias SPKI, snapshot hash and
evaluation time are explicit command inputs. Bundle metadata supplies no trust.
The tool emits a verified JSON record; shared fixture tests are host evidence,
not physical StrongBox qualification.

### Automatic native acceleration

`org.hyperledger.iroha.sdk.gpu.Accelerators` is shared by Kotlin and Java
callers. Construct it with an explicit `Backend`, `disabled()`, or
`loadNative(absoluteLibraryPath)`. Its five batch operations validate dimensions
and canonical BN254 limbs and own their inputs and outputs. The native bridge
selects qualified acceleration automatically and recomputes through the CPU path
when the device declines or fails. `status` describes CUDA availability; it does
not determine whether an ordinary operation succeeds. A disabled or injected
backend may return null when it declines a batch. Native resource errors remain
visible. See the [native bridge contract](../specs/sdk/android/gpu_operator_guide.md)
for artifact and qualification requirements. Ordinary native parity tests do not
qualify CUDA hardware; the JNI completion-receipt gate remains open.

### JNI declaration and export checks

Run `scripts/check_kotlin_jni.py` from the repository root after compiling all
three SDK modules and rebuilding `connect_norito_bridge`. Supply every main
class output with `--classes MODULE=DIR`, where MODULE is `core-jvm`,
`client-android`, or `kagemusha-wallet-android`, and supply the library with
`--library PATH`. `--report PATH` writes the inspected class/library hashes and
method descriptors after a successful check. `--help` describes the arguments.

For Android release inputs, add `--platform android --android-abi arm64-v8a`
(or `x86_64`), `--symbol-tool PATH`, `--symbol-tool-sha256 SHA256`,
`--symbol-tool-size-bytes SIZE`, and `--inspection-output NEW_DIRECTORY`.
The tool path must be the canonical absolute reviewed NDK `llvm-nm` executable;
the checker verifies its exact hash, size and file identity around the fixed
dynamic-export inspection. The library must be an ELF64 shared object for the
selected ABI. The fresh output directory retains the actual argv, clean
environment, stdout, stderr and result before file-drift checks, including
failed attempts. Host mode discovers platform tooling and reports that
inspection as unpinned. Neither mode executes native code.

The check reads class files without reflection or class loading. It requires
JDK 8 bytecode and Kotlin-owned native declarations, checks explicit signing
context and the closed privacy surface, and rejects missing or undeclared JNI
exports, including duplicate implementation namespaces. JNI names follow the
[JVM native lookup rules](https://docs.oracle.com/en/java/javase/21/docs/specs/jni/design.html#resolving-native-method-names).
Matching export names does not prove argument types, receiver semantics, source
build provenance, native execution, or device behavior. Those remain separate
release checks. The current migration still has 55 Android implementation
exports to retire, so this strict export check is not yet a passing release gate.

Android managed consumers run with
`./gradlew :client-android:testDebugUnitTest :kagemusha-wallet-android:testDebugUnitTest`.
The `:client-android:testDebugHostNative` task separately executes all tagged
Kotlin and Java consumers against the rebuilt host library in the
required, absolute `IROHA_NATIVE_LIBRARY_PATH` directory. Missing libraries
fail before execution; a missing native capability fails the test. Its results
are never reused from Gradle's test cache. This host JNI task does not qualify
Android native artifacts, StrongBox, or physical devices.
It covers the software key manager, explicit chain-context codecs, and
coordinator adapter fixtures that require the canonical Rust address validator.
The adapter's scripted endpoints remain mapping controls, not native coordinator
or hardware qualification.

The separate `:kagemusha-wallet-android:testDebugHostNative` task uses the wallet's
main JNI implementation and the same explicit canonical host-library directory.
It checks ABI 25, signer contract 7, actual initial startup phase 1 and selection
revocation phase 5 against an absent independently registered Native root.
Missing symbols or libraries fail; phase 6 is not exercised after phase 1 refuses.
A later Core-open assertion checks the managed startup fence. The task excludes
scripted client test JNI classes, disables result reuse, and is excluded from
ordinary managed tests through its `host-native` tag. It creates no account,
release, monetary or hardware authority and grants no device qualification.

### Java transaction metadata

`JsonValue` is one immutable Kotlin-owned type for both JVM languages. Its
`string`, `number`, `bool`, `nullValue`, and `parse` factories are callable from
Java. Parsing normalizes JSON; signed-wire decoding still rejects alternate
lexical forms. `TransactionPayload` copies metadata and exposes an immutable
map. Use `JsonValue.nullValue()` for JSON null; a Java null value is rejected.

### Transaction identity

Every ordinary `TransactionPayload` requires a nominal, immutable `NetworkId` parsed from the
exact canonical checksummed 32-byte genesis-header hash literal. The Norito codec emits it only as
`TransactionDomain::Network`; the genesis-only domain is not constructible through the SDK and is
rejected while decoding. JSON transaction surfaces use
`"domain":{"kind":"network","value":"<canonical NetworkId>"}` and reject the retired `chain`,
`chainId`, and `chain_id` identity fields. Security-sensitive SDK payloads and canonical HTTP
signatures use this exact `NetworkId`; human-readable deployment labels are display-only and are
never accepted as a signing domain.

`SignedTransactionHasher` computes the first-release external transaction ID
from `TransactionEntrypoint::External` plus the canonical signed
`TransactionPayload`. Authorization signatures and multisig proofs remain in
the submitted `SignedTransaction` wire but do not create alternate IDs for the
same intent. Proof attachments are carried by `TransactionPayload.attachments`,
so adding, removing, or replacing an attachment changes the signature preimage
and transaction ID.

### One-shot signed HTTP requests

Signed transactions, signed queries, transaction batches, and every request carrying an Iroha
nonce are dispatched at most once. The default OkHttp transport does not follow 307/308
redirects or retry connection/status failures. Custom `HttpTransportExecutor` implementations must
honor `TransportRequest.replayPolicy`: only unsigned, bodyless `GET`, `HEAD`, and `OPTIONS` requests
are `RETRY_SAFE`; all other requests are `ONE_SHOT`.

Raw `witness_base64` body authentication is not an SDK surface. Multisig writes must use a
canonical signed transaction or a closed typed signed intent.

`prepareContractCall` requires `ToriiCanonicalRequestAuth` for the same authority
and a configured `LocalSigningContext`. It signs the exact prepare request once
and accepts only the exact canonical contract payload. Keep the returned
payload bytes and quoted fee unchanged when signing and submitting through the
transaction API; submission failures are reconciled by transaction hash.

`prepareContractCall` accepts a draft receipt only when `payload_digest_hex` is
the exact lowercase BLAKE3-256 digest of the canonical UTF-8 JSON request
payload. An omitted payload hashes the empty byte sequence; noncanonical hex or
a digest mismatch fails closed before the draft is returned.

The canonical transaction payload has nine fields: domain, authority, creation
time, executable, TTL, nonce, fee payment, metadata, and attachments. Retired
admission fields or extra binary slots are rejected, including with a matching
recomputed payload hash. Sign the verified payload without changing any signed field.

Canonical request builders keep I105 as the semantic SDK identity but emit its lowercase
canonical-hex address in `X-Iroha-Account`, which is safe on strict ASCII HTTP stacks. Active
canonical ASCII aliases are emitted unchanged. Signed JSON `account_id` fields retain the caller's
exact canonical spelling: I105 remains I105, and a canonical body-auth alias remains unchanged.
Alias inputs must already use the exact canonical `label@dataspace` or
`label@domain.dataspace` lowercase-ASCII shape. The signer applies only bounded
structural preflight; Torii remains authoritative for UTS-46, active bindings,
and controller verification.
The first-release signing domain is an exact genesis-derived
`hash:<64 uppercase hex digits>#<4 uppercase CRC-16 digits>` `NetworkId` whose decoded 32-byte
value carries the V1 marker bit. Canonical nonces contain 1--256 visible ASCII bytes. Methods are
non-empty ASCII HTTP tokens of at most 32 bytes, and URI signers require an exact root-relative
ASCII raw path of at most 64 KiB; absolute inputs must be hierarchical HTTP(S) URIs with an
authority and no fragment.

Sora VPN receipt submission returns the native `SettleVpnLease` instruction
with exact status `settlement_pending`. That status remains provisional until
the instruction commits; only a receipt read from committed WSV state uses
`settled`. The parser also retains the exact `disconnected`, `expired`, and
`replaced` lifecycle values.

Encrypted RAM-LFE is unavailable: the diagnostic exact-lift BFV profile must be
replaced before production activation. `RamLfeExecuteResponse` represents
ciphertext and an execution receipt; it never supplies a plaintext opening.
Identifier requests require an independently authenticated opening bound to the
same execution. The parser rejects the retired execute `output_opening` field.
Local `encryptInput` and plaintext request factories throw
`RamLfeEncryptionUnavailableException` with code `ram_lfe_encryption_unavailable`
before processing input. Seed overrides and the insecure encryptor are absent
from the shipped JAR. JSON and Norito accept only the three exact backend tags
and `signed`/`proof`; accepting metadata does not activate either encrypted backend.

Identifier resolve/claim-receipt and RAM-LFE execute/receipt-verify calls require a per-call
`ToriiCanonicalRequestAuth` and `ClientConfig.localSigningContext`. The auth context accepts a
`RequestSigner` callback; applications retain ownership of software, hardware, or remote keys.
Use `RequestSigner.ed25519(privateKey)` for a JCA Ed25519 signer. The SDK builds the exact
canonical message, rejects empty, all-zero or oversized callback signatures, and propagates
signing failures without retrying. Auth contexts do not expose private-key properties.
The two-argument `ToriiCanonicalRequestAuth(accountId, signer)` generates a
fresh timestamp and nonce for every request. An explicit `timestampMs`/`nonce`
pair is single-use: a second signing attempt fails locally with
`IllegalStateException` instead of sending a replay that Torii would reject.
The transport signs the exact
POST path and body once, rejects caller-supplied canonical headers, and requires a claim-receipt
path account to be the same exact canonical I105 account as the signer.

Nearby's `IrohaPeerNearbySessionV1` owns the authenticated IPM1 boundary for Kotlin and Java.
`seal` accepts an `IrohaPeerWireMessageV1`; `open` returns a verified message for the session's
profile. Ciphertext authentication and complete IPM1 decoding must succeed before the receive
sequence advances. Encode the encrypted record for the Android radio transport and decode it
before calling `open`. Temporary plaintext buffers are wiped, and closing the session destroys
its owned keys. Radio/device lifecycle qualification is separate from the host protocol tests.

Nexus app requests and results are immutable values. Constructors snapshot keys, payloads,
metadata and scopes; byte accessors return owned copies and collections reject mutation.
`NexusSignableTransaction(payloadBytes, authority, signingPublicKey)` derives `payloadHashHex`
from its exact owned payload. Nexus uses only the canonical `ed25519` algorithm. Create a new
value to change authority or signing context. Transfer receipts snapshot nested JSON status
and bind their hash to the exact signed transaction; admission alone does not establish Applied.

If `submitTransaction` cannot obtain an authoritative admission result, it fails with
`AmbiguousTransactionSubmissionException`. Use its `hashHex` or `reconcileWith(client)` to query
pipeline status. The exact binary endpoint accepts only HTTP `202`; any other non-ambiguous HTTP
response fails with `TransactionSubmissionHttpException`, retaining the hash, status, reject code,
and bounded response detail. Never resend the same signed bytes. `RetryPolicy` applies only to
caller-managed replay-safe reads, and configured pending queues are explicit local staging:
submission neither fills nor drains them.

Public pipeline status contains only the canonical transaction hash—exactly
`[0-9a-f]{63}[13579bdf]`, including the Iroha `HashOf` marker—closed status kind,
optional committed height, read scope, and resolution source. The parser rejects rejection
text, diagnostics, trigger completions, batch outcomes, unknown kinds, and noncanonical
metadata. Status scope is exactly `local` or `global`; `auto` is not a first-release value, and
`waitForTransactionStatus(...)` always requests `global`. Transaction-hash request values,
status responses, and Torii receipt headers are never trimmed, case-folded, prefix-stripped, or
decoded from byte-shaped values. Status reads accept only an exact HTTP `200` envelope or `404`
not-found response; `202` and `204` are protocol errors. State-resolved `Rejected` and `Expired`
are the only failures; every other non-success status remains progress. Negative polling
intervals and timeouts are rejected rather than clamped. Detailed transaction reads require an involved account or operator to send a
one-shot canonical signed `FindTransactions` query bound to the exact genesis-derived
`NetworkId`; Kotlin intentionally exposes no details helper until its generated signed-query
surface supports that contract.

### DA commitment and pin-intent proofs

`HttpClientTransport.newDaToriiClient()` returns the typed DA client. It covers
the proof-policy, commitment list/prove/verify, and pin-intent
list/prove/verify routes. DA digests use `DaModels.Digest32`; proof counters use
`BigInteger` so the full unsigned 64-bit wire range remains exact.

```kotlin
val da = transport.newDaToriiClient()
var page = da.listPinIntents(
    DaModels.PinIntentListRequest(limit = BigInteger.valueOf(100)),
).join()
while (page.nextCursor != null) {
    page = da.listPinIntents(
        DaModels.PinIntentListRequest(
            limit = BigInteger.valueOf(100),
            cursor = page.nextCursor,
        ),
    ).join()
}
val proof = da.provePinIntent(
    DaModels.PinIntentQueryRequest(
        storageTicket = DaModels.Digest32.fromHex(ticketHex),
    ),
).join()
if (proof != null) {
    check(da.verifyPinIntent(proof).join().valid)
}
```

List routes use immutable, server-issued snapshot cursors and return an explicit
nullable `nextCursor`. Proof routes use separate selector-only request types;
pagination fields are not accepted by proof requests.

Responses are decoded into closed typed models. The client rejects unknown
fields, malformed transparent byte wrappers, invalid checksummed hashes,
contradictory verification responses, and Merkle paths whose direction/length
does not match the advertised bundle location. Requests are capped at 64 KiB
and buffered responses at 8 MiB.

### Native Sumeragi status

`HttpClientTransport.getSumeragiStatus()` reads `GET /v1/sumeragi/status` into
the closed protocol-1 `SumeragiStatus` model. Its current-round, footprint and
same-applied-cut beacon observations do not confer finality authority.

```kotlin
val status = transport.getSumeragiStatus().join()
check(status.protocolVersion == 1)
println("height=${status.height} view=${status.view} leader=${status.leader}")
```

`SumeragiStatusWire` encodes the same model in a complete canonical Norito frame.
Bare payloads and obsolete status layouts are rejected. Kotlin and Java share
the Rust-generated JSON/Norito corpus at `fixtures/sumeragi/native_status_v1.tsv`.
Every JSON `u64` remains lossless as `BigInteger`. Responses are capped at 1 MiB
and require an exact JSON content type, canonical matching `Content-Length`
when supplied, fatal UTF-8 and closed fields and tags.

`getSumeragiLanes()` parses the operator route `GET /v1/sumeragi/lanes`
(`specs/sumeragi_lanes.md` §8) into `SumeragiLaneStatus`. Each record requires
its committed `daLayout` RS16 geometry, including the encoding and resource bounds.
Lane observations do not confer finality.

### KAGEMUSHA peer transports

`KagemushaNoritoV1` is the canonical KAGEMUSHA wire codec. Kotlin/JVM and Android
encode the same three-message payment exchange—direct request, post-commit proof-bearing
payment, and durable acknowledgement. Each request binds one exact amount and a fresh
recipient encryption key; distinct valid payments against a reusable request are accepted.
Mint authorization, mint credit, and redemption vouchers are separately framed;
`kgm1:` is the sole text transport. Exposed credits cannot be cancelled. QR, NFC, and Nearby consume
`../fixtures/offline/kagemusha_v1.json`. Public wire
size and verification work are independent of balance history; no hop, input,
origin, ancestry, fan-in, or proof-depth limit is encoded.

Before requesting, sending, minting, or redeeming offline value, the app must durably
save a fresh nonzero 32-byte operation identity and its exact action parameters, then
pass that identity to the corresponding reservation and execution calls. An identical
retry retains the same identity; a lost native return must never cause the app to
allocate a replacement. The authenticated provider rejects a substituted reservation
identity before executing a device operation. Payment and redemption reservations
carry the canonical tagged `iroha.kagemusha.device.v1.sender-public-inputs` Norito
archive, shared with the native outgoing-operation index.

Authenticated provider construction requires an app-owned `KagemushaOperationIntentStoreV1`
and an explicit current onboarding-approval callback. The store must synchronously retain
immutable account/runtime scope, operation identity, exact typed command, and creation
qualification before dispatch; after Core accepts the response it retains the exact reply,
original authenticator, and reply qualification. Its shared reentrant lock and durable storage
must exclude multiple owners across providers and processes. There is no in-memory default.
The SDK resumes unfinished bootstrap and rotation commands before another transition.
Rotation retries use their retained original response key, while fresh qualification and state
reads use the current epoch. Coordinator method 11 begins each transient native observation
for operations 1, 13, 18, and 21; those reads never enter the operation intent store or durable
reservation method. Native recreation requires a fresh challenge, and each new challenge
invalidates its predecessor. Historical read replies are never treated as current state.
Bootstrap admission is checked again immediately before
dispatch, including a retry when fresh authenticated state remains uninitialized.

Apps call `acknowledgeDurableResult(operationId, canonicalResult)` only after syncing and
reopening the exact request, payment, acknowledgement, or redemption transcript. Receiver ACK
completion uses its credit identity. Internal transitions are acknowledged after a fresh
authenticated snapshot proves the accepted aggregate installed; its exact command, challenge,
reply, authenticator, and qualification are retained as immutable evidence on that mutation
before acknowledgement. Sender installation retains the same evidence on its operation-10
record. Such evidence never restores a read challenge.
Acknowledgement retains immutable history; errors, sign-out, and lost approval must not delete
uncertain obligations. Acknowledged history is not charged against a lifetime operation limit.

`KagemushaCoreCoordinatorBridgeV1.open(storagePath)` in `client-android` provides
the strict schema-2 JNI transport, backed by the pure `core-jvm` frame codec.
It checks the complete ABI-25 inventory and rejects substituted response bindings;
missing JNI or an absent qualified native coordinator fails closed.
Any failure after dispatch revokes the process-local handle before another JNI
call. Uncertain monetary state remains the qualified backend's responsibility.
Incoming staging uses native method 17 to select original reserve-mint, staged-mint or
staged-peer material; the managed caller supplies only the kind and credit identity.
Method 15 durably retains the native fold intent and its genuine original paired proof,
then returns the bounded public hardware work. The authenticated provider requires an
explicit `KagemushaIncomingFoldEvidenceProviderV1` before preparation or device work.
That source rechecks its original physical custody before and after obtaining the exact
State Guard and distinct device root-selection signature. Method 16 receives the held
proof unchanged and independently verifies those originals before publishing funds.
Exact retries recover the same history identity, proof and physical evidence; host
operation storage, generic device operation 17 and Core/software signatures cannot
substitute. A fresh authenticated wallet observation follows native completion.
The current closed schema-2 inventory contains all 21 coordinator methods.
Method 18 reads the installed native release ID, hardware-policy digest and provider
policy registry root from the original selected Core owner under its observation
lease. The policy digest remains bound to qualification signatures; aggregate state
is checked against the separately authenticated provider root.
`KagemushaNativeCoreCoordinatorAdapterV1.open(storagePath)` implements the typed
wallet coordinator over that transport. Its pure `KagemushaCoreCoordinatorArchiveV1`
codec handles bounded canonical preparation, candidate, recovery, and redemption
receipt projections. The adapter checks public-input digests, operation identities,
qualified creation context, retained recovery scope, and installed aggregate scope.
`KagemushaAndroidOrdinaryEnrollmentV1` composes the same adapter's Native C21
reservation, authenticated signed preparation, persistent generated hardware
P-256 key, raw admission, E20 possession, governed Play Integrity request and
credential admission. Native C20 phases 9–14 then retain the FI challenge,
wallet invocation and exact original wallet signature before the protected FI
finish request. The product constructor requires one opaque same-coordinator
current Native wallet selection: activated protected HTTP signatory S matches
selected S, and reserved wallet W matches selected W. Genuine W is distinct and
contains exactly one S member, weight 1 and threshold 1. Native C20 phase 10
supplies the retained account signature from its actual held member key; the
superseded public managed-wallet-signer constructor is removed. Hardware P-256
app approval and protected HTTP request custody remain separate.
The Android HTTP store retains required stable W plus original S/device/runtime/
request key/ID/kind/body/reply, checking that same private selection before commit,
HTTP retries and cached exposure. A session handle is not persisted authority;
missing or changed W refuses without rewriting, migrating or clearing old records.
The app serializes one original Bootstrap publication/retirement handoff and marks
completion only after the callback and owner checks return. Current-FI retries skip
that completed Bootstrap and retain their original request/signature/body.
All eight ordinary backend enrollment/Integrity routes now have the certified
customer W/S join applied in source: the genuine current World must authenticate
both exact account rows while HTTP/device recovery/CAS retain S. Backend
library compilation and three focused cases passed on the original cut; its HTTP
executable failed with eight errors before tests. Subsequent signed-policy joins
were reviewed and the exact evidence-byte borrow repair is applied. A fresh
preparation stopped before compilation on two changed Native test files. Current
HTTP cases and genuine installed issuer/runtime validation remain pending.
Genuine installed signed inputs and shipping Native account construction remain pending. Selected managed checks passed 58 SDK, 20 Core and
7 app cases per variant across retained source cuts; these checks grant no Root,
hardware, monetary, release or roster qualification.
Explicit retries reuse the same complete Google and wallet originals; a retained
wallet invocation without a signature requires Native recovery. The explicit
Google invalid-provider error permits a new warmup only on the next user action.
`beginOrResumeBootstrapApproval()` first completes that same FI ceremony, then
uses the separate method-19 phase-8 Bootstrap capability bound to the original C, enrolled
alias/key and retained FI credential. Its selector is SHA-256 of ASCII
`iroha:kagemusha:v1:ordinary-bootstrap-operation-id` followed by a NUL byte and
the complete original FI certificate bytes. Native independently checks that
selector before preparing the zero-index S and exact W. The platform signs only
after the Native durable fence; retries consume retained DER or recover the exact
original receipt without signing again. Generic monetary approval still rejects
Bootstrap. Returned enrollment and approval originals grant no money permission;
genuine State/Guard publication and the ordinary Native monetary owner remain
separate requirements.
`beginOrResumeInitialStatePublication()` then uses that same captured Bootstrap
ticket with method-19 phase 9. Native selects the authenticated initial-proof
profile and artifact resolver from its already installed source. Before any
Guard or State proof work, the original logical journal fsyncs an initial
publication intent bound to the same ticket, FI, credential and complete signed
Bootstrap approval. Its single-use Native permit checks the exact owned journal
prefix. Cold replay retains the intent without creating another permit. A cold
captured approval without an intent may resume its first publication; a surviving
intent selects only exact original publication recovery. A missing or damaged
publication after that intent freezes, with no fresh-proof fallback. Recovery
across every interruption cut remains unfinished.
The request carries only the phase and original ticket; no host proof, verifier,
profile or resolver can be admitted through the frame. Phase 10 only recovers and
rechecks an existing publication and never generates another proof or signature.
Both phases return the ticket, enrollment ID and SHA-256 digests of the complete
canonical publication, FI certificate, app credential, signed Bootstrap approval,
initial State, paired State proof and paired ordinary Guard. The managed holder
binds these to its original FI completion, app credential and ticket, and requires
every retained commitment to remain unchanged on recovery. A lost Native return
freezes the old holder; only actual Native original recovery in a fresh holder can
continue. `KagemushaOrdinaryInitialStatePublicationOriginalsV1` is detached
acknowledgement data with defensive copies and no monetary capability. Missing
authentic proof material remains unavailable, and ordinary monetary operations
still require their separate verified Native provider.
`KagemushaOrdinaryCashApprovalProjectionV1` checks copied public W325/S460 originals
through separate preparation and terminal entry points. Preparation requires signed
purpose 2 and zero candidate/body commitments; terminal requires signed purpose 1
and both commitments exactly for send/redemption. Both require exact-next unsigned
128-bit logical indices and SHA-256 of the complete original S460 bytes, including
its existing domain and length once. Retained ordinary public bindings must match
the full S and original operation/account/authority/key/credential/Guard fields;
S's credential ID must equal W's enrollment digest. These projections authenticate
no issuer, platform signature, current lease, State/Guard proof or monetary owner.
The ordinary cash method-19 producer and current-money holder remain separate work;
the projection does not reuse the Bootstrap capability or OEM enrollment.
`KagemushaOrdinaryTransitionStatementProjectionV1` accepts the existing full model
digest preimage: BE64(40), its NUL-terminated transition domain, BE64(1089), and
the 1089-byte body with little-endian integers. It hashes all 1145 original bytes
once, matches S's transition digest and operation, and correlates common release,
network/lane/profile/policy scope. It provides no alternate encoder or reconstruction
from the recursive State's 93 public cells; Native must supply the complete retained
model original and authenticate both State/Guard parities and current custody.
The product supplies protected HTTP and wallet signing after the trusted Native
account/release source is installed. Trusted platform intake can supply the actual
existing Ed25519 S key to that private Native owner while preserving account
identity, encrypted records and activation/recovery data. Native-only RNG, a new
vault and account-key rotation are not prerequisites. Public-key DTOs,
caller-supplied private-key frames and managed signing callbacks do not establish
Native custody. Genuine app/key attestation, separate Play Integrity verification,
operation binding, Native financial/state/time/replay/recovery proofs and the exact
accepted signed release remain required.

The following OEM method-12/device-service details are retained unsupported
Android prototype history. They are not an ordinary Android enrollment,
recovery, release or device qualification path; applets, OMAPI/eSE, OEM support/
provisioning and device-hardware installation are excluded. Any active
prototype-only admission must be replaced with the ordinary app-key provider.
The retained adapter exposes `initialEnrollment()` for its bounded OEM method-12 ceremony.
It retains one phase-1 selection only in the original process. Phase-7 rechecks
that exact selection, and phase-4 reads its original possession proof, while the
same native owner remains live. A failed dispatch or response publication revokes
that owner and prevents readback or cancellation retries. Successful cancellation
retries retain the original ticket. Closing the native owner revokes cached enrollment phases before
another account can use them. These typed frames do not install a qualified backend.
For a completed owner selected by the independently installed native provisioner,
`recoveredEnrollment()` uses method-12 phases 9–11. It retains the exact canonical
account challenge and native 32-byte signing message, checks the device request
nonce and exact canonical operation-1 read command, and submits the original Ed25519 account signature and complete signed
device response. Exact retries preserve the original deadline and proof. Native
Core authenticates its retained checkpoint, journal, hardware and owner before
granting only an observation lease; application caches never restore that authority.
Recovery rejection never falls back to initial enrollment. Closing the adapter
revokes both initial and recovered scopes.
Device-reply admission retains the original 64-byte response authenticator so
native Core independently verifies the exact response transcript.
The sole JNI verifier is `nativeVerifyCommandResponseV1`: its response signature
binds the exact canonical command body digest as well as the response header and
payload digest, hardware policy, and qualification report. The Android bridge
captures the dispatched command and request identity before execution and passes
those detached bytes to verification; no old verifier symbol or overload remains.
Those prototype projections remain selectors; they supply no Android monetary
or device admission. The ordinary Native/Core/State/Guard provider must
authenticate journal/replay state, release, Core authorization and actual recursive
proofs independently of app-key signing. Completing that provider is unfinished
software integration; OEM or applet installation is not a prerequisite. Software-only
P-256 approval keys remain rejected. See [the retained device-bridge source contract](../specs/kagemusha_device_bridge_v1.md).

Online reserve top-ups use the same payer authority as the debit. Build one
`TopUpKagemushaV1Instruction` from the proof-bearing request, put that sole
instruction in a transaction, and sign it with `TransactionBuilder`.
Send the resulting `SignedTransaction` and the request's exact
nonzero 32-byte `operationId` through
`KagemushaToriiClientV1.submitTopUp(...)`. The client posts the canonical
versioned signed-transaction bytes unchanged to `/v1/kagemusha/top-up` and
uses the lowercase operation ID as `Idempotency-Key`; there is no unsigned or
request-only top-up overload. The embedded request ceiling is 16 KiB so both
maximum-size recursive proof parities remain usable.
`KagemushaToriiClientV1.getOperation(...)` returns `null` only for an exact
Torii `kagemusha_operation_not_found` 404 whose JSON body and
`X-Iroha-Reject-Code` header agree. Other 404 responses fail; an absent
operation permits only byte-identical retry of the retained signed transaction.

### Petal Stream optical transport

Petal Stream (`org.hyperledger.iroha.sdk.offline.petal`) is the animated
"streaming QR" used to hand an offline payload such as an `IPM1` peer message
from one screen to a phone camera. Each square frame shows four sakura-blossom
finders, a `天`-shaped field of 256 tiles (light/dark polarity, lane `P`;
katakana glyph, lane `K`) and three dotted rings (lane `D`). Every lane is one
whitened GF(256) Reed–Solomon codeword carrying fountain-coded 16-byte atoms,
and every fourth frame's lane `D` carries the stream beacon, so any readable
lane of any frame helps and lost frames only cost time. The Kotlin port is
pure JVM in `core-jvm` and follows the normative Rust crate
`crates/iroha_petal`; encoding is bit-identical to it.

```kotlin
// Sender: frame n of the endless stream, drawn natively or in software.
val encoder = PetalStreamEncoder(payload, kind)
petalStreamView.setStream(encoder)           // client-android View, 8 fps by default
petalStreamView.start()
val list = PetalDrawList.of(encoder.cells(n)) // vector shapes for other canvases
val rgb = PetalRenderer.render(encoder.cells(n), PetalRenderOptions(1024, 3))

// Receiver: camera luma planes into one session; the payload arrives once.
val session = PetalScanSession()
val luma = PetalLumaAdapter.fromYPlane(y.buffer, y.rowStride, y.pixelStride, width, height)
session.push(luma, SystemClock.elapsedRealtime()).completed?.let { deliver(it.payload) }
```

`PetalDecoder.decode` locates the finders, tries four rotations and mirrored
front-camera previews ranked by the ring gates plus the `天` silhouette, reads
lane `D` first and turns low-confidence cells into Reed–Solomon erasures; a lane
is only reported when its codeword checks out. When a thumb, a glare or the
frame edge hides one corner blossom, three blossoms forming a corner still
locate the code: the fourth corner is inferred, moved to where the dotted rings
line up, and reported as `PetalDecodedFrame.inferredCorner` (canonical index).
`PetalScanSession` reads the frames after a decoded one by `PetalDecoder.track`,
which follows the last pose (at most 500 ms old) instead of searching the whole
image, and counts both in `stats().tracked` and `stats().inferred`.
The tile lanes `P` and `K` are read in two ways: the *level read* judges every
8×8 tile patch against the light and dark levels measured at the finders, and a
lane it cannot decode is retried with the *normalised read*, which rescales each
patch and each template by its own contrast and erases tiles that lost it, so
over-exposure, veiling light, glare and shadows cancel out (it only runs when a
tile lane is missing). Scanners should still ask for about 1280×720 analysis
frames, set exposure compensation to about −1 EV and fall back to 640×480 (lane
`K` unreadable) only when the device cannot sustain 5 decoded frames per second;
see `PetalLumaAdapter` and `specs/petal_stream.md` §8.
`PetalStreamAssembler` and `PetalScanSession` bound pending atoms, payload
size (64 KiB by default) and stream lifetime (30 s idle, 180 s absolute), and
deliver a payload only after its CRC-32C matches the beacon. `client-android`
adds `PetalStreamView`, `PetalCanvasRenderer` and the dependency-free
`PetalLumaAdapter` (CameraX `ImageAnalysis`, Camera2 `ImageReader` and Camera1
NV21 previews); they use API 19 or older platform calls.

The tests check every section of `../fixtures/petal/petal_stream_v1.json`,
decode the golden camera captures of `../fixtures/petal/petal_captures_v1.json`
with exactly the lanes and inferred corners the reference reads, and track its
golden frame pairs:

```bash
./gradlew :core-jvm:test --tests '*Petal*' --console=plain
```

### Fee quotes and sponsorship

Every transaction payload requires a typed `FeePaymentIntent`. Select the
authority directly, or bind sponsorship to one exact on-chain program and
immutable revision:

```kotlin
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId

val authorityPaid = FeePaymentIntent.authority(emptyList())
val sponsored = FeePaymentIntent.sponsor(
    FeeSponsorProgramId(sponsorAccountId, "wallet_payments"),
    3,
    emptyList(),
)
```

The empty charge-limit list is only the initial quote draft. Freeze the complete
unsigned payload, include `fee_payment = requested.toJsonMap()`, and call
`HttpClientTransport.quoteFees(unsignedPayload, canonicalAuth)`. Verify that the
response preserved the payer, exact program/revision, and gas bound; replace
only `fee_payment` with `FeeQuoteResponse.intent`, then sign and submit that same
payload. Use `getFeeSponsorProgram(programId, canonicalAuth)` to inspect one
exact lifecycle record before selecting its revision. Contract/IVM drafts must
include a positive gas bound in the intent.

The metadata keys `fee_sponsor`, `gas_asset_id`, and `gas_limit` are retired and
rejected. A sponsor rejection never falls back to charging the authority.

### Atomic mixed executable batches

Use `Executable.batchBuilder()` when one transaction must interleave native
instructions and deployed-contract calls:

```kotlin
val executable = Executable.batchBuilder()
    .addInstruction(registerInstruction)
    .addContractCall(
        ContractInvocation(contractAddress, expectedCodeHash, "apply", argumentRecord),
    )
    .addInstruction(transferInstruction)
    .build()
```

The item order is canonical and the node applies the whole batch atomically.
Empty batches are rejected. Contract addresses must be canonical lowercase V1
Bech32m literals, and any batch containing a contract call needs one positive,
signature-bound gas limit in its `FeePaymentIntent`; these constraints are
checked before the payload can be encoded or signed.

### Native asset-lock cancellation

`CancelAssetLockInstruction` implements the V1 compare-and-cancel contract.
Supply the exact application lock ID and the positive canonical Quantity read
from finalized ledger state:

```kotlin
import org.hyperledger.iroha.sdk.core.model.instructions.CancelAssetLockInstruction

val cancel = CancelAssetLockInstruction(
    lockId = "appeal:case-42",
    expectedRemainingAmount = "20",
)
val instructionBox = cancel.toInstructionBox()
```

The typed constructor derives the native `EscrowId` with Blake2b-256 and emits
only `escrow_id` plus `expected_remaining_amount`. The instruction pair uses
the canonical `iroha.instruction.v1::escrow::CancelAssetLock` wire ID; its
payload frame retains the concrete `iroha_data_model::isi::escrow::CancelAssetLock`
Norito schema name. The lock-ID
preimage must be nonempty exact text without surrounding whitespace or a BOM
and is bounded by `CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1`
(4,096 UTF-8 bytes, not characters); the on-wire `EscrowId` remains 32 bytes.
The retired one-field shape, aliases, extra fields, zero, and alternate numeric
spellings are rejected; stale expected amounts are rejected atomically by the
ledger.

### Torii server-sent events

`HttpClientTransport.newEventStreamClient()` signs with the client-wide
`ClientConfig.canonicalAuth()` when one is configured; otherwise its requests
remain anonymous and public-only. It inherits the HTTP client's base URI,
default headers, and observers. Use `newEventStreamClient(canonicalAuth)` with
a configured `LocalSigningContext` for a specific account identity.
`subscribe(filter, ToriiEventListener)` reads `/v1/events/sse` with a filter in
the collection-query text grammar over event fields (`EventFields.TX_HASH`,
`TX_STATUS`, `BLOCK_HEIGHT`, `PROOF_BACKEND`, ...) and decodes each payload into
a `ToriiEvent`: `Transaction` (`TransactionEventStatus` plus
`TransactionRejectionCode` and the public `rejectionReason` when rejected),
`Block` (`BlockEventStatus`, block `rejectionCode`), `Warning`, `Witness`,
`ProofVerified`/`ProofRejected`/`ProofPruned`, `DataChange` (`DataEventKind`
plus a diagnostic `summary`) and `Other`. Payloads with an unknown `event` or
status arrive as `ToriiEvent.Unknown` and never fail the stream; a terminal
`stream_error` frame reaches `onStreamError`. `openEventStream(filter,
listener)` delivers the raw `ServerSentEvent`s instead
(`ServerSentEvent.toriiEvent()` decodes one), and text filters can also be
passed with `ToriiEventStreamOptions.Builder.setFilter(String)`. The client
generates all four canonical headers after path resolution and option-query
assembly, so the signature is bound to the exact final URI;
precomputed or partial canonical headers are rejected before dispatch. Frames
follow the SSE specification: an event cut off by the end of the stream is
discarded and lines are bounded. `ToriiEventStreamOptions.timeout` bounds the
idle time between bytes (default 45 seconds, above Torii's 15-second heartbeat);
a stream has no total lifetime. The
canonical `/v1/events/sse` and `/v1/contracts/events/sse` feeds are live-only
and have no replay log. `ToriiEventStreamClient` therefore rejects every case
variant of `Last-Event-ID` before dispatch for exactly those two paths; custom
streams that provide replay may still receive the header through
`ToriiEventStreamOptions`.

Raw listeners receive terminal `event: stream_error` frames. Call
`ServerSentEvent.terminalStreamError()` before application-event projection to
obtain a strict `ToriiStreamException` containing the stable code, server
message, optional unsigned dropped-message count, replay flag, and raw JSON.
Malformed or schema-expanded terminal envelopes fail closed as
`ToriiStreamProtocolException`; they must not be filtered as unrelated events.
A reconnect to either canonical feed starts a new live subscription and can
have a gap.

### Native privacy bridge

`PrivacyNativeBridge` exposes local build metadata only.
`compiledProfileCatalogV1()` returns this binary's canonical typed
`PrivacyCompiledProfileCatalogV1` Norito archive, and
`protocolsV1()` exposes the closed `ProtocolIdV1` enum in exact wire order. The
generic proof request/build/verify ABI and free-form algorithm selectors are
absent; proofs must use protocol-specific typed APIs. The local catalog never
establishes activation or readiness; proof submission requires a fresh
committed `/v1/privacy/capabilities` manifest from live Torii.
`HttpClientTransport.getPrivacyCapabilities(canonicalAuth)` performs a one-shot
authenticated HTTPS fetch for `ClientConfig`'s immutable local network, verifies
the exact response URL and bounded Norito body, and native-validates its signatures
and deployment network before privately binding its origin. Public archive decoding
is inspection-only and cannot mint admission. Native construction validates the
selected activation and limits; the transaction encoder also requires the token's
network to equal the enclosing transaction network. Java consumers use this same
Kotlin-owned boundary.

Genesis `confidential_features` and `zk_policy_hash` values are opaque consensus
fingerprints, never client-side proof or backend selectors.
`ClientConfigManifestLoader` rejects those keys (and their camel-case aliases)
at any depth. Proof construction must use Torii's committed
`/v1/privacy/capabilities` response and the on-chain verifying-key registry.

`PrivacyExact12FixtureCodecV1` decodes the first-release
`PrivacyExact12FixtureBundleV1` entirely in Kotlin; it does not load the native
bridge. Pass one canonical standard-Base64 line (without the fixture file's
final LF), or decode raw Norito bytes directly:

```kotlin
val bundle = PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(fixtureLine)
val canonicalArchive = PrivacyExact12FixtureCodecV1.encodeCanonical(bundle)
PrivacyExact12FixtureCodecV1.requireCanonicalArchive(receivedArchive, canonicalArchive)
```

The codec requires the exact schema, version, twelve-row order, uncompressed
`COMPACT_LEN` layout, and configured field/aggregate limits. It rejects
alternate Base64, truncation, trailing or unknown data, and reordered rows.
Use `requireCanonicalArchive` with an independently trusted fixture when exact
cross-row and cross-field identity matters.

The registry has exactly twelve IDs: `zk-ace-pq-authorization-v1`,
`anonymous-pgc-k-out-of-n-v1`, `verange-transparent-range-v1`,
`iroha-zk-ams-v1`, `vega-existing-credential-zk-v1`,
`iroha-zk-x509-stark-p256-v1`,
`iroha-jindo-polynomial-commitment-v1`,
`iroha-bootle-lantern-anoncred-v1`, `orchard-halo2-actions-v1`,
`monero-fcmp-plus-plus-v1`, `iroha-ivm-private-note-stark-v1`, and
`pq-masp-stark-v1`. Parsing is exact: aliases, retired IDs, case changes, and
whitespace normalization fail closed.

### Shared Java transaction fixtures

Kotlin/JVM and the mirrored Java Android SDK validate the same Rust-owned
transaction corpus. The authority is `../fixtures/norito_rpc`: Kotlin's
`AndroidFixtureSupport` resolves the descriptors and all 27 canonical
`.norito` payloads there, while the owner publication also writes the identical
descriptor-and-blob set into `../java/iroha_android/src/test/resources` for
Java's classpath-based tests. There is no Kotlin-local fixture copy and the
generated Java resource directory is never a regeneration input. Rotate both
consumers only through the two-root `norito-rpc-fixtures` owner workflow and
finish with `norito-rpc-verify`.

---

## Build Instructions

Rust/Kotlin parity tests require a freshly built `kotlin-fixture-gen`
executable supplied explicitly through `IROHA_KOTLIN_FIXTURE_GEN_BIN`. Relative
paths are resolved from the repository root. The test runner rejects an unset,
blank, missing, non-file, or non-executable value and never invokes Cargo:

```bash
export IROHA_KOTLIN_FIXTURE_GEN_BIN=/absolute/path/to/kotlin-fixture-gen
./gradlew :core-jvm:test --console=plain
```

### Prerequisites

| Tool | Version | Required For |
|------|---------|-------------|
| JDK | 21 | All modules |
| Android SDK | compileSdk 35 | `client-android` |
| Rust | exactly 1.93.1 | Native `.so` build |
| Android NDK | exactly 28.0.12674087-beta2 (r28-beta2) | Native `.so` build |
| `cargo-ndk` | exactly 4.1.2 | Native `.so` build |

### Step 1: Build core-jvm

The pure JVM module has no native dependency and builds immediately. Android
variant assembly is covered in the next step because AGP is causally wired to
the generated native bridge task.

```bash
# Build and run tests
./gradlew :core-jvm:build --quiet

# Run core-jvm unit tests
./gradlew :core-jvm:test --console=plain
```

### Step 2: Build native libraries (for `client-android`)

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task now lives on `client-android`, which owns the shared native bridge used for ML-DSA-65 signing and KAGEMUSHA V1 device lifecycle operations. It defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

**One-time setup:**

```bash
# Install Rust Android targets
rustup target add --toolchain 1.93.1 aarch64-linux-android x86_64-linux-android

# Install cargo-ndk
cargo install cargo-ndk --version 4.1.2 --locked

# Select the authenticated Android NDK and an external artifact root
export ANDROID_NDK_HOME=/absolute/path/to/android-ndk/28.0.12674087
export MOBILE_SDK_ANDROID_ARTIFACT_DIR=/absolute/non-symlink/path/to/android-artifacts
mkdir -p "$MOBILE_SDK_ANDROID_ARTIFACT_DIR"
```

**Build the .so files:**

```bash
# Build the capability-only native bridge.
./gradlew :client-android:buildNativeLibs
```

This Gradle task (and every `client-android` release assembly):
1. Reads `iroha.dir` from `local.properties`
2. Captures the exact Android-target dependency-closure source seal, then runs
   locked `cargo ndk` separately for `arm64-v8a` and `x86_64`, checking that
   seal after every ABI build. Each cargo-ndk destination is transient because
   Cargo can copy unrelated workspace `cdylib` outputs there; only the exact
   `libconnect_norito_bridge.so` name is promoted into the authoritative raw
   directory under the external
   `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/native/cargo-ndk/<mode>/`.
   Compiler state remains isolated in its sibling
   `native/cargo-target/<mode>/` through a mode-specific `CARGO_TARGET_DIR`.
   Every raw and stripped/provenance
   promotion re-authenticates the saved source commit and selected dependency-
   closure fingerprint immediately before and after the promotion; the source
   sampler itself rejects commit or fingerprint drift during authentication.
3. Copies the raw libraries to a distinct generated directory, then canonically
   strips only those copies with the selected Android NDK's
   `llvm-strip --strip-unneeded`
4. Writes the authoritative libraries under the external
   `client-android/generated/jniLibs/<mode>/` subtree
5. Generates the external
   `client-android/generated/nativeProvenance/<mode>/iroha/native-build-provenance-v1.json`
   with the ABI, feature state, source commit/scoped dirty bit, dependency-
   closure `source_fingerprint_sha256`, toolchain identity, and raw/stripped
   sizes and hashes

AGP 9.0.1 registers both generated directories through
`addGeneratedSourceDirectory`, so the release AAR preserves those exact bytes
and embeds the provenance at
`assets/iroha/native-build-provenance-v1.json`. `src/main/jniLibs` is excluded;
ignored or hand-copied source-tree `.so` files cannot enter an AAR. The mobile
artifact checker rejects an unstripped library, stale source fingerprint,
malformed provenance, extra native file (including another Rust `cdylib`), or
any size/hash difference among raw cargo-ndk output, generated stripped output,
provenance, and the AAR.

Every app and instrumentation variant includes the sealed native bridge and its
provenance. Ordinary JVM unit-test compilation uses its compiler task graph and
does not launch Cargo or the Android NDK. The `irohaDebugNativeBridge` selector
has been removed; native packaging has no opt-out.

For local device integration inside this checkout, create the ignored
`dist/norito-bridge-android-local` directory with mode `0700` and set
`MOBILE_SDK_ANDROID_ARTIFACT_DIR` to that exact absolute canonical path. Set
`MOBILE_SDK_PYTHON_BINARY` to a canonical Python 3.12 executable and add
`-PirohaAndroidLocalIntegration=true` to the same normal Gradle command. This
developer routing keeps the regular locked two-ABI native build, source seal,
stripping, export and byte checks. It requires an owned directory with no tracked
files and never falls back to source-tree JNI copies. Its embedded provenance
has `artifact_scope: local-integration`; publication and release packaging reject
that scope, including when the source is clean. It supplies local test evidence,
not release or physical-device qualification by itself. Ordinary release output
continues to require the external artifact root.

This also applies to an Android app consuming the SDK as a composite build.
An unchanged raw build is reusable only while its saved source seal still
matches the live checkout; packaging re-runs stripping, provenance generation
and the final seal check.

Every native build includes KAGEMUSHA and privacy support. The fixed
`privacy-production-enabled` Cargo feature records the sole build recipe; it is
an empty provenance marker and grants no provider, proving, hardware or release
qualification. The `privacyProductionEnabled` property has been removed.

For every ABI, Gradle resolves canonical `cargo`, `rustc`, and `rustdoc`
executables from exact Rust 1.93.1. It requires one job, incremental compilation
off and offline dependency resolution, then invokes stock Cargo through
cargo-ndk with the authenticated root manifest and lock:

```text
build --locked --offline --jobs 1 \
  --manifest-path <canonical-iroha-root>/Cargo.toml
```

The original root `Cargo.lock` is authenticated before and after execution.
Bootstrap, alternate locks, and compiler or profile configuration overrides
are rejected.

The first build takes ~5-10 minutes because it compiles all Rust dependencies.
The isolated target can reuse dependency artifacts, but compiler incremental
state remains disabled.

**Output:**

| ABI | File |
|-----|------|
| arm64-v8a | `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/generated/jniLibs/<mode>/arm64-v8a/libconnect_norito_bridge.so` |
| x86_64 | `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/generated/jniLibs/<mode>/x86_64/libconnect_norito_bridge.so` |

`<mode>` is always `production`. There is no disabled native profile.

> **Note:** `armeabi-v7a` (32-bit ARM) is not supported due to an upstream `rkyv` crate incompatibility with 32-bit targets.

### Step 3: Publish to local Maven

```bash
# Publish all three artifacts to ~/.m2/repository/
./gradlew publishToMavenLocal
```

This makes the artifacts available to any project on the same machine via `mavenLocal()`.

**Verify:**

```bash
ls ~/.m2/repository/org/hyperledger/iroha/sdk/core-jvm/0.1.0/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/client-android/0.1.0/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/kagemusha-wallet-android/0.1.0/
```

### Quick reference

```bash
# Full build from scratch (after local.properties is configured):
./gradlew :client-android:buildNativeLibs          # ~5-10 min first time
./gradlew publishToMavenLocal                       # ~30 sec

# Rebuild only core-jvm (no native deps):
./gradlew :core-jvm:publishToMavenLocal

# Rebuild after Rust source changes:
./gradlew :client-android:buildNativeLibs
./gradlew :client-android:publishToMavenLocal
```

## Push Device Registration

`core-jvm` includes thin Torii helpers for `/v1/notify/devices`. Android apps still obtain FCM tokens from their app layer; the SDK only encodes the signed Torii request:

```kotlin
val request = PushDeviceRequest(accountId, "FCM", fcmToken, listOf("activity"))
transport.registerPushDevice(request, canonicalAuth).join()
transport.unregisterPushDevice(request, canonicalAuth).join()
```

## Verifying Key Registry

Binding-only IVM verifier labels are retired and rejected. Production proof-backed IVM invocation remains closed until the complete native execution relation and finalized State authority are implemented and qualified.

`core-jvm` exposes Torii helpers for `/v1/zk/vk/register` and
`/v1/zk/vk/update`. They validate production verifier backends, required
registry fields, height ranges, and inline verifier-key commitments before
sending the request. Neither request accepts or transmits a private key. Torii
returns HTTP 200 with an unsigned transaction draft. SDK `Signer`
implementations apply Iroha's prehash themselves, so pass
`transactionPayloadBytes()` to `Signer.sign`; use `signingMessageBytes()` only
with an external primitive that signs an already-prehashed message. Attach the
signature to the transaction payload and use the standard transaction ingress.
The `ClientConfig` used by the transport must include an immutable
`LocalSigningContext`; read-only clients may omit it, but draft-producing
mutation routes fail before network I/O when it is absent. The draft parser
binds the exact canonical genesis-derived `NetworkId` and rejects
non-canonical Norito, another network or authority, extra/substituted
instructions, any mismatch in the complete verifying-key record, and signing
messages that do not match the payload prehash:

```kotlin
val networkId = NetworkId.parse("<canonical_network_id_hash_literal>")
val config = ClientConfig.builder()
    .setLocalSigningContext(LocalSigningContext(networkId))
    // Configure the Torii endpoint and other client policy here.
    .build()
val transport = HttpClientTransport(executor, config)
val vkBytes = byteArrayOf(1, 2, 3)

val registerDraft = transport.registerVerifyingKey(
    VerifyingKeyRegisterRequest(
        authority = "<authority_i105>",
        backend = "halo2/ipa",
        name = "vk_main",
        version = 1,
        circuitId = "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex = "a".repeat(64),
        gasScheduleId = "halo2_default",
        verifyingKeyBytes = vkBytes,
        status = "Active",
    )
).join()

val updateDraft = transport.updateVerifyingKey(
    VerifyingKeyUpdateRequest(
        authority = "<authority_i105>",
        backend = "halo2/ipa",
        name = "vk_main",
        version = 2,
        circuitId = "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex = "a".repeat(64),
        status = "Withdrawn",
    )
).join()

check(!registerDraft.submitted)
val registerPayload = registerDraft.transactionPayloadBytes()
val registerSigningMessage = registerDraft.signingMessageBytes()
val registerSignature = signer.sign(registerPayload)
```

## Signing Algorithm Selection

Android apps can now choose the transaction signing
algorithm explicitly:

```kotlin
import org.hyperledger.iroha.sdk.IrohaKeyManager
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.crypto.keystore.KeyGenParameters

val ed25519Manager = IrohaKeyManager.withSoftwareProvider()
val mlDsaManager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ML_DSA)
val gostManager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.GOST_2012_256_A)

val tunedManager = IrohaKeyManager.withDefaultProviders(
    KeyGenParameters.Builder()
        .setSigningAlgorithm(SigningAlgorithm.ML_DSA)
        .build()
)
```

`ED25519` remains the default. `SECP256K1`, `BLS_NORMAL`, `BLS_SMALL`,
`ML_DSA`, the five `GOST_2012_*` variants, and `SM2` use the shared native
bridge and are software-only in this SDK pass, so hardware/StrongBox
preferences fail fast instead of silently downgrading.

For Android Keystore Ed25519 aliases, required hardware preferences are checked
against the selected key's `KeyInfo`, not only provider capability or the
generation request. Unknown provenance fails closed; preferred policies may
downgrade and expose the measured route. Custom `KeyProvider` implementations
must override `outcomeFor(...)` to prove a hardware route, and a preference set
through `KeystoreKeyProvider.withPreference(...)` remains effective for plain
`generate(...)` calls. Deterministic Ed25519 export/import in
`core-jvm` also derives the public key from the private seed at both boundaries,
rejecting substituted public keys and inconsistent input pairs without changing
the v4 bundle layout.

## Resolving Account Aliases

`HttpClientTransport.resolveAccountAlias` posts to Torii's `/v1/aliases/resolve`
endpoint and returns the mapped account id. `AccountAliasResolution.index` is
optional and may be absent for backends that do not expose a deterministic
alias index. Unknown aliases surface as `Optional.empty()` without throwing:

```kotlin
HttpClientTransport.createDefault(config).use { client ->
    val resolved = client.resolveAccountAlias("some_alias@universal").join()
    if (resolved.isPresent) {
        val record = resolved.get()
        println("account_id=${record.accountId} source=${record.source}")
    } else {
        println("alias not found")
    }
}
```

HTTP and SSE use `OkHttpTransportExecutor` on JVM and Android. Client `close()`
cancels that client's calls, open streams and transaction-status polls. Default
clients own their backend; injected backends remain application-owned and can
serve other clients. Closing a client rejects subsequent network operations.

`ToriiEventStreamClient` also owns its stream readers. Closing a handle or client
disposes the response and completes caller cancellation without `onError` or
`onClosed` callbacks. Blocking SSE reads run on the client's own reader executor;
`Builder.setReaderExecutor(...)` explicitly borrows an application executor.
Listeners may close the client from `onOpen`; response ownership is established
before callbacks run.

WebSockets use `NettyWebSocketConnector`, injected through
`ToriiWebSocketClient.builder().setWebSocketConnector(connector)`.
`NettyWebSocketConnector.create()` owns its NIO event loop. The constructor
`NettyWebSocketConnector(eventLoopGroup, sslContext)` borrows application
resources; closing it cancels its sessions and pending handshakes without
shutting down the borrowed group. TLS always verifies the hostname.
The connection deadline covers DNS, TCP, TLS and the single HTTP upgrade.
Redirects and failed upgrades are never replayed.

`sendText` and `sendBinary` send complete messages; binary buffers are copied
before a send returns. A successful send future means queue acceptance.
The engine bounds individual frames, aggregate messages, queued bytes and queued
message count, and handles wire ping/pong internally. Callbacks must not block;
they may close their session or connector. Close a subscription to cancel both
its pending handshake and any scheduled reconnect.

Android attestation verification requires an explicit non-empty challenge.
Construct the verifier with `AttestationVerifier.builder(revocationPolicy,
evaluationTimeEpochMillis)` and independently trusted roots. A different policy
or evaluation time requires a new verifier context.

For application TLS, pool or dispatcher configuration, construct an
`OkHttpClient` without application/network interceptors and pass it to
`OkHttpTransportExecutor(client)`. The adapter borrows those resources while
owning its calls. `OkHttpTransportExecutor.create(...)` creates an owned adapter;
its optional scheduling executor remains borrowed, allowing Android applications
to tag their worker threads explicitly. Close the adapter separately when it was
injected into SDK clients. Java consumers use the same types with try-with-resources.

The adapter disables redirects and authentication follow-ups. Signed/mutating
requests dispatch once, including connection loss and `Retry-After: 0`; transport
failures never imply that a submitted transaction was rejected. Buffered bounds
apply after decompression. Close each streaming response/body when finished.

## Reading Kotodama Manifests

`HttpClientTransport.getContractManifest(artifactId, canonicalAuth)` reads
`/v1/contracts/artifacts/{dataspace_id}/{code_hash}` using canonical account authentication and verifies the configured network and exact full-width artifact identity before returning the complete Kotodama V1 manifest model.
The decoder preserves `seiyaku_name`, branded `kotoage`/`hajimari`/`kaizen`
kinds, exact flat-preorder argument and return schemas, access completeness,
triggers, state, error-code, `kotoba`, and provenance metadata. A `List` node
contains only `capacity` and its element subtree immediately follows it. The
decoder rejects unknown fields, legacy nested `element` metadata, incomplete or
trailing tapes, over-depth schemas, noncanonical Norito hash literals,
inconsistent convenience hashes, and drifted interface schemas before returning
the record.

## Musubi V1 registry reads

`MusubiToriiClientV1` is the exact-network authenticated client for the twelve typed
`/v1/musubi/queries/*` POST routes. Its builder requires `LocalSigningContext`, and every method
requires `ToriiCanonicalRequestAuth`. Each exact raw body/path is signed with the configured
`NetworkId` and dispatched with one-shot replay policy. The `sdk.musubi` models preserve structural
package IDs, immutable namespace bindings, canonical structured SemVer
requirements, exact unsigned integers, finalized cursors, archive commitments,
and one exact genesis-derived `NetworkId` without legacy aliases or compatibility decoding.
Unknown fields, unsupported ABI/edition versions, and noncanonical names or
requirements are rejected. Each manifest, verification-lock parent, and
resolver row must use a distinct parent-local alias for every dependency.

The canonical cross-SDK JSON contract is
[`fixtures/musubi/sdk_v1.json`](../fixtures/musubi/sdk_v1.json), owned by the Rust
`iroha_data_model::musubi` surface. Canonical or witness headers cannot be injected through
default transport headers; the client derives them only from the explicit signing values.

The `/v1/musubi/queries/search` route exposes bounded exact-token
description and keyword discovery with a search-specific finalized projection
cursor. It is intentionally independent of dependency resolution.

`findArchiveRetention` submits a sorted, bounded exact archive batch and binds
the response to the requested identities and optional finalized snapshot before
returning any prune classification.

`MusubiInstructionsV1` supplies typed field-to-Norito constructors for immutable
namespace registration; package-maintainer invitation, acceptance, revocation,
role replacement, and removal; archive registration, signed pin-outbox inventory
advance, location addition or renewal, and location retirement; release publication,
yank, and unyank;
permanent alias registration; exact release-digest assertion; package metadata
replacement; and Parliament-enacted package ownership recovery,
permanent-alias retargeting, artifact takedown, and registry-policy replacement.
Each builder exposes `barePayload()`, `concreteFrame()`, and
`toInstructionBox()`; transaction encoding preserves the dynamic pair inline,
while standalone boxes use Rust's exact tuple schema. All twenty cases and
their four wire layers are checked against
[`fixtures/musubi/instructions_v1.json`](../fixtures/musubi/instructions_v1.json).

## Motivation

`core-jvm` now ships typed builders for the first dedicated RWA instruction
slice alongside the existing NFT helpers: `RegisterRwaInstruction`,
`TransferRwaInstruction`, `MergeRwasInstruction`, `RedeemRwaInstruction`,
`FreezeRwaInstruction`, `UnfreezeRwaInstruction`, `HoldRwaInstruction`,
`ReleaseRwaInstruction`, `ForceTransferRwaInstruction`,
`SetRwaControlsInstruction`, and RWA-aware `SetKeyValueInstruction` /
`RemoveKeyValueInstruction` targets.

### Kotlin as the standard

Kotlin is the default language for Android development. Migrating from Java makes the SDK consistent with the Android ecosystem and eliminates the friction of Java/Kotlin interop at the call site.

### Java 8 bytecode safety

Android libraries must target Java 8 bytecode. Java 11+ API calls (`String.isBlank()`, `List.of()`, `Files.readString()`) crash at runtime on older Android devices. All modules enforce JDK 8 API compatibility at compile time via `-Xjdk-release=8` — using JDK 9+ APIs is a compilation error, not a silent runtime failure. Kotlin's standard library provides equivalent functions that are safe across all API levels.

### Reflection-free

The original Java SDK used reflection in multiple places (Android API discovery, BouncyCastle loading, keystore operations). Kotlin production sources are reflection-free across `core-jvm`, `client-android`, and `kagemusha-wallet-android`; `scripts/check_kotlin_no_reflection.sh` enforces that contract. BouncyCastle is linked directly, Android keystore APIs are guarded by platform-version checks, and WebSocket clients require callers to inject a connector with `setWebSocketConnector(...)` instead of discovering one at runtime.

### Modular architecture

The original SDK shipped as a single monolith. This rewrite splits it into three artifacts with clear boundaries:

- **`core-jvm`** — pure JVM, no Android framework dependency. Usable in Kotlin Multiplatform modules, JUnit tests without Robolectric, server-side tools, and admin panels. Contains all protocol logic: Norito codec, transaction building, client transport, connect protocol.

- **`client-android`** — Android keystore integration, hardware-backed key generation, device telemetry, and the shared JNI bridge used for ML-DSA-65 signing. Depends on `core-jvm` via `api()` — consumers get all core types transitively.

### Null safety

The Java SDK required defensive null checks at every Kotlin call site (`!!`, `?:`, `?.let {}`). Kotlin's type system makes nullability explicit — parameters that accept null are declared `T?`, everything else is guaranteed non-null by the compiler. This removes most `NullPointerException` risks from consumer apps. Some risk remains at Java interop boundaries (BouncyCastle, JCA) where platform types (`T!`) may hide nullability.

### Testability without Android

`core-jvm` runs on any JVM. Consumers can unit-test transaction building, address encoding, signing, and Norito serialization with plain JUnit — no Android instrumentation, no Robolectric, no emulator.

## Side Dependencies

| Dependency | Version | Used By | Risk |
|-----------|---------|---------|------|
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` crypto, connect, and deterministic key export | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that force a different BC version may hit linkage errors at runtime. The SDK links the pinned provider directly and fails clearly when the mandatory implementation is broken; it never probes BouncyCastle through reflection. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression requires the native library to be available. |
