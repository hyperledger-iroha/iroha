---
lang: az
direction: ltr
source: docs/source/soranet_capabilities_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2afee099062adf617df2d5e03c9db345511634f7705eb0a1b8558228906ebb1
source_last_modified: "2026-01-05T09:28:12.089020+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Capability Negotiation Plan (Draft)
summary: Outline for SNNet-1c capability TLVs and downgrade detection.
---

# SoraNet Capability Negotiation Plan (Draft)

## Deliverables
- Capability TLVs (`snnet.pqkem`, `snnet.pqsig`, GREASE fillers).
- Dual-KDF extraction rules.
- Downgrade alarms and abort logic.
- Interop fixtures for relay/client implementations.

## TLV Format & Registry Process

- **TLV framing.**
  - Each capability record is encoded as: `type(2 bytes) | flags(1 byte) | length(2 bytes) | value`.
  - `flags` bits:
    - `0x01` — `required`: sender cannot proceed unless peer echoes at least one supported value.
    - `0x02` — `preferred`: hint for ordering but not mandatory.
    - `0x80` — reserved for future extensions (must be zero for now).
  - Values are big-endian. `length` counts only the value payload.
  - TLVs appear inside the Noise handshake transcript (ClientHello, ServerHello, ClientFinish) in ascending
    `type` order; duplicates are allowed (multi-valued TLVs).

- **Type registry.**
  - Formal registry maintained in `docs/source/soranet/capability_registry.md`. Type ranges:
    - `0x0100–0x01FF` — cryptography (PQ KEM, PQ signatures, transcript commits).
    - `0x0200–0x02FF` — transport/padding policies.
    - `0x0300–0x03FF` — relay role metadata.
    - `0x7F00–0x7FFF` — GREASE / experimental.
  - Requests to allocate new TLVs go through SNNet-1c review; once approved, entries list:
    `type`, `name`, `flags`, `value format`, `mandatory presence`, `version introduced`.
  - Encoding helpers in the Rust/Go/TS SDKs read the registry file at build time (generated Rust module /
    TypeScript enum) to avoid divergence.

- **Existing assignments (v1).**
  | Type    | Name                   | Value format                                      | Required | Notes |
  |---------|------------------------|---------------------------------------------------|----------|-------|
  | 0x0101  | `snnet.pqkem`          | sequence of KEM descriptors (`kem_id`, `kem_name`) | yes      | Must include ML-KEM-768. |
  | 0x0102  | `snnet.pqsig`          | list of signature algorithms (`sig_id`, `sig_name`) | yes    | Dilithium3 + Ed25519 witness. |
  | 0x0103  | `snnet.transcript_commit` | 32-byte hash                                     | yes      | Binds microdescriptor to transcript. |
  | 0x0201  | `snnet.role`           | bitfield (`entry=0x01`, `middle=0x02`, `exit=0x04`) | yes (relay) | Clients set `required=0`. |
  | 0x0202  | `snnet.padding`| 2-byte profile id                                 | yes      | `0x0400` = 1024-byte cells. |
  | 0x0301  | `snnet.capability_epoch` | u32 epoch                                         | optional | Used for capability rotation. |

## Example Capability Sets

- **Hybrid (default)**
  ```
  ClientHello TLVs:
    snnet.pqkem   required=1  → [ (0x01, "mlkem768"), (0x02, "mlkem1024") ]
    snnet.pqsig   required=1  → [ (0x01, "dilithium3"), (0x02, "ed25519") ]
    snnet.role    required=0  → [ client ]
    snnet.padding required=1 → [ 0x0400 ]
    GREASE entries         required=0 → [ 0x7F19:{random bytes}, 0x7F42:{random bytes} ]
  Relay echoes the same set with `snnet.role` = entry/middle/exit bits.
  ```
- **Classical-only fallback (audit mode)**
  - Only accepted on dedicated audit relays. Clients set `required=0` so PQ downgrade can be detected.
  ```
  snnet.pqkem   required=0 → [ (0xFF01, "x25519") ]    // indicates classical-only
  snnet.pqsig   required=0 → [ (0x02, "ed25519") ]
  snnet.transcript_commit required=1 → hash
  ```
  Relays receiving this set MUST reject unless configured for audit mode, emitting a downgrade alarm.
- **PQ-only (future readiness)**
  - Clients may prefer ML-KEM-1024:
  ```
  snnet.pqkem required=1 → [ (0x02, "mlkem1024") ]
  snnet.pqsig required=1 → [ (0x01, "dilithium3") ]
  snnet.padding required=1 → [ 0x0400 ]
  ```
  Standard relays will respond with 768/1024; clients choose the intersection.

## GREASE Behaviour & Parsing

- Clients MUST include at least two GREASE TLVs per handshake, using types chosen randomly from
  `0x7F00–0x7FFF` with monotonically increasing order. Value length between 4 and 16 bytes, filled with random
  data. `required` flag = 0.
- Relays MUST preserve unknown TLVs in the transcript hash and echo them back unchanged if they understand the
  framing (`type`, `flags`, `length`). If a relay ignores GREASE TLVs, the transcript will still validate but the
  client records the omission for telemetry.
- Parsers:
  - Unknown non-GREASE types with `required=1` → abort handshake with downgrade error.
  - Unknown GREASE types → ignore payload, but MUST keep them when computing transcript commitment `T`.
  - Duplicated GREASE types allowed (client must not rely on reflection).
- Telemetry:
  - Emit `soranet_capability_override` metric when required types are missing or when GREASE TLVs are stripped.
  - Downgrade alarms include the offending TLVs for debugging.

These additions complete the capability negotiation plan by specifying the TLV structure, registry mechanics,
reference capability sets, and GREASE handling expectations.
