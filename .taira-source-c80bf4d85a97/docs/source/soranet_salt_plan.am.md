---
lang: am
direction: ltr
source: docs/source/soranet_salt_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3a803dfbd39c9a1e29d5c7d25f386715891b6930eadc186a451d771e6805849
source_last_modified: "2025-12-29T18:16:36.210549+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Salt Rotation SOP
summary: Standard operating procedure for SNNet-1b salt rotation & recovery.
---

# SoraNet Salt Rotation SOP

This runbook is the canonical operating procedure for publishing the daily
SoraNet salt announcement defined by SNNet-1b. It supersedes earlier draft
notes and is binding for Salt Council members, directory operators, and relay
teams whenever a rotation or recovery drill is executed.

## Topics
- Rotation cadence, approvals, and publication windows.
- Salt epoch publication (daily) via governance channel.
- Missed epoch recovery steps for gateways/clients.
- Atomic index swaps and rollback handling.
- Client notification (Norito envelope + IPNS pointer).
- Telemetry, evidence capture, and audit retention.
- Tooling references for rehearsals and verification.

## Roles & Responsibilities
- **Salt Council (3-of-5 Dilithium3 multisig):** Draft and sign the daily `SaltAnnouncementV1`, coordinate emergency rotations, and maintain the key custody log.
- **Directory Authority:** Attach the Ed25519 witness signature, push announcements into IPNS/SoraNS, and archive signed payloads.
- **Torii Operators:** Publish the announcement through `/v1/soranet/salt/*`, monitor request success, and expose the latest digest to observability.
- **Relay Operators:** Consume the announcement within the grace window, rotate caches, evict stale circuits, and surface skew metrics.
- **Client & SDK Owners:** Ensure applications fetch updates, respect recovery windows, and provide user-facing error handling for `SaltMismatch`.
- **Observability & SRE:** Watch `soranet_salt_*` telemetry, trigger incident response when the publication SLA is missed, and verify alert coverage quarterly.

## Rotation Cadence & Prerequisites
- **Schedule:** Daily rotation at 00:00 UTC. The announcement becomes valid immediately and remains active for 24 h; a 12 h tolerance window is permitted for lagging peers.
- **Inputs:** New 32-byte `blinded_cid_salt`, confirmed `epoch_id = floor(unix_time / 86400)`, previous epoch (optional), and any incident notes.
- **Approvals:** Salt Council quorum approval recorded in the governance tracker. Emergency rotations additionally require incident commander sign-off.
- **Artifacts to prepare:** Draft payload (`salt.json`), signed envelope, governance CAR record, operator announcement template, and observability checklist.

## Salt Format & Distribution Mechanism

- **Canonical format:** Use `SaltAnnouncementV1` as defined in the handshake RFC. Fields:
  - `epoch_id` (u32): incrementing counter, one per day; computed as floor(unix_time / 86400).
  - `valid_after`, `valid_until`: nanosecond timestamps, 24 h window.
  - `blinded_cid_salt`: 32-byte random value.
  - `previous_epoch`: optional `epoch_id` pointer.
  - `emergency_rotation`: boolean flag.
  - `notes`: optional string (incident ID, maintenance info).
- **Signature envelope:**
  - Salt Council (3-of-5 Dilithium3) signs the Norito payload. Directory authority adds Ed25519 witness signature.
  - Envelope structure `SignedSaltAnnouncementV1` contains:
    ```norito
    struct SignedSaltAnnouncementV1 {
        announcement: SaltAnnouncementV1,
        council_signatures: Vec<DilithiumSignature>,
        witness_signature: Ed25519Signature,
        manifest_version: u16,      // indicates keyset manifest version
    }
    ```
- **Distribution pipeline:**
  1. Salt Council prepares the announcement, signs it, and submits to Torii.
  2. The announcement is stored in the governance DAG and exposed via IPNS (`/ipns/salt.sora.net/<epoch_id>`). The SoraNS resolver mirrors the pointer.
  3. Torii exposes `GET /v1/soranet/salt/latest` and `GET /v1/soranet/salt/history` responses signed with the same Norito envelope. CLI helpers emit structured Norito JSON for SDK parity.
  4. Relays cache the latest two epochs. Clients store the latest `epoch_id` and salt for circuit operations.
- **Header broadcast:** Control-plane channel (`/soranet/control/salt`) pushes announcement digests allowing clients to detect new epochs without polling.

## SLO & Alert Matrix

