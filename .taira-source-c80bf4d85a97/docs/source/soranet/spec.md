---
title: SoraNet Transport Specification
summary: Final specification for SNNet-1 covering handshake, salt rotation, capability negotiation, blinded CID, and rollout.
---

# SoraNet Transport Specification

## Goals & Scope
- Define the transport layer for the SoraNet anonymity overlay: handshake, key exchange, transcript binding, salt rotation, and capability negotiation.
- Provide implementation guidance for relays, clients, gateways, and tooling to ensure deterministic, secure behaviour.

This document satisfies **SNNet-1** (handshake, salt rotation, capability TLVs) and its sub-items SNNet-1a (PQ transcript review) and SNNet-1b (salt rotation & recovery plan).

## Network Model
- SoraNet uses a three-hop circuit (entry, middle, exit) built on QUIC transport with additional Noise-based handshake layering.
- Relays operate in roles: `entry`, `middle`, `exit`. Gateways may act as exit for content fetch.
- Clients maintain guard sets (entry relays) pinned for 30 days to prevent guard enumeration.
  Directory entries may attach endpoint `tags`; relays advertising
  `"norito-stream"` can forward Norito RPC/streaming traffic over Torii and
  should be preferred when constructing privacy routes.
- Circuits carry fixed-size cells (1024 bytes) with padding/dummy cells to obfuscate traffic.

## Handshake Overview (SNNet-1a)
- Base transport: QUIC v1 with TLS 1.3 handshake. TLS provides DoS-resistant connection establishment.
- On top of TLS, a Noise XX hybrid handshake negotiates PQ and classical keys, binding capabilities.
- Steps:
  1. **QUIC/TLS**: client connects to relay, completes TLS handshake using Ed25519 certificates signed by governance CA. TLS session used for initial key material.
  2. **Noise XX Hybrid**:
     - Protocol name: `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256`.
     - Prologue includes `tls-exporter` secret ensuring binding to TLS session.
     - Message pattern:
       - `-> e, s`
       - `<- e, ee, se, s, pq_ciphertext` (relay sends Kyber public key encapsulation)
       - `-> ee, se, pq_ciphertext` (client responds with encapsulation)
     - Derives shared secret mixing classical (Curve25519 DH) + PQ (Kyber768) outputs.
  3. **Capability TLV**: final handshake message includes TLV set:
     - `snnet.pqkem` (Kyber level)
     - `snnet.pqsig` (Dilithium)
     - `snnet.role` (entry/middle/exit)
     - `snnet.version` (protocol version)
     - `snnet.grease` (random filler values for negotiation resilience)
- When the directory publishes a puzzle policy, clients must send a
  `PowTicketV1` frame *before* the `ClientHello` unless they present a valid
  admission token. The frame is prefixed with a 16-bit length and carries:

  ```norito
  struct PowTicketV1 {
      version: u8,
      difficulty: u8,
      expires_at: u64,
      client_nonce: [u8; 32],
      solution: [u8; 32],
  }
  ```

  The relay reconstructs the Argon2id challenge by hashing the descriptor
  commitment, relay identifier, optional transcript hash, and client nonce. The
  resulting digest must contain at least `difficulty` leading zero bits once the
  client-supplied solution is hashed with `Argon2id(memory_kib, time_cost,
  lanes)`. Tickets must expire within `max_future_skew_secs` of the relay's
  clock and remain valid for at least `min_ticket_ttl_secs` seconds. Missing or
  invalid tickets cause the relay to terminate the connection before parsing
  the Noise payload.
