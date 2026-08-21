# Offline Cash V1 secure-device bridge

This note fixes the optional mobile ABI between the Offline Cash V1 Core adapter
and a platform service that owns rollback-resistant wallet state. It does not
make ordinary Android KeyMint or iOS App Attest sufficient for offline cash.
Those APIs already authenticate one-use or online assertions, but they do not
provide the complete atomic journal, counter, trusted-time, and outbox contract.
When the optional backend is absent or reports anything other than this exact
profile, the SDK remains online-only. There is no software fallback.

All integers are unsigned little-endian. Digests are SHA-256. Reserved bytes
must be zero. Every frame version is exactly `1`; values `4` and `5` are not
compatibility selectors and are rejected. The command payload is the bounded
canonical Offline Cash V1 archive owned by Core. A native implementation must
decode the exact operation-specific V1 type and reject a relabelled Kagemusha
V4/V5 archive.

## Capability frame

The capability frame is exactly 96 bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJCP1` |
| 8 | 2 | version `1` |
| 10 | 1 | platform: Android `1`, iOS `2` |
| 11 | 1 | zero flags |
| 12 | 4 | exact required-feature mask `0x000001ff` |
| 16 | 4 | maximum command payload `65,536` |
| 20 | 4 | maximum response payload `65,536` |
| 24 | 32 | non-zero hardware-policy id |
| 56 | 32 | non-zero backend-attestation digest, distinct from the policy id |
| 88 | 8 | zero trailer |

The nine required bits, in order, are:

1. one active intent slot;
2. exact-next monetary counter;
3. authenticated durable journal;
4. authenticated durable payment outbox;
5. trusted time;
6. atomic receive reservation and signing;
7. atomic commit and durable terminal receipt;
8. terminal recovery; and
9. no software fallback.

Missing or unknown bits fail closed. The digest fields identify an attested
backend; they are not self-authenticating release evidence. Registration and
Core's sealed backend adapter must verify the actual platform evidence and
terminal semantics before any result becomes authority.

## Command frame

The command frame has an 80-byte header followed by 1 to 65,536 payload bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJCM1` |
| 8 | 2 | version `1` |
| 10 | 1 | operation code |
| 11 | 1 | zero flags |
| 12 | 32 | non-zero idempotency/request id |
| 44 | 4 | payload length |
| 48 | 32 | SHA-256 of the payload |
| 80 | variable | canonical operation payload |

Operation codes exactly mirror the two sealed Core backends:

| Code | Operation |
| ---: | --- |
| 1 | reserve receive intent and sign |
| 2 | recover receive intent and signature |
| 3 | bind signed receive-request digest |
| 4 | publish canonical send payment |
| 5 | recover active intent |
| 6 | cancel expired receive intent using trusted time |
| 7 | consume intent and commit exact-next state |
| 8 | recover terminal outcome by exact intent |
| 9 | recover receive terminal by request/payment bindings |
| 10 | sign receive acknowledgement from a committed receipt |
| 11 | stage authenticated canonical payment |
| 12 | recover only the staged payment digest |
| 13 | publish the staged payment after journal authorization |
| 14 | recover an already-published payment |

## Response frame

The response has a 116-byte header, at most 65,536 payload bytes, and at most
8,192 authenticator bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJRS1` |
| 8 | 2 | version `1` |
| 10 | 1 | exact echoed operation |
| 11 | 1 | status |
| 12 | 32 | exact echoed request id |
| 44 | 4 | payload length |
| 48 | 4 | authenticator length |
| 52 | 32 | SHA-256 of the payload |
| 84 | 32 | SHA-256 of the authenticator |
| 116 | variable | payload, then authenticator |

Statuses are `0` success, `1` unavailable, `2` stale/concurrent, `3` intent
mismatch, `4` trusted-time rejection, `5` policy rejection, `6` missing,
`7` conflict, `8` corrupt, and `9` malformed request. Success requires a
non-empty payload and non-zero authenticator. Failure carries neither. The
authenticator remains platform-policy-specific; the trusted native backend must
verify it before returning a success frame, while Core independently validates
the exact terminal or outbox binding. Kotlin and Swift wipe their temporary
framed command and response buffers after each call; callers still own and must
dispose of the original canonical command according to its secret policy.

## Platform entry points

Swift discovers two optional C symbols in the already authenticated
`NoritoBridge` image:

```c
int32_t connect_norito_offline_cash_device_capabilities_v1(
    uint8_t *output,
    size_t output_capacity
);

int32_t connect_norito_offline_cash_device_execute_v1(
    const uint8_t *command,
    size_t command_length,
    uint8_t *output,
    size_t output_capacity,
    size_t *output_length
);
```

Android declares equivalent optional JNI methods on the Kotlin bridge. A
reviewed Android build may bind them with `RegisterNatives` or the corresponding
generated JNI names. A missing method, linkage error, malformed frame, platform
mismatch, partial feature mask, or native non-zero status produces online-only
discovery or a failed call; none triggers KeyMint/TEE/software downgrade.

The Java Android SDK delegates to the Kotlin implementation so there is one
codec and one production decision. The current stock-platform build exposes no
qualifying backend. Closing that physical gate requires an audited device/OEM
service and device evidence for journal rollback resistance, atomic outbox
publication, trusted-time expiry, exact-next concurrency, crash recovery, and
attestation-policy binding.