| Metric | Target / Warn | Page / Action | Notes |
|--------|---------------|---------------|-------|
| `soranet_salt_rotation_lag_seconds` | Target ≤ 600 s, warn at 600–900 s | Page Salt Council at ≥ 900 s lag for 3 consecutive samples | Mirrors the spec requirement that rotations stay within 15 minutes of midnight UTC. |
| `soranet_salt_skew_gauge` | Target 0, warn when > 1 for 5 min | Page relay on-call when ≥ 2 for 5 min | Indicates how many relays still advertise the previous salt epoch. |
| `soranet_salt_publish_latency_seconds` | Target ≤ 120 s, warn at 180 s | Page directory authority at ≥ 300 s | Measures signing → Torii publication latency; fetch IPNS logs when breached. |
| `soranet_salt_client_catchup_seconds` | Target ≤ 300 s | Page SDK leads at ≥ 600 s | Derived from client telemetry; ensures apps ingest announcements quickly. |
| `lagging_clients` | Target 0 | Page SDK leads when > 5 | Count of clients still serving the previous epoch after the grace window. |
| `salt_publication_missed` | Always 0 | Immediate incident when 1 | Boolean gauge toggled when the publication window is exceeded. |

Alert definitions in Alertmanager must match the thresholds above. Operators are
expected to attach the alert firing ID to the change record when overrides are
invoked (e.g., delayed rotation due to governance directives).

## Standard Rotation Procedure

### T-60 Minutes - Pre-Rotation Checklist
1. Generate a new 32-byte salt from the hardware RNG vault; record the output hash in the custody log.
2. Confirm Torii and relay clusters are healthy and telemetry export is green (`salt_publication_sla` alert cleared).
3. Validate the upcoming `epoch_id` (no gaps, strictly greater than the last published value).
4. Run a dry-run generation:
   ```bash
   cargo run -p soranet-handshake-harness -- salt \
     --epoch-id <epoch> \
     --valid-after "$(date -u -d '@<epoch_start>' +%FT%TZ)" \
     --valid-until "$(date -u -d '@<epoch_end>' +%FT%TZ)" \
     --salt-hex <64-hex-chars> \
     --previous-epoch <epoch-1>
   ```
   Review the JSON for correctness and share the draft with the witness signer.

### T-10 Minutes - Signing & Staging
1. Salt Council members sign the Norito payload, producing `SignedSaltAnnouncementV1`.
2. Directory authority applies the Ed25519 witness signature and verifies the signature set via:
   ```bash
   cargo run -p soranet-handshake-harness -- salt-verify \
     --vector fixtures/soranet_handshake/salt/epoch-<epoch>.norito.json
   ```
3. Upload the signed payload to the governance staging bucket and prepare the IPNS publish command.

### T+/-0 Minutes - Publish
1. Execute the governance submission to append the record to the DAG.
2. Publish to IPNS/SoraNS:
   ```bash
   ipfs name publish --key salt-gov /ipfs/<dag-cid>
   ```
3. Trigger the Torii ingestion hook (`POST /v1/soranet/salt/publish`) pointing at the new record.
4. Notify relay operators via the `#soranet-ops` channel with the epoch number, hash digest, and rollout window.

### T+5 Minutes - Propagation Validation
1. Fetch the announcement via Torii and IPNS; compare digests:
   ```bash
   curl -s https://torii.sora.net/v1/soranet/salt/latest | jq '.announcement.epoch_id'
   ipfs cat /ipns/salt.sora.net/<epoch_id> | jq '.announcement.epoch_id'
   ```
2. Verify relays adopted the epoch by checking `soranet_salt_skew_gauge` (target 0) and the relay audit log.
3. Confirm clients acknowledge the new epoch through the telemetry dashboard (`lagging_clients` must stay within tolerance).

### T+30 Minutes - Post-Rotation Review
1. Archive the signed payload, Torii response, and telemetry snapshot to the compliance store.
2. Update the rotation tracker with publication timestamp, signer IDs, and validation evidence.
3. Close the change record once observability confirms the skew alert remained green for 30 minutes.

## Failure Scenarios & Recovery

- **Missed epoch (client offline):**
  1. Client compares local `epoch_id` with `/latest`. If the delta is greater than one, it requests `GET /v1/soranet/salt/history?from=<last>&to=<current>`.
  2. Client verifies all signatures, ensures strict monotonicity, and repopulates the cache prior to re-opening circuits.
  3. Circuit attempts using stale salts must surface `SaltMismatch` including `latest_epoch_id` and guidance to refetch history.
