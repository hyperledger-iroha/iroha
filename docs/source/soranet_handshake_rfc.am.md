---
lang: am
direction: ltr
source: docs/source/soranet_handshake_rfc.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 986de03795a92fe5c3930e14a7a3b2b36ae926fec08c7d6780b5ae8fd9ea0e5f
source_last_modified: "2026-01-05T09:28:12.096010+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet Handshake and Salt Rotation Specification (Draft RFC)

> **Status:** Working draft. This document captures the normative requirements
> agreed by the Networking & Core Protocol working groups on 2026-02-12. It will
> evolve into the formal SNNet-1 RFC once test vectors and harnesses land.

## 1. Introduction

SoraNet provides an anonymous transport layer for Iroha 3 clients and services.
This specification defines the handshake, capability negotiation, and salt
rotation mechanisms required for anonymous circuit establishment.

## 2. Symbols and Terminology

- `MUST`, `MUST NOT`, `SHOULD`, `MAY` follow RFC 2119 semantics.
- `Client` (C): endpoint initiating a SoraNet circuit.
- `Relay` (R): entry, middle, or exit relay participating in circuit creation.
- `Salt Council`: governance committee maintaining blinded CID salts.
- `SaltAnnouncementV1`: Norito payload declaring the active blinded salt epoch.

## 3. Transport Overview

1. **Transport:** QUIC (RFC 9000) carrying a Noise XX handshake as application
   data. Handshake messages are framed inside a dedicated QUIC stream with
   1024-byte padding.
2. **Cryptography:**
   - Post-quantum key exchange: ML-KEM-768 (`kem_id = 0x01`) mandatory; optional
     ML-KEM-1024 (`kem_id = 0x02`).
   - Classical key exchange: X25519 included for auditing, not used for keying.
   - Signatures: Dilithium3 (`sig_id = 0x01`) primary, Ed25519 (`sig_id = 0x02`)
     witness signature.
3. **Failure policy:** Classical-only clients (missing ML-KEM-768 share) MUST be
   rejected prior to key derivation and logged via the downgrade telemetry path.

## 4. Handshake Sequence

### 4.1 Descriptor Fetch

Clients MUST obtain relay microdescriptors signed by the directory authority.
Descriptors include `descriptor_commit`, capability TLVs, and the latest salt
epoch pointer.

### 4.2 Noise XX Message Flow

```
 Client                                          Relay
  |----Descriptor fetch (SoraNS / directory)----->|
  |                                              |
  |---ClientHello (Noise e, pqkem TLVs, GREASE)-->|
  |                                              |
  |<--ServerHello (Noise re, pqkem echo, sig)----|
  |                                              |
  |---ClientFinish (Noise s, transcript hash T)-->|
  |                                              |
  |<--RelayConfirm (ticket, salt epoch, retry)--|
```

| Step | Sender | Payload | Notes |
|------|--------|---------|-------|
| 1 | Client | `ClientHello` carrying Noise ephemeral key `e`, ML-KEM-768 share, capability TLVs (with required flags), GREASE entries | Padded to 1024 bytes |
| 2 | Relay  | `ServerHello` carrying Noise key `re`, ML-KEM-768 response, Dilithium3 signature over transcript-so-far, capability echo | Padded |
| 3 | Client | `ClientFinish` with ML-KEM confirmation, Dilithium3 signature, transcript hash `T` commitment | Must include `snnet.transcript_commit` TLV |
| 4 | Relay  | `RelayConfirm` delivering entry ticket, current salt epoch, retry token encrypted under derived traffic keys | |

Transcript hash `T` MUST be computed as described in `docs/source/soranet_handshake.md`
and mixed into the dual HKDF.

### 4.3 Circuit Padding

- Frames padded to multiples of 1024 bytes.
- Idle cover traffic emitted every 150 ms ±25 ms.
- Teardown: relay and client send three cover frames before FIN.

## 5. Capability Negotiation

Capability TLVs MUST conform to the registry:

| Type | Name | Requirement |
|------|------|-------------|
| 0x0101 | `snnet.pqkem` | MUST appear in both ClientHello and ServerHello |
| 0x0102 | `snnet.pqsig` | MUST appear with Dilithium3 + Ed25519 IDs |
| 0x0103 | `snnet.transcript_commit` | Relay MUST include commitment hash |
| 0x0201 | `snnet.role` | Relay MUST declare role bitfield |
| 0x0202 | `snnet.padding` | MUST be `0x0400` (1024-byte cells) |
| 0x7F00–0x7FFF | GREASE | Clients MUST send ≥2 entries |

Relays MUST abort if clients mark a capability `required` and the relay cannot
satisfy it.

If multiple TLVs of the same type appear, relays MUST respect the `required`
flag semantics:

- `required=1`: relay MUST support at least one provided value for that type.
- `required=0`: relay MAY ignore unsupported values but MUST echo supported
  ones.

All TLVs are serialized as `type(2 bytes) | length(2 bytes) | value`. Unknown
types in the GREASE range (`0x7F00–0x7FFF`) MUST be ignored for negotiation but
MUST remain part of the transcript hash.

## 6. Salt Rotation

### 6.1 Governance

- Salt Council operates a 3-of-5 Dilithium3 multisig. All announcements also
  carry an Ed25519 witness signature from the directory authority.
- Rotations occur every 24 h. Emergency rotations require incident references
  recorded in the `notes` field.

### 6.2 Norito Payload

