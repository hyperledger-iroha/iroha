# Iroha peer transport V1

This document defines the transport-neutral **Retail Offline Peer V1** family:
IPM1 messages, IQR1/IRQR QR, authenticated IPN1 Nearby, and the F049 NFC
application. The envelope carries canonical
application bytes unchanged; QR, NFC, and Nearby are presentation or delivery
layers around the same message.

The cross-SDK golden vectors are in
`fixtures/offline/peer_transport_v1.json`. Swift, Kotlin/JVM, and Android Java
must reproduce the static, RFC 1950, and ordered animated-frame vectors exactly.
`fixtures/offline/kagemusha_peer_transport_v2.json` is the distinct profile-2
structural vector: its qualified 49-byte NRT0 request archive and resulting
IPM1, IQR1, NFC, and authenticated Nearby bytes must also match exactly across
all three SDKs. Its one-byte body is intentionally not semantically valid and
must never be passed to the typed Kagemusha adapter.

## Canonical payload

A canonical payload consists of:

- profile `1` for Offline Note or profile `2` for a bounded handoff of the
  mainline typed Kagemusha native archive;
- kind `1` receive request, `2` payment, or `3` acknowledgement;
- the profile's sole unsigned 16-bit schema version: `1` for profile `1`, or
  `0x0102` for profile `2`; and
- non-empty canonical bytes bounded to 32 KiB.

Profile adapters preserve canonical bytes exactly. Profile `1` application
bytes are opaque to IPM1 and are never normalized, trimmed, or converted.
`IrohaPeerCanonicalTextPayloadCodecV1` is an Offline Note-only exact UTF-8
boundary; it rejects profile `2` so text cannot bypass the typed ABI21 archive
boundary. A signed `pk2off2:` custody-lineage payload therefore remains
byte-identical through every rail.

Profile `2` supports only schema `0x0102`. Generic IPM construction and decode
validate native-independent canonical Norito framing before accepting it:
NRT0 v0.0, no compression, the compact-length flag exactly, CRC64, a non-empty
body, and the authoritative fully-qualified kind schema. Receive requests and
payments require exactly eight zero padding bytes after the 40-byte header;
acknowledgements require zero. The Kagemusha adapter then delegates deeper
semantic decoding to the existing typed decoder and never rebuilds Norito
bytes. Full ABI21
Kagemusha QR/NFC/native archives may be up to 32 MiB and continue to use their
existing typed rails; IPM1 profile `2` is an explicit 12,288-byte bounded
small-handoff, not a
replacement for those rails. `peer_nfc_v1.json` is an all-retail profile-1
fixture; NFC does not permit a mixed-profile phase policy.

The Retail V1 "no fallback" rule is scoped only to `IrohaPeer*V1`. The
independent Kagemusha ABI21 bulk family remains available and is never
negotiated, reinterpreted, or selected as fallback for Retail V1:

- Swift retains `KagemushaQRStreamCodec`, `KagemushaNFCProtocol`, and
  `KagemushaNearbyExchange`;
- Kotlin retains `KagemushaQrStreamCodec`, `KagemushaNfcProtocol`, and
  `KagemushaNearbyEnvelopeCodec`; Android Java retains `KagemushaQrStream` and
  the corresponding NFC/Nearby Kagemusha facades; and
- its `PKK2*`/`PKKQ1` text, F050 NFC AID, and Kagemusha Bonjour/Multipeer rails
  remain distinct from IPM1 profile `1` and the bounded profile `2` handoff.

Kagemusha Nearby's JSON/text envelope has its own smaller bound; the 32 MiB
ceiling applies to its QR, NFC, and native archive paths, not that Nearby
envelope.

## IPM1 message

All integers are unsigned big-endian.

| Offset | Size | Field |
| ---: | ---: | --- |
| 0 | 4 | ASCII `IPM1` |
| 4 | 1 | version `1` |
| 5 | 1 | encoding: none `0`, RFC 1950 zlib `1` |
| 6 | 2 | profile |
| 8 | 1 | payload kind |
| 9 | 1 | flags, zero |
| 10 | 2 | schema version |
| 12 | 4 | canonical byte length |
| 16 | 4 | encoded body length |
| 20 | 32 | canonical hash |
| 52 | 32 | wire hash |
| 84 | variable | encoded body |

The canonical hash is BLAKE2b-256 over:

```text
UTF8("IROHA-PEER-PAYLOAD-V1\0") ||
profile_u16be || kind_u8 || schema_u16be || canonical_bytes
```

The wire hash is BLAKE2b-256 over:

```text
UTF8("IROHA-PEER-MESSAGE-V1\0") || header[0..<52] || encoded_body
```

