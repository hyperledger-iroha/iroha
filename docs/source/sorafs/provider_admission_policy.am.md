---
lang: am
direction: ltr
source: docs/source/sorafs/provider_admission_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d8898043c9248db9ee6718b8a971f28413bbe0d91a422a1051a0d98d8d2f7c
source_last_modified: "2025-12-29T18:16:36.122533+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Provider Admission & Identity Policy (SF-2b Draft)

This note captures the actionable deliverables for **SF-2b**: defining and
enforcing the admission workflow, identity requirements, and attestation
payloads for SoraFS storage providers. It expands the high-level process
outlined in the SoraFS Architecture RFC and breaks the remaining work into
trackable engineering tasks.

## Policy Goals

- Ensure only vetted operators can publish `ProviderAdvertV1` records that the
  network will accept.
- Bind every advertisement key to a governance-approved identity document,
  attested endpoints, and minimum stake contribution.
- Provide deterministic verification tooling so Torii, gateways, and
  `sorafs-node` enforce the same checks.
- Support renewal and emergency revocation without breaking determinism or
  tooling ergonomics.

## Identity & Stake Requirements

| Requirement | Description | Deliverable |
|-------------|-------------|-------------|
| Advertisement key provenance | Providers must register an Ed25519 keypair that signs every advert. The admission bundle stores the public key alongside a governance signature. | Extend `ProviderAdmissionProposalV1` schema with `advert_key` (32 bytes) and reference it from the registry (`sorafs_manifest::provider_admission`). |
| Stake pointer | Admission requires a non-zero `StakePointer` pointing at an active staking pool. | Add validation in `sorafs_manifest::provider_advert::StakePointer::validate()` and surface errors in CLI/tests. |
| Jurisdiction tags | Providers declare jurisdiction + legal contact. | Extend proposal schema with a `jurisdiction_code` (ISO 3166-1 alpha-2) and optional `contact_uri`. |
| Endpoint attestation | Each advertised endpoint must be backed by an mTLS or QUIC certificate report. | Define `EndpointAttestationV1` Norito payload and store per endpoint inside the admission bundle. |

## Admission Workflow

1. **Proposal creation**
   - CLI: add `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     producing `ProviderAdmissionProposalV1` + attestation bundle.
   - Validation: ensure required fields, stake > 0, canonical chunker handle in `profile_id`.
2. **Governance endorsement**
   - Council signs `blake3("sorafs-provider-admission-v1" || canonical_bytes)` using existing
     envelope tooling (`sorafs_manifest::governance` module).
   - Envelope is persisted to `governance/providers/<provider_id>/admission.json`.
3. **Registry ingestion**
   - Implement a shared verifier (`sorafs_manifest::provider_admission::validate_envelope`)
     that Torii/gateways/CLI re-use.
   - Update Torii admission path to reject adverts whose digest or expiry differs from the envelope.
4. **Renewal & revocation**
   - Add `ProviderAdmissionRenewalV1` with optional endpoint/stake updates.
   - Expose a `--revoke` CLI path that records the revocation reason and pushes a governance event.

## Implementation Tasks

| Area | Task | Owner(s) | Status |
|------|------|----------|--------|
| Schema | Define `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) under `crates/sorafs_manifest/src/provider_admission.rs`. Implemented in `sorafs_manifest::provider_admission` with validation helpers.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ Completed |
| CLI tooling | Extend `sorafs_manifest_stub` with subcommands: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

The CLI flow now accepts intermediate certificate bundles (`--endpoint-attestation-intermediate`), emits
canonical proposal/envelope bytes, and validates council signatures during `sign`/`verify`. Operators can
provide advert bodies directly, or reuse signed adverts, and signature files may be supplied by pairing
`--council-signature-public-key` with `--council-signature-file` for automation friendliness.

### CLI Reference

Run each command via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …`.

- `proposal`
  - Required flags: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, and at least one `--endpoint=<kind:host>`.
  - Per-endpoint attestation expects `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, a certificate via
    `--endpoint-attestation-leaf=<path>` (plus optional `--endpoint-attestation-intermediate=<path>`
    for each chain element) and any negotiated ALPN IDs
    (`--endpoint-attestation-alpn=<token>`). QUIC endpoints may supply transport reports with
    `--endpoint-attestation-report[-hex]=…`.
  - Output: canonical Norito proposal bytes (`--proposal-out`) and a JSON summary
    (default stdout or `--json-out`).
- `sign`
  - Inputs: a proposal (`--proposal`), a signed advert (`--advert`), optional advert body
    (`--advert-body`), retention epoch, and at least one council signature. Signatures can be provided
    inline (`--council-signature=<signer_hex:signature_hex>`) or via files by combining
    `--council-signature-public-key` with `--council-signature-file=<path>`.
  - Produces a validated envelope (`--envelope-out`) and JSON report indicating digest bindings,
    signer count, and input paths.
