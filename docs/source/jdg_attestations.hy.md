---
lang: hy
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
---

# JDG Attestations: Guard, Rotation, and Retention

This note documents the v1 JDG attestation guard that now ships in `iroha_core`.

- **Committee manifests:** Norito-encoded `JdgCommitteeManifest` bundles carry per-dataspace rotation
  schedules (`committee_id`, ordered members, threshold, `activation_height`, `retire_height`).
  Manifests are loaded with `JdgCommitteeSchedule::from_path` and enforce strictly increasing
  activation heights with an optional grace overlap (`grace_blocks`) between retiring/activating
  committees.
- **Attestation guard:** `JdgAttestationGuard` enforces dataspace binding, expiry, stale bounds,
  committee id/threshold matching, signer membership, supported signature schemes, and optional
  SDN validation via `JdgSdnEnforcer`. Size caps, max lag, and allowed signature schemes are
  constructor parameters; `validate(attestation, dataspace, current_height)` returns the active
  committee or a structured error.
  - `scheme_id = 1` (`simple_threshold`): per-signer signatures, optional signer bitmap.
  - `scheme_id = 2` (`bls_normal_aggregate`): single pre-aggregated BLS-normal signature over the
    attestation hash; signer bitmap optional, defaults to all signers in the attestation. BLS
    aggregate validation requires a valid PoP per committee member in the manifest; missing or
    invalid PoPs reject the attestation.
  Configure the allow-list via `governance.jdg_signature_schemes`.
- **Retention store:** `JdgAttestationStore` tracks attestations per dataspace with a configurable
  per-dataspace cap, pruning oldest entries on insert. Call `for_dataspace` or
  `for_dataspace_and_epoch` to retrieve audit/replay bundles.
- **Tests:** Unit coverage now exercises valid committee selection, unknown signer rejection, stale
  attestation rejection, unsupported scheme ids, and retention pruning. See
  `crates/iroha_core/src/jurisdiction.rs`.

The guard rejects schemes outside the configured allow-list.