- **Relay lag:** Relay instances that fall behind are expected to refuse new circuits until cache parity is restored. Operators restart the fetch job, run `salt-verify` against the cached file, and only rejoin the rotation once telemetry reports skew <= 1.
- **Emergency rotation:** Set `emergency_rotation=true`, include the incident reference in `notes`, and page relay/client owners. Relays immediately tear down circuits; clients treat the announcement as an eviction notice and reinitialise transports.
- **Rollback guardrails:** The Salt Council must never reuse an `epoch_id`. If a malformed payload is published, governance issues an `EmergencyRollback` referencing the bad epoch and publishes a new announcement with an incremented `epoch_id`. Clients mark the rolled-back epoch as forever invalid.
- **Recovery drills:** Use the harness to simulate outages:
  ```bash
  cargo run -p soranet-handshake-harness -- salt \
    --epoch-id <epoch> --valid-after <...> --valid-until <...> --salt-hex <...>
  ```
  CI enforces verification of `fixtures/soranet_handshake/salt/epoch-*.norito.json` and ensures the recovery loop tolerates at least seven missed days.

## Telemetry & Audit Trail
- **Metrics & alerts:** The [SLO matrix](#slo--alert-matrix) defines the thresholds that Alertmanager enforces. Exporters in `iroha_telemetry` populate the `soranet_salt_*` gauges, while the staging harness emits synthetic events for drills. Keep `soranet_salt_publication_timestamp` in sync with Torii responses so investigators can correlate dashboard spikes with raw payloads.
- **Evidence capture:** Archive Torii responses, IPNS raw payload, signature bundle, alert IDs, and Grafana snapshots in the governance evidence store (`s3://soranet-ops/salt/<epoch_id>/`).
- **Audit hooks:** Run `scripts/telemetry/check_soranet_salt_skew.sh` after each publication to capture the skew report, and attach the textual summary plus JSON output to the change record.
- **Quarterly verification:** Observability replays historical announcements through the harness to confirm signature validity, retention completeness, and that the SLO alerts still fire under simulated drift.

## Tooling & Automation
- `cargo run -p soranet-handshake-harness -- salt ...` - render draft announcements.
- `cargo run -p soranet-handshake-harness -- salt-verify --vector <path>` - validate signed fixtures.
- `cargo run -p soranet-handshake-harness -- simulate --only-salt` - exercise client recovery scenarios.
- `xtask soranet-fixtures --salt` - regenerate reference fixtures for CI.
- `scripts/telemetry/check_soranet_salt_skew.sh` - automated skew tester used in staging (runs hourly).

## Communication Workflow
1. **Pre-announce:** Post T-60 reminder in `#soranet-ops`, tagging relay and client leads.
2. **Publish notice:** Provide epoch number, Torii digest, and IPNS CID once signatures are live.
3. **Validation confirmation:** Reply with telemetry screenshots and audit log offsets when propagation completes.
4. **Incident handling:** If alerts trigger, open an incident in the governance tracker referencing the epoch; include mitigation steps and timelines.

## Evidence & Compliance Checklist
- Entry in the Salt Council custody register (who generated and who signed).
- Governance ticket with payload hash, signature list, and Torii publish ID.
- Observability report proving skew stayed <= 1 throughout the day.
- Confirmation that recovery fixtures were regenerated when schema changes occur.
- Tabletop drill artefacts archived under `artifacts/soranet_drill/` (see `2026-03-21-tabletop.json`, `telemetry-2026-03-21.json`, `frames-2026-03-21/`, and `2026-03-28-salt-rotation.json`) with redacted fixtures mirrored at `fixtures/soranet_handshake/salt/2026-03-28-drill/`.

## Review & Approval
- 2026-03-21 Crypto WG - Verified handshake transcript vectors (`docs/source/soranet_handshake.md`, `fixtures/soranet_handshake/salt/epoch-000042.norito.json`), exercised the `soranet-handshake-harness` salt and telemetry commands, and approved this SOP for publication. Tabletop drill executed the same day with results logged in `ops/drill-log.md`, followed by a 2026-03-28 SNNet-1b harness rerun capturing parity metrics for relay/client operators. Outcome: Approved.

## Next Steps
- 2026-03-28: Completed rerun of the salt rotation tabletop drill with expanded relay/client participation; results archived under `artifacts/soranet_drill/2026-03-28-salt-rotation.json` and `fixtures/soranet_handshake/salt/2026-03-28-drill/`.
- Ongoing: Continue quarterly verification using the harness replay checklist and update this SOP if publication tooling changes.
