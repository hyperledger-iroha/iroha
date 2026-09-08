# JVM consolidation capability inventory

This 2026-09-06 source review resolves the 33-entry queue in
`target/architecture-redesign/jvm-unmatched-source-names.json` (SHA-256
`32704aaa153bc3ba33c7730e52ab93449b8a1f9e4abc086ababb5ac9f94ed5da`).
It compares implementations and consumers, not filenames. Eleven entries have
an existing Kotlin owner; eight expose missing capabilities or invariants;
fourteen are duplicate implementations, construction wrappers or samples to
retire after their capability dependencies are satisfied. This closes that
queue only: it is not a complete SDK parity or runtime qualification claim.
The inventory itself is a source audit. Subsequent qualification below records implementation changes separately.

Path notation below is repository-relative:

- **J**: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/`
- **JA**: `java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/`
- **JJ**: `java/iroha_android/jvm/src/main/java/org/hyperledger/iroha/android/`
- **K**: `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/`
- **KA**: `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/`

Line numbers identify the reviewed source; subsequent edits can move them.
The sole public SDK package is `org.hyperledger.iroha.sdk`. Migration must not
retain Java-package aliases, duplicate public value wrappers, reflective
platform discovery or a JDK 11 implementation in the JDK 8 API surface.

## Existing canonical capability

| Java queue entry | Kotlin owner and implementation evidence |
| --- | --- |
| J `consensus/SumeragiJsonSupport.java` | Removed. K `consensus/SumeragiStatusModels.kt`, internal `SumeragiJsonPrimitives`, owns strict UTF-8, exact object fields, negative-zero rejection, bounded unsigned numbers and canonical hashes. Migrated Java consumers exercise the Kotlin status and diagnostics parsers. |
| J `consensus/NativeAmxV2Models.java` | Removed with the duplicate Sumeragi status, diagnostics and wire classes. K `consensus/NativeAmxV2.kt` owns the sole public value hierarchy and grouped parser; Java consumers call it directly. Remaining private-settlement responder checks invoke its canonical BLS peer validator directly. |
| J `alias/AliasNameSupport.java` | K `alias/AliasNames.kt:201` performs NFC/IDN segment and qualified-domain normalization plus exact u64 bounds; `AliasSetupModels.kt:961` and `AliasPlanVerifier.kt:423` own token/hash checks. No standalone public helper is needed. |
| J `privacy/ConfidentialNoteScalars.java` | K `privacy/ConfidentialNote.kt:533`: same Pasta modulus, 32-byte canonical/nonzero scalars, positive canonical u128 and defensive byte copies. |
| J `privacy/ConfidentialNoteCrypto.java` | K `privacy/ConfidentialNote.kt:149–390`: public-key derivation, X25519 agreement, HKDF and authenticated note encryption/decryption, including deterministic entropy entry points. Kotlin `ConfidentialNoteTest.kt:170` covers plaintext contract and tampering. |
| J `client/ZkRootsJson.java` | K `client/ZkRoots.kt:69`: response parser is absorbed by the response companion; preserves root strings and evaluated block height/hash checks. |
| J `client/AccountAliasUInt64.java` | K `client/AccountAliasReadModels.kt:202`: `requireAliasU64`/`aliasU64` retain BigInteger range and integer-token validation. Existing `AccountAliasReadModelsTest.kt` covers the read models. |
| J `client/SccpSubmitEncoding.java` | K `client/SccpSubmitRequests.kt:90–211`: canonical base64/Norito envelopes, bounded compact cursor and sparse replay non-membership witness validation. Keep these internal to SCCP submissions. |
| J `client/ZkMerklePathJson.java` | K `client/ZkMerklePath.kt`, response companion parser: required `next_zero_path`, integer directions, witness nodes and evaluated snapshot are retained. Kotlin `ConfidentialAssetToriiClientTest.kt` and `ZkAssetMerklePathTest.kt` are relevant consumers. |
| J `client/transport/BoundedResponseBodyReader.java` | K `client/transport/BoundedResponseBodyReader.kt`: canonical single Content-Length, CL/TE rejection, decoded-body limits, premature EOF, zero-progress/invalid stream-read checks and bodyless HTTP responses. Its existing Kotlin test covers these cases. Extract a shared internal body reader only if another retained transport needs it. |
| J `model/instructions/ZkInstructionUtils.java` | K `core/model/instructions/ZkAssetInstructions.kt:458–555`: exact text, portable verifier components, verifier-key IDs, fixed/nonzero bytes and fixed-32 flattening already have canonical owners. |

These rows establish capability ownership, not equality of every validation
branch. Preserve shared fixture assertions and Java-source consumer coverage
against these Kotlin classes when retiring the Java implementation.

The six Sumeragi/Native AMX Java suites now run from `kotlin/core-jvm` against
the canonical Kotlin API. All 51 original Java tests are preserved; the focused
111-test selection passes without failures, errors or skips under JDK 8 API
compilation. The five duplicate consensus production classes and six original
Java suites are removed, together with the old `IrohaClient`/`HttpClientTransport`
Sumeragi methods and their unused operator-config wiring. Private-settlement
attestation checks and shared onboarding response validation remain. This
records SDK ownership and focused coverage; release qualification remains
separate.

## Canonical custom-instruction bytes (2026-09-09)

Kotlin's custom JSON instruction adapter added a redundant field-length prefix,
producing a different multisig hash from Rust. The adapter now writes the one
canonical nested JSON field. A [shared Rust-produced fixture](../fixtures/multisig/README.md)
binds the complete 79-byte instruction vector, nested custom frame and its
independently checked hash. Rust asserts both bytes and hash. Three Kotlin tests
cover exact bytes/hash, truncation and extra-prefix rejection, and compact-length
boundaries; one Java consumer calls Kotlin's public codec API. The four tests
reproduce two failures before the correction and all pass afterward, with JDK 8
API enforcement retained and 678 qualification inputs unchanged during each run.

The wider 19-test selection passes nine and fails ten while requiring the missing
ABI-23 `connect_norito_bridge` address validator. Those native-dependent controls
remain unverified; no address-validation fallback or test bypass is introduced.
The duplicate Java codec and its conflicting older fixture are still pending
capability/consumer retirement. This focused correction does not establish full
JVM, JNI, Android or publication qualification. Frozen patch, original failures,
source seals and runtime reports are retained under ignored
`target/architecture-redesign/norito-identity-cutover/multisig-custom-json-correction-v1/`.

## Capabilities and invariants still needing migration

Signer, verified Nearby and immutable Nexus migrations have focused and full JVM
evidence. WebSocket ownership now has real wire and Java-consumer coverage.
Attestation command ownership and migrated Java assertions now pass in Kotlin tooling.
Complete remaining Java implementation, JNI and publication retirement.

The compiled JNI inventory contains 77 Kotlin native declarations across ten
owners and three modules. The Rust SDK namespace now has exactly 77 matching
source exports. Six memo JNI exports without a JVM declaration and three
undeclared app-specific coordinator exports are removed; all 132 C ABI function
bodies are unchanged. Coordinator behavior lives directly under its Kotlin SDK
exports. There are still 55 declared Android duplicates to retire with their
Java implementation consumers.

Three new Java consumer classes preserve privacy, SoraFS reference validation,
and signer behavior against Kotlin. All 41 cases compile to JDK 8; 26 managed
Java cases and one existing Kotlin selector test pass with native loading
explicitly unavailable. Fifteen native cases require the rebuilt bridge.
The compiled declaration/export guard rejects the stale host library's seven
missing CUDA symbols and 76 unowned exports. It checks ownership and names;
native signature compatibility, execution, packaging and provenance remain
unqualified. The Android migration adds 21 managed Java cases covering key-manager
provider evidence/dispatch, explicit context isolation, canonical envelopes and
ZK model fixtures. All 32 scenarios in the three selected Java source suites now
map to these consumers, prior core consumers, existing internal Kotlin checks,
or compiled declaration constraints. The native ML-DSA manager case compiles
and requires `:client-android:testDebugHostNative` with an explicit rebuilt bridge.
The latest managed Android run passes 78 client and 2 wallet tests, including the
strengthened software fallback assertion. It does not execute the native case.

Java metadata authoring is now supported directly through the canonical
`JsonValue` factories. The immutable nominal class preserves canonical JSON
value equality; the payload owns an unmodifiable metadata copy for every map
size and rejects Java null entries. Five Java consumers and 41 existing
JSON/payload/codec tests pass. A wider selection also reached an existing
required-native assertion and failed without the bridge; the full JVM suite
has not requalified after this API change.

The canonical CUDA source is migrated; rebuilt native and hardware qualification remain open. These are seven capability groups across eight
queue entries, not eight replacement classes.

| Java queue entry | Concrete gap and canonical destination |
| --- | --- |
| J `client/CanonicalRequestSignatureProvider.java` | Migrated to Kotlin `RequestSigner`, used by canonical request authorization and all signing consumers. Five Java consumer tests cover opaque callbacks and canonical bytes; the full 1,253-test JVM run passes. Java implementation retirement remains. |
| J `nexus/NexusModelUtils.java` | Invariants now belong to immutable Kotlin `NexusAppModels` values: validated construction, owned arrays and immutable maps/sets, payload-derived hashes and canonical Ed25519. Twenty-seven focused tests, including five Java consumers, pass in the full JVM run. No public utility replacement or data-class copying remains. |
| J `offline/IrohaPeerNearbySecureChannelV1.java` | Kotlin `IrohaPeerNearbyV1.Session` owns typed verified-IPM1 seal/open, profile/sequence checks, bounded copies and wiping. Seven Kotlin and four Java consumer tests pass; both duplicate Java Nearby facades were removed. Device/radio qualification remains. |
| J `gpu/CudaAccelerators.java` | Migrated to K `gpu/CudaAccelerators.kt`: one bounded batch API per operation, explicit disabled/injected/native contexts, owned arrays and canonical BN254 limbs. Eight Java consumer tests pass. Seven compiled JVM native declarations match Kotlin-only exports in `platform_jni/gpu.rs`; five hardware tests compile under JDK 8 and the nightly lane selects them. The duplicate Java implementation/tests are removed. Rebuilt native binding execution and CUDA numerical/device qualification remain unverified. |
| J `tools/AndroidKeystoreAttestationHarness.java` | Replaced by `kotlin/tools` application `iroha-attestation`, using the byte-identical pure verifier moved to `core-jvm`. All 25 tool tests pass (22 Java consumers, 3 bounded-reader cases), including the original fixture assertions, independently supplied roots/challenge/SPKI/snapshot/time, duplicate/conflicting argument rejection, ZIP/byte bounds and atomic output identity. The shell launcher invokes the installed Kotlin command; old Java command and tests are removed. Physical StrongBox qualification remains separate. |
| JA `client/okhttp/OkHttpTransportExecutor.java` | Migrated to Kotlin `OkHttpTransportExecutor` and per-client `HttpTransportScope`. Owned defaults, borrowed injection, cancellation, permanent close, bounded framing/decompression and one-shot dispatch pass 29 transport, 8 scope and 8 Java consumer tests. Twelve new SSE lifecycle tests also pass. The full JVM checkpoint is 1,295 tests; Android debug unit suites pass 57 client + 2 wallet tests. Java backend retirement remains. |
| JA `client/okhttp/OkHttpWebSocketConnector.java` | Kotlin `NettyWebSocketConnector` owns explicit NIO/TLS lifetimes and a single upgrade attempt. Twenty-nine WebSocket tests pass, including two Java consumers, real wire bounds, TLS hostname/trust rejection and concurrent close/send. The 72-test focused selection also covers security, SSE and HTTP scopes. |
| JA `client/okhttp/OkHttpTransportWebSocket.java` | Canonical Kotlin interfaces send/deliver complete messages; ignored fragment flags and unsupported public ping/pong controls are removed. The engine bounds frame/aggregate size and outgoing bytes/count, checks UTF8 before allocation and handles wire control frames internally. Subscription cancellation/reconnect tests pass; the full 1,295-test JVM suite passes, with Android requalification and Java backend retirement remaining. |

HTTP/SSE use the Kotlin-owned OkHttp adapter. WebSockets use an explicit Netty
handshake because OkHttp hides the hooks needed to prohibit upgrade replay. Both
engines live in Android-free `core-jvm`, retain JDK 8 API enforcement, and expose
explicit resource ownership. There is one implementation per protocol.

## Duplicate surface to retire

| Java queue entry | Disposition |
| --- | --- |
| J `gpu/CudaAcceleratorsKotlin.java` | Removed. Java and Kotlin consumers call the same Kotlin-owned batch API; no Optional/null facade, scalar alias or global backend setter remains. |
| J `crypto/keystore/AndroidKeystoreStubBackend.java` | Desktop unavailable-backend sentinel. KA `crypto/keystore/AndroidKeystoreBackend.kt:43` explicitly constructs `SystemAndroidKeystoreBackend`; platform/version handling stays in KA and software providers stay explicit. Do not restore desktop Android discovery. |
| JA `client/AndroidClientFactory.java` | Construction convenience for HTTP, RPC, SSE, WebSocket and SoraFS. Use Kotlin builders with explicit shared adapters once the transport gaps above close. Preserve capability tests, not the umbrella factory. |
| JA `offline/IrohaPeerNearbyAndroidV1.java` | Removed after verified-IPM1 ownership and Java consumer tests moved to the Kotlin session and Android adapter. |
| JA `client/transport/OkHttpTransportExecutor.java` | Delegates to the other Java OkHttp executor; no independent transport behavior. |
| JA `client/okhttp/OkHttpClientProvider.java` | Global shared-client selection/replacement wrapper. Replace with explicit adapter resource ownership; do not keep the singleton discovery surface. |
| JA `client/okhttp/OkHttpTransportExecutorFactory.java` | Construction wrapper; retain lifecycle tests when replacing it, not a second factory. |
| JA `client/okhttp/OkHttpWebSocketConnectorFactory.java` | Construction wrapper for the connector to migrate. |
| JJ `client/JavaHttpExecutor.java` | Alternative JDK `java.net.http` HTTP implementation. Canonical K OkHttp provides HTTP/SSE within JDK 8; move its useful retry/framing cases into the chosen transport tests and retire this backend. |
| JJ `client/JavaHttpExecutorFactory.java` | Alternative JDK-client construction wrapper; no independent protocol capability. |
| JJ `client/websocket/JdkWebSocketConnectorFactory.java` | Retire with alternate backend after the canonical WebSocket connector exists. |
| JJ `client/websocket/JdkWebSocketConnector.java` | Retire JDK-11-dependent connector; carry handshake/transport contract tests to the canonical implementation. |
| JJ `client/websocket/JavaTransportWebSocket.java` | Retire the alternate JDK socket wrapper. The canonical Kotlin API delivers complete messages; frame aggregation and ping/pong belong to the engine and have no duplicate public control surface. |
| `java/iroha_android/samples-android/src/main/java/org/hyperledger/iroha/android/samples/MainActivity.java` | AAR-linkage/address-rendering sample only (lines 10–31). Address functionality already exists in K. Replace the packaging smoke coverage with a Java consumer of the Kotlin AAR, not a second SDK sample implementation. |

## Concrete test migration and completion conditions

- **Request signing:** port the callback cases from J tests
  `client/CanonicalRequestSignerTests.java:386–452` (maximum signature,
  empty/all-zero/oversized rejection and canonical bytes supplied to callback)
  and callback consumers in `AtomicPrivateSettlementToriiClientV1Tests.java`.
  Kotlin currently uses direct PrivateKey fixtures; add a Java-source consumer
  with an opaque signer and no private-key access, mutation isolation and
  propagated signing failure. Existing Kotlin canonical-message goldens remain
  authoritative; the provider must not alter account/network/body/freshness
  binding or validation bounds.
- **Nearby:** change the existing session's `seal` to accept a verified
  `IrohaPeerWireMessageV1` and `open` to return it. Validate the message profile
  against the authenticated session; retain bounds before copying, use owned
  encode/decrypt buffers and wipe them in `finally`. Decode/authenticate fully
  before advancing inbound sequence. The Android radio transport remains a
  bounded encrypted-record carrier. Java facades are deleted as consumers move
  to the canonical Kotlin types. No dedicated Java Nearby test was located in
  the current test tree, despite the facade's golden-test comment. Add tests
  for valid verified payments, authenticated invalid IPM1, wrong profile,
  replay/reordering, malformed/oversized records, failed-decode sequence
  behavior, mutation and idempotent destruction. Existing Swift
  `IrohaSwift/Tests/IrohaSwiftTests/IrohaPeerNearbyV1Tests.swift:307–650` supplies
  relevant crypto/sequence cases, but its raw `IPM1-payment-fixture` strings
  must become valid verified messages for the typed boundary. Kotlin's existing
  Android permission test is not secure-channel or radio-lifecycle coverage.
- **WebSocket and HTTP lifecycle:** retain JA tests
  `client/okhttp/OkHttpWebSocketConnectorTests.java:30/94` (real mock-server
  messages and timeout), JJ test `client/websocket/JavaTransportWebSocketTests.java:14`
  (callbacks, binary/text, controls), and JA
  `client/okhttp/OkHttpTransportExecutorTests.java:62–315` (307/308,
  connection/status retries, gzip expansion, limits, timeout, scoped cancellation
  and shared-resource isolation). Factory-identity assertions can be removed;
  rewrite assertions around explicit ownership. Keep K
  `client/transport/OkHttpTransportExecutorTest.kt` for the
  existing framing/limit contract. A passing mock connector is insufficient:
  require real connector handshake/message/close tests and Android consumption.
- **CUDA:** the eight canonical Java consumer tests preserve input validation,
  null-result, status, ownership and injected-backend behavior. The host native
  binding test executes all seven JNI declarations with empty batches; execution
  against a rebuilt bridge is pending. The separate `cudaHardwareTest` compiles
  five tests with all ten IVM Poseidon CPU goldens and independent BN254 modular
  arithmetic, including batching and size-one equivalence. Nightly builds the
  CUDA bridge and runs this task; missing device/results fail. Hardware execution
  remains unqualified on the current macOS host.
- **Attestation command:** all original harness assertions are migrated into
  `kotlin/tools` Java consumer tests. The 25-test suite passes, including both
  shared mock vendor fixtures, directory/ZIP roots, independently supplied
  identity/revocation commitments, and bounded reader/output behavior. The pure
  verifier moved byte-for-byte into `core-jvm`; Android device provisioning
  remains in `client-android`. The launcher builds the Kotlin distribution and
  preserves argv and the configured external artifact directory. Requalified Android host suites pass 57 client + 2 wallet tests; physical
  device evidence remains required.
- **Nexus values:** retain `NexusAppClientTest.java` transaction/Connect fixtures
  against K `NexusAppClientTest.kt`; add explicit constructor-input and
  accessor-output mutation tests for arrays, maps and sets, blank identifiers,
  and Java construction/signing. No dedicated mutation regression was found
  for all these Java values. Replacing the public Kotlin data classes also
  requires migrating test `.copy(...)` calls to valid immutable construction.

Retirement is complete only after the canonical Kotlin JVM/Android artifacts,
Java-source consumers, selected native/Android integration gates and retained
shared fixtures pass without compiling or loading the Java implementation.
The 33-entry queue cannot detect behavior gaps between identically named
classes (the request-auth and Nexus findings demonstrate this), nor does it
cover the complete Java publication, resource or JNI inventory.

## Subsequent implementation and qualification

- Kotlin auth now accepts the canonical `RequestSigner` interface. Software uses
  `RequestSigner.ed25519`; no auth constructor or request builder accepts a raw
  private key. All request consumers and 55 test calls migrated.
  Header/body signatures validate callback bounds and copy owned message/signature
  buffers. Five Java consumer tests cover exact canonical bytes, opaque callbacks,
  invalid output, software verification and failure without a second attempt.
- A fresh local `connect_norito_bridge` plus `kotlin-fixture-gen` build passes.
  With those explicit artifacts, the full Kotlin `core-jvm` suite passes 1,253
  tests, zero failures/errors/skips. This includes native JNI execution and the
  migrated Norito Java consumers. The production reflection guard passes.
- Nexus request/session/payload/signature records are now regular immutable Kotlin
  classes with byte-content equality, owned bytes and immutable collection snapshots.
  Signable payload hashes are derived from those owned bytes; the caller-supplied
  hash and all data-class copy methods are removed. Only canonical `ed25519` is
  accepted, including at construction; the `"0"` algorithm alias is removed.
  Receipts deeply snapshot JSON and retain transaction-hash validation. All 27
  focused Nexus tests pass, including five new Java construction, signing,
  mutation, algorithm and nested-receipt tests. Production reflection checks pass.
- The existing Kotlin Nearby session now seals and opens verified
  `IrohaPeerWireMessageV1` values, binds the authenticated profile, checks bounds
  before encoding and advances the receive sequence only after authenticated
  decoding succeeds. Owned temporary plaintext is wiped on every exit, including
  nested IPM1 decoder failure. Both duplicate Java Nearby facades are deleted;
  applications use the Kotlin session and Android radio adapter directly.
  Seven Kotlin adversarial/fixture tests and four Java consumer tests cover real
  role-bound signatures, compressed/exact shared messages, malformed authenticated
  plaintext, tamper/replay/reordering, ownership and destruction. These are host
  transport/shape tests, not offline-money proof or physical-radio qualification.
- HTTP/SSE now has one Kotlin-owned OkHttp backend. URLConnection, runtime platform
  discovery and overlapping default-client factories are removed. Shared framing
  validation runs before decompression, and decoded buffered bodies retain exact
  per-request limits. Requests and stream headers own immutable snapshots.
  Signed/mutating requests cannot replay on redirects, authentication follow-ups,
  connection loss or retry hints; original response hints are retained.
  Every HTTP/RPC/SSE/service client owns a call scope and implements `close`:
  default backends are owned; injected backends and scheduling executors remain
  borrowed. Closing disposes active streams safely during concurrent reads,
  rejects new admission, and cancels transaction polls. Twenty-nine Kotlin
  transport cases, seven scope cases and eight Java runtime consumers pass in
  the 1,253-test full JVM run. A subsequent focused run passes the eighth scope
  case and twelve SSE lifecycle cases, including cancellation during response
  admission/read, callback closure, reader rejection and borrowed executors.
- The canonical Netty WebSocket engine and subscription lifecycle pass 29 tests,
  including two Java consumers, real TLS/hostname rejection, oversized frame
  headers, fragmented aggregate bounds, UTF8, control replies, queued-byte/count
  bounds, late handshakes, reentrant closure and concurrent event-loop shutdown.
  All 72 focused transport/security/SSE/scope tests pass. The subsequent full JVM
  suite passes **1,295 tests, zero failures/errors/skips**, with the explicit local
  JNI library and fixture generator. The retired OkHttp WebSocket implementation
  is absent; its 503 upgrade replay could not meet the no-replay contract.
- Android debug JVM tests pass 57 client and 2 wallet tests without native builds.
  Challenge-less verification overloads/default arguments are removed, and the
  verifier builder requires policy/time inputs. Four Java consumer tests execute
  shared StrongBox certificates, challenge rejection, roots and time isolation.
  These are host tests, not physical StrongBox or release-native qualification.
- Android/device, publication/provenance and CUDA hardware qualification remain
  open. The remaining capability gaps above and duplicate Java implementation have
  not yet been retired.

- **Exact12 fixture owner:** the Java fixture codec and its two outer model
  classes are retired. All seven original Java tests, including every assertion,
  now compile against the canonical Kotlin API in
  `PrivacyExact12FixtureJavaConsumerTest`. JVM CI runs that consumer alongside
  the six Kotlin fixture tests; all 13 pass with the unchanged Rust-derived
  archive. This is codec/fixture execution evidence, not JNI or native packaging
  qualification. The privacy source guard takes its exact six C exports from
  the manifest parity auditor's single approved inventory.
