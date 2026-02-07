---
id: transport
lang: am
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
---

:::note Canonical Source
:::

SoraNet is the anonymity overlay that backs SoraFS range fetches, Norito RPC streaming, and future Nexus data lanes. The transport program (roadmap items **SNNet-1**, **SNNet-1a**, and **SNNet-1b**) defined a deterministic handshake, post-quantum (PQ) capability negotiation, and salt rotation plan so every relay, client, and gateway observes the same security posture.

## Goals & network model

- Build three-hop circuits (entry → middle → exit) over QUIC v1 so abusive peers never reach Torii directly.
- Layer a Noise XX *hybrid* handshake (Curve25519 + Kyber768) on top of QUIC/TLS to bind session keys to the TLS transcript.
- Require capability TLVs that advertise PQ KEM/signature support, relay role, and protocol version; GREASE unknown types to keep future extensions deployable.
- Rotate blinded-content salts daily and pin guard relays for 30 days so directory churn cannot deanonymize clients.
- Keep cells fixed at 1024 B, inject padding/dummy cells, and export deterministic telemetry so downgrade attempts are caught quickly.

## Handshake pipeline (SNNet-1a)

1. **QUIC/TLS envelope** – clients dial relays over QUIC v1 and complete a TLS 1.3 handshake using Ed25519 certificates signed by the governance CA. The TLS exporter (`tls-exporter("soranet handshake", 64)`) seeds the Noise layer so the transcripts are inseparable.
2. **Noise XX hybrid** – protocol string `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` with prologue = TLS exporter. Message flow:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH output and both Kyber encapsulations are mixed into the final symmetric keys. Failure to negotiate PQ material aborts the handshake outright—no classical-only fallback is permitted.

3. **Puzzle tickets & tokens** – relays can demand an Argon2id proof-of-work ticket before `ClientHello`. Tickets are length-prefixed frames that carry the hashed Argon2 solution and expire within the policy bounds:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Admission tokens prefixed with `SNTK` bypass puzzles when an ML-DSA-44 signature from the issuer validates against the active policy and revocation list.

4. **Capability TLV exchange** – the final Noise payload transports the capability TLVs described below. Clients abort the connection if any mandatory capability (PQ KEM/signature, role, or version) is missing or mismatched with the directory entry.

5. **Transcript logging** – relays log the transcript hash, TLS fingerprint, and TLV contents to feed downgrade detectors and compliance pipelines.

## Capability TLVs (SNNet-1c)

Capabilities reuse a fixed `typ/length/value` TLV envelope:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Defined types today:

- `snnet.pqkem` – Kyber level (`kyber768` for the current rollout).
- `snnet.pqsig` – PQ signature suite (`ml-dsa-44`).
- `snnet.role` – relay role (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` – protocol version identifier.
- `snnet.grease` – random filler entries in the reserved range to ensure future TLVs are tolerated.

Clients maintain an allow-list of required TLVs and fail handshakes that omit or downgrade them. Relays publish the same set in their directory microdescriptor so validation is deterministic.

## Salt rotation & CID blinding (SNNet-1b)

- Governance publishes a `SaltRotationScheduleV1` record with `(epoch_id, salt, valid_after, valid_until)` values. Relays and gateways fetch the signed schedule from the directory publisher.
- Clients apply the new salt at `valid_after`, keep the previous salt for a 12 h grace period, and retain a 7-epoch history to tolerate delayed updates.
- Canonical blinded identifiers use:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways accept the blinded key via `Sora-Req-Blinded-CID` and echo it in `Sora-Content-CID`. Circuit/request blinding (`CircuitBlindingKey::derive`) ships in `iroha_crypto::soranet::blinding`.
- If a relay misses an epoch, it halts new circuits until it downloads the schedule and emits a `SaltRecoveryEventV1`, which on-call dashboards treat as a paging signal.

## Directory data & guard policy

- Microdescriptors carry relay identity (Ed25519 + ML-DSA-65), PQ keys, capability TLVs, region tags, guard eligibility, and the currently advertised salt epoch.
- Clients pin guard sets for 30 days and persist `guard_set` caches alongside the signed directory snapshot. CLI and SDK wrappers surface the cache fingerprint so rollout evidence can be attached to change reviews.

## Telemetry & rollout checklist

- Metrics to export before production:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Alert thresholds live alongside the salt rotation SOP SLO matrix (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) and must be mirrored in Alertmanager before the network is promoted.
- Alerts: >5 % failure rate over 5 minutes, salt lag >15 minutes, or capability mismatches observed in production.
- Rollout steps:
  1. Exercise relay/client interoperability tests on staging with the hybrid handshake and PQ stack enabled.
  2. Rehearse the salt rotation SOP (`docs/source/soranet_salt_plan.md`) and attach drill artefacts to the change record.
  3. Enable capability negotiation in the directory, then roll out to entry relays, middle relays, exits, and finally clients.
  4. Record guard cache fingerprints, salt schedules, and telemetry dashboards for each phase; attach the evidence bundle to `status.md`.

Following this checklist lets operator, client, and SDK teams adopt SoraNet transports in lockstep while meeting the determinism and audit requirements captured in the SNNet roadmap.