Compression is opt-in. The peer-optimized producer uses encoding `1` only when
the deterministic level-6 RFC 1950 body saves at least 32 bytes and strictly
reduces `ceil(length / 256)`; otherwise it emits encoding `0`. A decoder accepts
encoding `1` only as a complete stream with the canonical `78 9c` header, no
dictionary or trailing bytes, a valid Adler-32, and exactly the declared
decompressed length. This decision is shared by QR, NFC, and Nearby. Encoded
bodies are profile-bounded: 24,576 bytes for Offline Note and 12,288 bytes for
the bounded Kagemusha handoff.
Hashes and declared lengths are verified before a message is exposed.

## IQR1 text and IRQR frames

QR text is exactly:

```text
IQR1: + RFC_9285_BASE45(IRQR_bytes) + :
```

The final colon prevents a valid Base45 space from becoming scanner-owned
trailing whitespace. The strict decoder performs no trimming and requires a
decode/re-encode match. Scan sessions use that same strict decoder: leading or
trailing SP, TAB, CR, or LF is non-canonical and rejected. One text frame is at
most 700 UTF-8 bytes.

An IRQR frame uses unsigned big-endian integers:

| Offset | Size | Field |
| ---: | ---: | --- |
| 0 | 4 | ASCII `IRQR` |
| 4 | 1 | version `1` |
| 5 | 1 | frame kind: complete `0`, header `1`, data `2`, parity `3` |
| 6 | 2 | payload profile |
| 8 | 1 | payload kind |
| 9 | 1 | flags, zero |
| 10 | 16 | first 16 bytes of the IPM1 wire hash |
| 26 | 2 | frame index |
| 28 | 2 | data-shard total (or `1` for a complete frame) |
| 30 | 2 | payload length |
| 32 | variable | payload |
| 32 + length | 4 | CRC32C over offsets `0 ..< 32 + length` |

If the complete IPM1 message fits, it is carried by one complete frame.
Otherwise the header frame carries the exact 84-byte IPM1 header. Every data
frame carries exactly 256 bytes; the final shard is zero-padded and the decoder
trims the reconstructed body to the IPM1 header's declared encoded length.
One 256-byte XOR parity frame protects each fixed pair of data shards and may
recover exactly one missing shard in that pair. Frames are emitted as
`header,D0,D1,P0,D2,D3,P1,...`; the identical header is inserted again after
each twelve emitted non-header frames. Header, data, and parity frames all put
the data-shard count in `total`; parity `index` is the pair index.

Scan sessions accept duplicate identical frames and header-last ordering,
quarantine conflicting streams, bound both active-stream count and pre-header
buffering, expire idle and absolute-age state, and verify the reconstructed
IPM1 message before completion. This prevents a long-lived scanner from being
pinned to an old transfer or grown indefinitely by camera noise. Optional
expected profile, kind, and schema are bound immutably at session construction.
A wrong-schema complete frame or header is quarantined before completion;
trailing parity and repeated headers remain quarantined until expiry, while a
successful completion itself is not quarantined. If application-domain
validation rejects a structurally valid completed IPM1 message, call the scan
session's bounded `quarantine(streamID, ...)` API before resuming capture; its
16-byte IDs use the same capped table and absolute lifetime. Explicit scanner
times must be nonnegative and monotonic; Swift also rejects non-finite values,
and integer deadline arithmetic saturates rather than wrapping.
The standard values are also hard V1 maxima: at most three active streams,
twelve pre-header frames and 3,072 pre-header payload bytes per stream, a
30-second idle lifetime, and a 180-second absolute lifetime. Constructors may
select smaller values but cannot raise those ceilings.

## IPD1 discovery and authenticated IPN1 Nearby

Google Nearby Connections uses point-to-point service ID
`org.hyperledger.iroha.offline.transfer.v1`. IPD1 discovery carries only
profile, role, a 16-byte session ID, and a 32-byte request canonical hash. A
normal context requires nonzero session and request values. The sole discovery
sentinel is the exact pair of all-zero values used by a sender before it has
selected a receiver; half-zero contexts are invalid. Selection adopts the
receiver's nonzero context and the subsequent hello must match it. Radio
discovery is strictly the Base64URL-no-padding ASCII encoding of all 56 IPD1
bytes on both iOS and Android; raw IPD1 bytes, padding, whitespace, standard
Base64 punctuation, and non-canonical aliases are rejected.

