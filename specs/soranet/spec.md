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
- Every post-handshake relay application stream is additionally protected by
  the mandatory SoraNet record protocol (`SNR1`). TLS protects the live QUIC
  transport; `SNR1` uses the hybrid handshake secret so recorded application
  traffic retains the negotiated post-quantum confidentiality guarantee.
  Direction- and QUIC-stream-specific ChaCha20-Poly1305 keys are derived with
  HKDF-SHA-256. Each record carries
  `magic("SNR1") || sequence(u64 BE) || plaintext_len(u32 BE) || ciphertext || tag`.
  The header is authenticated as associated data, sequences start at zero and
  must be contiguous, and plaintext is capped at 64 KiB before allocation.
  Invalid, replayed, out-of-order, truncated, or unauthenticated records close
  the affected stream without exposing plaintext. Node-to-node SoraNet sessions
  use their existing equivalent ChaCha20-Poly1305 application framing.
  The interoperable v1 derivation is
  `HKDF-SHA-256(salt="iroha.soranet.record.hkdf-sha256.v1",
  IKM=session_key).expand(info, 32)`, where `info` is the byte concatenation
  `"iroha.soranet.record.chacha20poly1305.key.v1" || direction ||
  initiator || stream_kind || stream_index:u64be`. Direction is `0` for
  client-to-relay and `1` for relay-to-client; initiator is `0` for the QUIC
  client and `1` for the QUIC server; stream kind is `0` for bidirectional and
  `1` for unidirectional; and stream index is the QUIC stream number after
  removing the two type bits. The ChaCha20-Poly1305 nonce is four zero bytes
  followed by the record sequence as `u64be`. Each stream context may be
  derived only once per session. Implementations retain at most 65,536 used
  contexts per first-release session and reject reuse or capacity exhaustion.
- Relays reject TLS 0-RTT and VPN helper clients do not offer it, so relay or
  helper authentication and hybrid key derivation always complete before
  application streams can carry data.
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

  The fixed-width `client_nonce` slot is the domain-separated commitment
  `BLAKE3("soranet.pow.ticket_binding.v1" || u64be(len(descriptor_commit)) ||
  descriptor_commit || u64be(len(relay_id)) || relay_id ||
  u64be(len(transcript_hash)) || transcript_hash)`. The relay recomputes and
  compares it in constant time before Argon2id, then reconstructs the challenge
  by hashing the descriptor commitment, relay identifier, required admission
  transcript binding, exact commitment, and expiry. The transcript binding is
  carried in the `ClientHello` resume-hash field and must be exactly 32 non-zero
  bytes. Unbound tickets and client hellos are invalid. The
  resulting digest must contain at least `difficulty` leading zero bits once the
  client-supplied solution is hashed with `Argon2id(memory_kib, time_cost,
  lanes)`. The first-release default is 6 bits and zero is invalid. Tickets
  must expire within `max_future_skew_secs` of the relay's
  clock and remain valid for at least `min_ticket_ttl_secs` seconds. A relay
  mints each Argon2 candidate against a freshly anchored expiry and discards a
  candidate if the clock moves backwards or the completed trial would leave
  less than the required remaining lifetime. A configured mint target must
  exceed the minimum remaining lifetime while remaining below the future-skew
  ceiling, preserving headroom for clock differences between peers. A relay
  persistently consumes the ticket fingerprint before accepting the handshake;
  a replay, concurrent duplicate, full replay store, or unavailable replay
  store fails closed. Missing or invalid tickets cause the relay to terminate
  the connection before invoking the hybrid Noise/ML-KEM engine.
- Relays may also issue signed admission tokens to trusted clients. Tokens are
  sent as a standalone frame prefixed with the `SNTK` magic *before* any puzzle
  ticket and carry the relay identifier, handshake transcript hash, validity
  window, and an ML-DSA-44 signature from the configured issuer. When a token is
  presented and verifies against the active policy (including revocation
  checks), the relay skips the puzzle requirement for that handshake. Tokens are
  single-use: relays store consumed `token_id_hex` entries in a bounded replay
  ledger configured by `pow.token.replay_store_capacity` and the mandatory
  durable `pow.token.replay_store_path`. Retention covers the maximum token TTL
  plus both clock-skew edges (`max_ttl + 2 * clock_skew`). Active entries are
  never evicted: an unreadable, malformed,
  unwritable, or exhausted ledger fails startup or rejects admission instead of
  admitting a replay. The first-release ledger ceiling is 65,536 entries. Its
  capacity-derived byte limit and explicit Norito allocation budget are checked
  before replay maps are allocated; the final path component must remain a
  stable direct regular file and cannot be a symbolic link or reparse point.
  Each relay identity must have one authoritative ledger;
  a process-lifetime exclusive sidecar lock rejects a second owner, and cloned
  active replicas with independent stores are invalid. The Prometheus counter
  `soranet_token_verify_total{issuer,relay,outcome}` records accepted/replay/
  mismatch/expiry outcomes for dashboards and alerting.
  The token body includes a reserved `flags` byte (must be `0` in v1) and
  requires `issued_at < expires_at` for a non-zero validity window.
- Transcript binding:
  - TLS exporter `tls-exporter("soranet handshake", 64)` hashed into Noise prologue.
  - The 32-byte admission binding in `ClientHello` is mandatory and must equal
    the binding used to mint a puzzle ticket or admission token.
  - Ticket/token validation and single-use consumption complete before the relay
    validates or encapsulates client ML-KEM material.
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