- Relays may also issue signed admission tokens to trusted clients. Tokens are
  sent as a standalone frame prefixed with the `SNTK` magic *before* any puzzle
  ticket and carry the relay identifier, handshake transcript hash, validity
  window, and an ML-DSA-44 signature from the configured issuer. When a token is
  presented and verifies against the active policy (including revocation
  checks), the relay skips the puzzle requirement for that handshake. Tokens are
  single-use: relays store consumed `token_id_hex` entries in a bounded replay
  store (`pow.token.replay_store_capacity`, `pow.token.replay_store_ttl_secs`,
  optional `pow.token.replay_store_path` for persistence across restarts) and
  evict the oldest non-expired entries when full. The Prometheus counter
  `soranet_token_verify_total{issuer,relay,outcome}` records accepted/replay/
  mismatch/expiry outcomes for dashboards and alerting.
  The token body includes a reserved `flags` byte (must be `0` in v1) and
  requires `issued_at < expires_at` for a non-zero validity window.
- Transcript binding:
  - TLS exporter `tls-exporter("soranet handshake", 64)` hashed into Noise prologue.
  - Transcript hash logged in handshake logs for downgrade detection.
- Failure handling: if PQ negotiation fails, connection aborted; no fallback to classical-only allowed (mandated for first release).

## Capability Negotiation (SNNet-1c)
- Capability TLV format:
  ```norito
  struct CapabilityTLV {
      typ: u16,
      length: u16,
      value: Vec<u8>,
  }
  ```
- Clients maintain allowlist of required capabilities (PQ KEM, PQ signature). Missing capability -> handshake fails.
- GREASE entries (random types in reserved range) ensure future expansion.
- Relays publish capability sets in directory microdescriptors; clients verify TLV matches directory info.

## Salt Rotation & CID Blinding (SNNet-1b)
- Daily salt rotation to prevent CID correlation.
- Salt rotation plan:
  - Governance publishes `SaltRotationScheduleV1` containing `epoch_id`, `salt`, `valid_after`, `valid_until`.
  - Relays fetch schedule via directory publisher (see SNNet-3).
  - CID blinding is layered:
    - Canonical cache key used for deterministic storage and audits: `cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)`.
    - Gateways accept this value via the `Sora-Req-Blinded-CID` header and echo the canonical identifier in `Sora-Content-CID`.
    - `CircuitBlindingKey::derive(salt, circuit_secret)` prepares the per-circuit/request derivations that will be activated alongside the full SoraNet handshake.
    - The implementation ships in `iroha_crypto::soranet::blinding`; see `cid_blinding_research.md` for evaluation notes and staged rollout details.
  - Clients rotate salt at `valid_after`; maintain previous salt for tolerance window (12h) to handle lag.
- Recovery:
  - If relay misses rotation, it requests latest schedule; if `valid_until` passed, relay halts circuits until updated.
  - Clients maintain salt history (last 7 epochs) to handle delayed updates.
  - `SaltRecoveryEventV1` logged when relay catches up; triggers monitoring alert.

## Directory Data
- Microdescriptor fields:
  - Relay identity (Ed25519), PQ keys, capabilities, guard flags.
  - Salt epoch, blinded CID support, region info.
- Consensus file (`consensus.car`) includes digest of salts, handshake capabilities, version.

## Monitoring & Telemetry
- Metrics:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Alerts:
  - Handshake failure spike >5% per 5 minutes.
  - Salt rotation lag > 15 minutes (relay lagging).
  - Capability mismatch observed.
- Logs: `handshake_event` capturing transcript hash, capability TLVs, negotiated version (no PII).

## Rollout Plan
- Implement handshake in relay/client; run interop tests on staging network.
- Validate PQ integration with Crypto WG; publish audit report (SNNet-1a deliverable).
- Set up salt rotation schedule and recovery playbook; dry-run with staging relays (SNNet-1b).
- Enable capability negotiation in directory and handshake; monitor for GREASE handling.
- Production rollout phased: entry relays first, then middle/exit; clients updated with new handshake.

## Implementation Checklist
- [x] Define hybrid handshake and transcript binding.
- [x] Document capability TLVs and negotiation rules.
- [x] Specify salt rotation schedule, recovery, and CID blinding.
- [x] Capture metrics, alerts, and rollout steps.

Further sub-specifications (SNNet-2, SNNet-3, etc.) will extend this foundation.