Both users must explicitly approve Google's matching 4-to-12 ASCII
verification digits. IPN1 then authenticates fresh nonzero hello/session/request/nonce
values, P-256 ephemeral keys, roles, device certificate bytes, and the exact
service ID in one SHA-256 transcript. Certificate signatures authenticate that
transcript. ECDH keys are expanded with HKDF-SHA256 into independent
sender-to-receiver and receiver-to-sender AES-256-GCM keys. Sequence numbers
start at zero and are strict: duplicate or reordered encrypted records fail,
and second hello/authentication records are rejected rather than resetting
keys or sequences.

Radio adapters accept only BYTES payloads, cap pending sends, assign unique
payload IDs, apply connection/send deadlines, and drain every completion once
on stop or failure. Queue acceptance is not delivery; only the exact payload's
terminal `PayloadTransferUpdate.Status.SUCCESS` completes a send. There is no raw,
unauthenticated, file, stream, or MultipeerConnectivity fallback. Plaintext
records are capped at 32,704 bytes. The encrypted IPN1 record adds 54 bytes and
must remain within 32 KiB; mobile adapters use the conservative 64-byte
allowance by default.
The complete authentication record is capped at 32 KiB, including its 60-byte
fixed header, so signatures are limited to 32,708 bytes. Mobile operation and
send timeouts must be finite, positive, and at most 300 seconds. A full
receive/payment/acknowledgement transcript can queue four records; a fifth
record in the same connection fails closed.

Callback invalidation linearizes admission, not application execution: work
admitted before stop/failure may finish, while callbacks that have not passed
the epoch gate are suppressed. Application callbacks may synchronously call
stop. Android defers callback-executor submission until after releasing its
lifecycle monitor, including when a caller injects a direct executor.
Listener queues are bounded and may reject overload. Terminal send completions
are never dropped: they use a separately bounded serial fallback and finally
run inline if both configured and fallback lanes are saturated. That exceptional
path preserves exact-once completion and bounded memory, but cannot promise the
configured callback context or global FIFO order while an earlier lane is
permanently stalled.

## NFC V1 and durability

The sole AID is `F049524F48415045455201`. Proprietary class `0x80` uses
instructions GET_INFO `10`, READ_REQUEST `11`, BEGIN_PAYMENT `20`, WRITE `21`,
COMMIT `22`, READ_ACK `23`, CONFIRM_ACK `24`, and GET_STATUS `25`. INF1 and
NST1 responses are fixed at 98 and 174 bytes. APDU offsets are unsigned 32-bit
big-endian values; short and extended lengths are decoded strictly.

The receiver permits byte-identical overlapping WRITE retries but rejects
gaps and conflicting replays. BEGIN_PAYMENT returns success only after the
application atomically stores and returns the exact fixed-width 244-byte IPA1
record. The callback receives an ephemeral
`IrohaPeerNfcPaymentAdmissionContextV1` and must return a distinct
`IrohaPeerNfcDurablePaymentAdmissionV1`; projected fields or a reconstructed
header are not persistence proof. IPA1 contains its request identity, payment
descriptor, and exact 84-byte IPM1 header with redundant fields checked on
decode. A restored IPA1 resumes `PAYMENT_RECEIVING` at byte zero, so the sender
rewrites deterministically from GET_STATUS. IDA1 takes precedence after COMMIT,
and BEGIN_PAYMENT is rejected in ACK-ready/complete phases.

COMMIT returns success only after application
storage has atomically persisted the payment result and exact IDA1 durable
acknowledgement. On the sender, `loadOrCreateDurableCheckpoint` is one
transactional boundary: it either loads the exact request- and peer-bound ISC1,
or creates the payment, applies its debit, and stores that ISC1 atomically. It
must return only the durable value. The SDK validates the returned request,
profile policy, and peer continuity before BEGIN_PAYMENT; a store failure sends
no BEGIN_PAYMENT. `updateDurableCheckpoint` is the separate monotonic update
that persists the ACK-bearing ISC1 before CONFIRM_ACK. If it fails, no
CONFIRM_ACK is sent. After process or RF loss, the load-or-create boundary and
GET_STATUS resume the same payment; creating or debiting a replacement payment
is forbidden.
Swift and Kotlin expose the same complete `IrohaPeerNfcReaderExchangeV1`
exchange. The Java facade delegates to the Kotlin runner rather than
duplicating phase logic. Swift and Kotlin apply their action budgets to the entire
exchange, including SELECT, GET_INFO, request reads, phase status probes, value
commands, and durable transitions. Their 73,996-action default covers three
maximum-size messages even at the protocol's one-byte minimum chunk.

