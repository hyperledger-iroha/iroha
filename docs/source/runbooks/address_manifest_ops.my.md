---
lang: my
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1831136ce6b5968bd357e1ae03c5ee978a188aff7c4b9b0872cdc1e9f1bab636
source_last_modified: "2026-01-28T17:11:30.744436+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Address Manifest Operations Runbook (ADDR-7c)

This runbook operationalises roadmap item **ADDR-7c** by detailing how to
verify, publish, and retire entries in the Sora Nexus account/alias manifest.
It supplements the technical contract in
[`docs/account_structure.md`](../../account_structure.md) §4 and the telemetry
expectations recorded in `dashboards/grafana/address_ingest.json`.

## 1. Scope & Inputs

| Input | Source | Notes |
|-------|--------|-------|
| Signed manifest bundle (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | SoraFS pin (`sorafs://address-manifests/<CID>/`) and HTTPS mirror | Bundles are emitted by release automation; retain the directory structure when mirroring. |
| Previous manifest digest + sequence | Prior bundle (same path pattern) | Required to prove monotonicity/immutability. |
| Telemetry access | Grafana `address_ingest` dashboard + Alertmanager | Needed to monitor Local‑8 retirement + invalid address spikes. |
| Tooling | `cosign`, `shasum`, `b3sum` (or `python3 -m blake3`), `jq`, `iroha` CLI, `scripts/account_fixture_helper.py` | Install before running the checklist. |

## 2. Artifact Layout

Every bundle follows the layout below; do not rename files when copying between
environments.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

`manifest.json` header fields:

| Field | Description |
|-------|-------------|
| `version` | Schema version (currently `1`). |
| `sequence` | Monotonic revision number; must increment by exactly one. |
| `generated_ms` | UTC timestamp of publication (milliseconds since epoch). |
| `ttl_hours` | Maximum cache lifetime Torii/SDKs may honour (defaults 24). |
| `previous_digest` | BLAKE3 of the prior manifest body (hex). |
| `entries` | Ordered array of records (`global_domain`, `local_alias`, or `tombstone`). |

## 3. Verification Procedure

1. **Download bundle.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Checksum guardrail.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   All files must report `OK`; treat mismatches as tampering.

3. **Sigstore verification.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\\.sora\\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Immutability proof.** Compare `sequence` and `previous_digest` against the
   archived manifest:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   The printed digest must match `previous_digest`. Monotonic sequence gaps are
   not allowed; reissue the manifest if violated.

5. **TTL compliance.** Ensure `generated_ms + ttl_hours` covers anticipated
   deployment windows; otherwise governance must republish before caches expire.

6. **Entry sanity.**
   - `global_domain` entries MUST include `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - `local_alias` entries MUST embed the 12-byte digest produced by Norm v1
     (use `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
     to confirm; the JSON summary echoes the provided domain via `input_domain` and
     `legacy  suffix` replays the converted encoding as `<ih58>@<domain>` for manifests).
   - `tombstone` entries MUST reference the exact selector being retired,
     include a `reason_code`, `ticket`, and `replaces_sequence` field.

7. **Fixture parity.** Regenerate canonical vectors and ensure the Local digest
   table did not change unexpectedly:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Automation guardrail.** Run the manifest verifier to re-check the bundle
   end-to-end (header schema, entry shapes, checksums, and previous-digest wiring):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   The `--previous` flag points at the immediately prior bundle so the tool can
   confirm `sequence` monotonicity and recompute the `previous_digest` BLAKE3
   proof. The command fails fast when a checksum drifts or a `tombstone`
   selector omits the required fields, so include the output in your change
   ticket before requesting signatures.

## 4. Alias & Tombstone Change Flow

1. **Propose change.** File a governance ticket that states the reason code
   (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED`, etc.) and affected selectors.
2. **Derive canonical payloads.** For each alias being updated, run:

   ```bash
   iroha tools address convert sora... --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .ih58' /tmp/alias.json
   ```

3. **Draft manifest entry.** Append a JSON record similar to:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   When replacing a Local alias with a Global one, include both a `tombstone`
   record and the subsequent `global_domain` record carrying the Nexus
   discriminator.

4. **Validate bundle.** Re-run the verification steps above against the draft
   manifest before requesting signatures.
5. **Publish + monitor.** After governance signs the bundle, follow §3 and keep
## 5. Monitoring & Rollback

- Dashboards: `dashboards/grafana/address_ingest.json` (panels for
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`, and
  `torii_address_collision_domain_total{endpoint,domain}`) must stay
  green for 30 days before permanently gating Local-8/Local-12 traffic.
- Gate evidence: export a 30‑day Prometheus range query for
  `torii_address_local8_total` and `torii_address_collision_total` (e.g.,
  `promtool query range --output=json ...`) and run
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`;
  attach the JSON + CLI output to rollout tickets so governance can see the
  coverage window and confirm counters stayed flat.
- Alerts (see `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — pages whenever any context reports a fresh
    Local-8 increment. Treat as a release blocker until the offending client
    is remediated and telemetry is clean.
  - `AddressLocal12Collision` — fires the moment two Local-12 labels hash to
    the same digest. Pause manifest promotions, run
    `scripts/address_local_toolkit.sh` to confirm the digest mapping, and
    coordinate with Nexus governance before reissuing the affected registry
    entry.
  - `AddressInvalidRatioSlo` — warns when invalid IH58 (preferred)/sora (second-best) submissions
    exceed the 0.1 % fleet-wide SLO for ten minutes. Investigate
    `torii_address_invalid_total` by context/reason and coordinate with the
    owning SDK team before declaring the incident resolved.
- Logs: retain Torii `manifest_refresh` log lines and the governance ticket
  number in `notes.md`.
- Rollback: republish the previous bundle (same files, bumped ticket noting the
  affected environment) until the issue is resolved.

## 6. References

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (contract).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (fixture sync).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (canonical digests).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (telemetry).
