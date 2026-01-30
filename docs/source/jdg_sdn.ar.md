---
lang: ar
direction: rtl
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-01-30
---

% JDG-SDN attestations and rotation

This note captures the enforcement model for Secret Data Node (SDN) attestations
used by the Jurisdiction Data Guardian (JDG) flow.

## Commitment format
- `JdgSdnCommitment` binds the scope (`JdgAttestationScope`), the encrypted
  payload hash, and the SDN public key. Seals are typed signatures
  (`SignatureOf<JdgSdnCommitmentSignable>`) over the domain-tagged payload
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- Structural validation (`validate_basic`) enforces:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - valid block ranges
  - non-empty seals
  - scope equality against the attestation when run via
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- Deduplication is handled by the attestation validator (signer+payload hash
  uniqueness) to prevent withheld/duplicate commitments.

## Registry and rotation policy
- SDN keys live in `JdgSdnRegistry`, keyed by `(Algorithm, public_key_bytes)`.
- `JdgSdnKeyRecord` records the activation height, optional retirement height,
  and optional parent key.
- Rotation is governed by `JdgSdnRotationPolicy` (currently: `dual_publish_blocks`
  overlap window). Registering a child key updates the parent retirement to
  `child.activation + dual_publish_blocks`, with guardrails:
  - missing parents are rejected
  - activations must be strictly increasing
  - overlaps that exceed the grace window are rejected
- Registry helpers surface the installed records (`record`, `keys`) for status
  and API exposure.

## Validation flow
- `JdgAttestation::validate_with_sdn_registry` wraps the structural
  attestation checks and SDN enforcement. `JdgSdnPolicy` threads:
  - `require_commitments`: enforce presence for PII/secret payloads
  - `rotation`: grace window used when updating parent retirement
- Each commitment is checked for:
  - structural validity + attestation-scope match
  - registered key presence
  - active window covering the attested block range (retirement bounds already
    include the dual-publish grace)
  - valid seal over the domain-tagged commitment body
- Stable errors surface the index for operator evidence:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  or structural `Commitment`/`ScopeMismatch` failures.

## Operator runbook
- **Provision:** register the first SDN key with `activated_at` at or before the
  first secret block height. Publish the key fingerprint to JDG operators.
- **Rotate:** generate the successor key, register it with `rotation_parent`
  pointing at the current key, and confirm the parent retirement equals
  `child_activation + dual_publish_blocks`. Re-seal payload commitments with
  the active key during the overlap window.
- **Audit:** expose registry snapshots (`record`, `keys`) via Torii/status
  surfaces so auditors can confirm the active key and retirement windows. Alert
  if the attested range falls outside the active window.
- **Recovery:** `UnknownSdnKey` → ensure the registry includes the sealing key;
  `InactiveSdnKey` → rotate or adjust activation heights; `InvalidSeal` →
  re-seal payloads and refresh attestations.

## Runtime helper
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) packages a policy +
  registry and validates attestations via `validate_with_sdn_registry`.
- Registries can be loaded from Norito-encoded `JdgSdnKeyRecord` bundles (see
  `JdgSdnEnforcer::from_reader`/`from_path`) or assembled with
  `from_records`, which applies the rotation guardrails during registration.
- Operators can persist the Norito bundle as evidence for Torii/status
  surfacing while the same payload feeds the enforcer used by admission and
  consensus guards. A single global enforcer can be initialised at startup via
  `init_enforcer_from_path`, and `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  expose the live policy + key records for status/Torii surfaces.

## Tests
- Regression coverage in `crates/iroha_data_model/src/jurisdiction.rs`:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, alongside the existing
  structural attestation/SDN validation tests.