Swift CoreNFC bounds each admission or COMMIT durability callback to five
seconds, including callbacks that ignore task cancellation. Both boundaries
share one process-wide, queue-free lease. Timeout/cancel does not release the
lease until the actual callback returns; retaps fail immediately with a
distinct saturation failure instead of spawning more tasks, and a callback
that never returns requires process restart. Stop or timeout prevents a late
returned record from installing or publishing into that CardSession; any write
that did become durable is decoded on the next start.
Exact/restored BEGIN and COMMIT replays emit idempotent `paymentAdmitted` and
`acknowledgementReady` observations. Reader retries default to three contacts
over three seconds and are hard-capped at ten contacts and 30 seconds. The
connect slot is claimed before an attempt is consumed, so duplicate detection
callbacks cannot spend retry budget.

One immutable `IrohaPeerNfcProfilePolicyV1.profile` binds a session: request,
payment, and acknowledgement must all match it. Every phase intersects local
and peer limits, and a full IPM1 value is capped at 24,660 bytes (84-byte header
plus the maximum Offline Note body). Android reader mode derives its
local limits from `IsoDep.maxTransceiveLength` and extended-APDU support. A
261-byte short-APDU path has only 203 WRITE payload bytes after V1 metadata;
extended-capable iOS↔iOS and Android↔Android paths may negotiate up to 4,096.
Cross-platform peers automatically use the smaller safe value.
The 24,660-byte NFC message value is a hard constructor ceiling, as are the
32-KiB canonical, 24,576-byte Offline Note encoded, and 12,288-byte bounded
Kagemusha encoded wire limits. Custom policies can tighten but not expand them.

All changes in this contract are client/SDK-local. They require no backend or
Torii endpoint change.

## SDK ownership and application permissions

- Swift portable types live in `IrohaSwift`; link and import
  `IrohaSwiftMobileTransports` for Google Nearby and Core NFC. The checked-in
  lock pins Nearby revision `53568fe88281d4408e48e3ebec7d8560bed7077d`,
  BoringSSL `0.7.2`, and the resolved Abseil revision.
- Kotlin portable types live in `core-jvm`; `client-android` provides Google
  Nearby 19.3.0, IsoDep, synchronous/async HCE boundaries, the canonical AID
  resource, and the durable receiver bridge. The async HCE boundary owns one
  process-wide, queue-free durability worker and at most one five-second lease.
  RF loss/reset detaches the old response but lets that already-started
  BEGIN/COMMIT callback finish or reach its deadline; repeated taps cannot start
  more callbacks while the lease is active. Attempt and activation identities
  prevent a detached, timed-out, or late callback from installing or replying
  into a later tap. Admission and commit callbacks must therefore be atomic and
  idempotent: a late successful write remains durable and is loaded as IPA1/IDA1
  on the next start even though its old RF response is suppressed.
  The Nearby adapter keeps a live operation unchanged on repeated/conflicting
  starts and binds payload callbacks, send deadlines, and pending deliveries to
  one activation epoch. Only terminal `PayloadTransferUpdate.Status.SUCCESS` crosses
  the success barrier; stopped-epoch callbacks cannot affect a restarted rail.
- The Java Android compatibility SDK consumes those same Kotlin portable and
  Android adapters, avoiding a second cryptographic implementation.

The first application-facing entry points are:

| SDK | QR | Nearby | NFC |
| --- | --- | --- | --- |
| Swift | `IrohaPeerQRCodecV1` and `IrohaPeerQRScanSessionV1` | `IrohaPeerNearbySessionV1` plus `IrohaPeerNearbyConnectionsTransportV1` | `IrohaPeerNfcReaderServiceV1`, `IrohaPeerNfcCardSessionControllerV1`, or portable `IrohaPeerNfcReaderExchangeV1.run` |
| Kotlin | `IrohaPeerQRCodecV1.encode` and `IrohaPeerQRScanSessionV1.ingest` | `IrohaPeerNearbySessionV1` plus `IrohaPeerNearbyConnectionsTransportV1` | `IrohaPeerIsoDepTransceiverV1` plus `IrohaPeerNfcReaderExchangeV1.run`; HCE uses `IrohaPeerAsyncHostApduServiceV1` and one stable `IrohaPeerNfcReceiverApduBridgeV1` |
| Android Java | `IrohaPeerQRCodecV1.encode` and `IrohaPeerQRScanSessionV1.ingest` | `IrohaPeerNearbyAndroidV1`, `IrohaPeerNearbySecureChannelV1`, and the transitive Kotlin Connections adapter | `IrohaPeerAndroidNfcV1` plus `IrohaPeerNfcV1.runReaderExchange`; HCE subclasses the transitive Kotlin service |