```norito
struct SaltAnnouncementV1 {
    epoch_id: u32,
    valid_after: Timestamp,
    valid_until: Timestamp,
    blinded_cid_salt: [u8; 32],
    previous_epoch: Option<u32>,
    emergency_rotation: bool,
    notes: Option<String>,
}
```

Announcements MUST be published via SoraNS (`soranet.salt.<epoch_id>`) and
pushed over the control plane. Relays cache the latest two epochs.

Clients that miss more than one epoch MUST perform the recovery protocol: fetch
all intermediate announcements, validate the signature chain, and only then
attempt new circuits. Relays encountering a client using a stale salt MUST
respond with `SaltMismatch` and include the latest `epoch_id` in the error
payload.

## 7. Telemetry and Downgrade Reporting

Relays MUST emit:

- `DowngradeAlarmReportV1` for each rejected handshake (within 5 seconds).
- Hourly `SoraNetTelemetryV1` records containing `downgrade_attempts`,
  `pq_disabled_sessions`, `cover_ratio`, `lagging_clients`, `max_latency_ms`.

Alerts trigger when `lagging_clients > 5 %` or `pq_disabled_sessions > 0`. In
either case operators have one hour to remediate and record the incident ID in
the telemetry feed (Norito field `incident_reference`). Persistent violations
MUST escalate to the governance channel.

## 8. Compliance & Monitoring

- Consensus diff bundles (`consensus_diff.car`) require 4/5 directory signatures
  (3/5 in emergency with incident ticket).
- Padding overhead targets: mean cover traffic ≤18 %; CI harness fails if median
  >20 %.

## 9. Test Vectors and Fixtures

To guarantee cross-implementation parity, the following fixture bundle MUST be
maintained under `docs/assets/soranet/fixtures/v2/` and regenerated via
`cargo xtask soranet-fixtures` whenever the protocol evolves.

1. **Handshake transcripts**
   - `snnet-cap-001-success.norito.json`: Successful negotiation with
     ML-KEM-768 + Dilithium3 (baseline vector). Includes padded frames,
     transcript hash, and replay ticket.
   - `snnet-cap-002-downgrade.norito.json`: Relay strips `snnet.pqkem`; client aborts.
     Demonstrates downgrade telemetry.
   - `snnet-cap-003-digest-mismatch.norito.json`: Transcript-commit tampering case
     leading to rejection.
   - `snnet-cap-004-grease.norito.json`: Valid handshake preserving multiple
     GREASE TLVs.
   Each transcript file MUST carry a Dilithium3 signature over the JSON payload
   and an Ed25519 witness signature. Clients slab-verify these signatures before
   ingesting the vector.
2. **Salt announcements**
   - `salt/epoch-000042.norito.json`: Routine rotation example (epoch 41→42).
   - `salt/epoch-000099.norito.json`: Emergency rotation with incident ticket in
     `notes`.
   These fixtures include signatures from the Salt Council multisig and the
   directory witness.
3. **Downgrade telemetry**
   - `telemetry/snnet-cap-002-downgrade-alarm.norito.json`: A canonical
     `DowngradeAlarmReportV1` for the missing-KEM case.
   - Additional telemetry samples for `ClassicalOnlyFallback`, `TranscriptCommitMismatch`,
     and `CapabilityParseError`.
4. **Hourly telemetry reports**
   - `telemetry/hourly/soranet_telemetry_baseline.norito.json` with cover ratio,
     lagging clients, and incident reference fields populated.
5. **Harness parity**
   - `docs/assets/soranet/fixtures/v2/manifest.json` enumerating fixture IDs,
     version labels, and hashes of each file (Blake2b-256). The `verify` mode of
     the harness MUST recompute these hashes during CI.

SDKs MUST ingest these fixtures, replay the transcript hash, and compare results
against the declared values during their interoperability suites.

## 10. Implementation Status

- Draft spec recorded in `docs/source/soranet_handshake.md` (source of truth).
- RFC text pending completion by **2026-03-15**.
- Harness and fixtures due **2026-04-15**.


## 11. Telemetry Payload Definitions

### 11.1 DowngradeAlarmReportV1

```norito
struct DowngradeAlarmReportV1 {
    relay_id: String,
    relay_role: u8,
    client_id: Option<String>,
    capability_type: u16,
    capability_value: Option<Vec<u8>>,
    reason: DowngradeReason,
    transcript_hash: [u8; 32],
    observed_at: Timestamp,
    circuit_id: Option<u64>,
    counter: u32,
    signature: DetachedSignature,
}

enum DowngradeReason {
    ClassicalOnlyFallback = 0,
    MissingRequiredKEM = 1,
    MissingRequiredSignature = 2,
    TranscriptCommitMismatch = 3,
    CapabilityParseError = 4,
}
```

- `relay_id` MUST match the base58 identity key appearing in the consensus
  descriptor.
- `capability_value` MUST contain the offending TLV payload when available.
- `signature` uses Dilithium3; the Ed25519 witness signature appears in the hourly
  telemetry bundle.

### 11.2 SoraNetTelemetryV1

```norito
struct SoraNetTelemetryV1 {
    epoch: u32,
    downgrade_attempts: u32,
    pq_disabled_sessions: u32,
    cover_ratio: f32,
    lagging_clients: u32,
    max_latency_ms: u32,
    incident_reference: Option<String>,
}
```

Hourly reports MUST be signed (Dilithium3 + Ed25519 witness) and archived for at
least 30 days. Alerts trigger when `lagging_clients` exceeds 5 % of active
clients or `pq_disabled_sessions` is non-zero.