- `verify`
  - Validates an existing envelope (`--envelope`), optionally checking the matching proposal,
    advert, or advert body. The JSON report highlights digest values, signature verification status,
    and which optional artefacts matched.
- `renewal`
  - Links a newly approved envelope to the previously ratified digest. Requires
    `--previous-envelope=<path>` and the successor `--envelope=<path>` (both Norito payloads).
    The CLI verifies that profile aliases, capabilities, and advert keys remain unchanged while
    allowing stake, endpoints, and metadata updates. Outputs the canonical
    `ProviderAdmissionRenewalV1` bytes (`--renewal-out`) plus a JSON summary.
- `revoke`
  - Issues an emergency `ProviderAdmissionRevocationV1` bundle for a provider whose envelope must
    be withdrawn. Requires `--envelope=<path>`, `--reason=<text>`, at least one
    `--council-signature`, and optional `--revoked-at`/`--notes`. The CLI signs and validates the
    revocation digest, writes the Norito payload via `--revocation-out`, and prints a JSON report
    capturing the digest and signature count.
| Verification | Implement shared verifier used by Torii, gateways, and `sorafs-node`. Provide unit + CLI integration tests.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ Completed |
| Torii integration | Thread verifier into Torii advertisement ingestion, reject out-of-policy adverts, emit telemetry. | Networking TL | ✅ Completed | Torii now loads governance envelopes (`torii.sorafs.admission_envelopes_dir`), verifies digest/signature matches during ingestion, and surfaces admission telemetry.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renewal | Add renewal / revocation schema + CLI helpers, publish lifecycle guide in docs (see runbook below and CLI commands in `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ Completed |
| Telemetry | Define `provider_admission` dashboards & alerts (missing renewal, envelope expiry). | Observability | 🟠 In progress | Counter `torii_sorafs_admission_total{result,reason}` exists; dashboards/alerts pending.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Renewal & Revocation Runbook

#### Scheduled renewal (stake/topology updates)
1. Build the successor proposal/advert pair with `provider-admission proposal` and `provider-admission sign`, increasing `--retention-epoch` and updating stake/endpoints as required.
2. Execute  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   The command validates unchanged capability/profile fields via
   `AdmissionRecord::apply_renewal`, emits `ProviderAdmissionRenewalV1`, and prints digests for the
   governance log.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Replace the previous envelope in `torii.sorafs.admission_envelopes_dir`, commit the renewal Norito/JSON to the governance repository, and append the renewal hash + retention epoch to `docs/source/sorafs/migration_ledger.md`.
4. Notify operators that the new envelope is live and monitor `torii_sorafs_admission_total{result="accepted",reason="stored"}` to confirm ingestion.
5. Regenerate and commit the canonical fixtures via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) validates the Norito outputs stay stable.

#### Emergency revocation
1. Identify the compromised envelope and issue a revocation:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   The CLI signs the `ProviderAdmissionRevocationV1`, verifies the signature set via
   `verify_revocation_signatures`, and reports the revocation digest.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Remove the envelope from `torii.sorafs.admission_envelopes_dir`, distribute the revocation Norito/JSON to admission caches, and record the reason hash in the governance minutes.
3. Watch `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` to confirm caches drop the revoked advert; keep the revocation artefacts in incident retrospectives.

## Testing & Telemetry

- Add golden fixtures for admission proposals and envelopes under
  `fixtures/sorafs_manifest/provider_admission/`.
- Extend CI (`ci/check_sorafs_fixtures.sh`) to regenerate proposals and verify envelopes.
- Generated fixtures include `metadata.json` with canonical digests; downstream tests assert
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Provide integration tests:
  - Torii rejects adverts with missing or expired admission envelopes.
  - CLI round-trips a proposal → envelope → verification.
  - Governance renewal rotates endpoint attestation without changing provider ID.
- Telemetry requirements:
  - Emit `provider_admission_envelope_{accepted,rejected}` counters in Torii. ✅ `torii_sorafs_admission_total{result,reason}` now surfaces accepted/rejected outcomes.
  - Add expiry warnings to observability dashboards (renewal due within 7 days).

## Next Steps

1. ✅ Finalised the Norito schema changes and landed validation helpers in
   `sorafs_manifest::provider_admission`. No feature flags required.
2. ✅ CLI workflows (`proposal`, `sign`, `verify`, `renewal`, `revoke`) are documented and exercised via integration tests; keep governance scripts in sync with the runbook.
3. ✅ Torii admission/discovery ingest the envelopes and expose telemetry counters for acceptance/rejection.
4. Focus on observability: finish the admission dashboards/alerts so renewals due within seven days raise warnings (`torii_sorafs_admission_total`, expiry gauges).