Swift apps link `IrohaSwift` and, for either radio adapter, also
`IrohaSwiftMobileTransports`. Kotlin apps use `core-jvm` for the portable
contract and `client-android` (or `offline-wallet-android`, which re-exports it)
for Android rails. The Java AAR carries `core-jvm` and `client-android` as
transitive peer dependencies.

### Apple application configuration

Camera QR capture is application-owned and needs `NSCameraUsageDescription`.
Google Nearby needs `NSBluetoothAlwaysUsageDescription`,
`NSLocalNetworkUsageDescription`, and this exact `NSBonjourServices` value:

```xml
<key>NSBonjourServices</key>
<array>
    <string>_F2EBA4BCB49B._tcp</string>
</array>
```

Core NFC reader mode needs `NFCReaderUsageDescription`, the Tag Reading
capability (`com.apple.developer.nfc.readersession.formats = [TAG]`), and the
following exact Info.plist selector list:

```xml
<key>com.apple.developer.nfc.readersession.iso7816.select-identifiers</key>
<array>
    <string>F049524F48415045455201</string>
</array>
```

CardSession receiver mode additionally requires iOS 17.4 or newer, a supported
and eligible device, and an Apple-provisioned profile containing:

```xml
<key>com.apple.developer.nfc.hce</key>
<true/>
<key>com.apple.developer.nfc.hce.iso7816.select-identifier-prefixes</key>
<array>
    <string>F049524F48415045455201</string>
</array>
```

Linking the Swift product grants none of these permissions. An unavailable or
ineligible NFC receiver stays disabled; it is not a signal to select a legacy
rail.

### Android manifest and runtime permissions

The Kotlin and Java AAR manifests merge `NFC`, legacy
`ACCESS_WIFI_STATE` / `CHANGE_WIFI_STATE`, legacy `BLUETOOTH` /
`BLUETOOTH_ADMIN`, `ACCESS_COARSE_LOCATION` through API 28,
`ACCESS_FINE_LOCATION` on APIs 29–31, `BLUETOOTH_ADVERTISE` /
`BLUETOOTH_CONNECT` / `BLUETOOTH_SCAN` from API 31,
`NEARBY_WIFI_DEVICES` from API 32, and `ACCESS_LOCAL_NETWORK` from API 37.
The host requests every applicable dangerous permission before starting the
corresponding role. Camera QR additionally needs app-declared/runtime
`android.permission.CAMERA`; NFC and the legacy state permissions are
manifest-only.

The SDK ships `@xml/iroha_peer_nfc_v1_aids`, but the host must declare its
concrete `IrohaPeerAsyncHostApduServiceV1` subclass:

```xml
<service
    android:name=".OfflinePeerHostApduService"
    android:exported="true"
    android:permission="android.permission.BIND_NFC_SERVICE">
    <intent-filter>
        <action android:name="android.nfc.cardemulation.action.HOST_APDU_SERVICE" />
    </intent-filter>
    <meta-data
        android:name="android.nfc.cardemulation.host_apdu_service"
        android:resource="@xml/iroha_peer_nfc_v1_aids" />
</service>
```

`BIND_NFC_SERVICE` protects the service declaration; applications do not ask
the user to grant it. The SDK AID resource requires device unlock and contains
only `F049524F48415045455201`.

## Fixtures and repeatable tests

The checked-in first-release fixture digests are:

| Fixture | SHA-256 |
| --- | --- |
| `peer_transport_v1.json` | `431be8c5b0bfb0be977821443d0a25659e8fa2e9b5924817c4488e8984ab1a70` |
| `peer_nearby_v1.json` | `cdcfb6073597087ab632dea4474bd7be8aeb7ffe3a3b75aa4bc03e572c2059c8` |
| `peer_nfc_v1.json` | `c9ee7fe20732b993f61b4f02278ed8267b0885a0a54f2536fa7ecb8c917fce16` |

Verify and exercise every SDK from the repository root:

```bash
shasum -a 256 fixtures/offline/peer_{transport,nearby,nfc}_v1.json

(cd IrohaSwift && swift test --filter IrohaPeer)
(cd IrohaSwift && swift test --filter KagemushaPeerTransportTests)

(cd kotlin && ./gradlew :core-jvm:test \
  --tests 'org.hyperledger.iroha.sdk.offline.IrohaPeer*' --console=plain)
(cd kotlin && ./gradlew :client-android:testDebugUnitTest \
  --tests 'org.hyperledger.iroha.sdk.offline.IrohaPeer*' --console=plain)

(cd java/iroha_android && \
  ./gradlew :core:check :android:testDebugUnitTest)
```

These transports are device-local. They add no backend route and do not change
Torii or native bridge ABI behavior.
