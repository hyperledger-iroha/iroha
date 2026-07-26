---
title: SoraNet GAR Compliance Playbook
summary: Operational process for maintaining jurisdictional and blinded-CID opt-out catalogues after the SNNet-9 completion.
---

# GAR Compliance Playbook (SNNet-9)

This playbook enumerates the governance and operational tasks required to keep
the SoraNet Governance Action Report (GAR) policy in sync with emerging legal
requirements. SNNet-9 is now closed; this playbook remains the canonical
reference for maintenance workstreams.

The goals are:

- publish jurisdictional opt-out catalogues so operators can prepare direct-only
  fallbacks ahead of the testnet launch;
- maintain blinded-CID opt-out lists for sensitive manifests; and
- archive compliance attestation digests so overrides remain auditable; and
- document audit procedures that prove policy updates were reviewed and applied
  deterministically across the fleet.

## Artefact Layout

| Path | Purpose | Update cadence |
|------|---------|----------------|
| `governance/compliance/soranet_opt_outs.json` | Canonical list of jurisdiction opt-outs and blinded-CID digests signed off by the Governance Council. | As required (typically quarterly) |
| `docs/examples/sorafs_compliance_policy.json` | End-to-end sample orchestrator configuration showing how to thread the compliance block. | Updated alongside SDK/CLI guidance |
| `docs/source/soranet/gar_jurisdictional_review.md` | Jurisdiction-by-jurisdiction legal review with attestation digests and guidance. | Semi-annual (or when law/policy changes) |
| `docs/source/soranet/gar_operator_onboarding_brief.md` | Operator checklist that stitches review digests, config snippets, validation, and evidence capture. | Each activation / whenever the review updates |

The canonical catalog omits `operator_jurisdictions` so each deployment can
declare its own jurisdiction footprint. Operators combine their footprint with
the catalog to build the effective policy.

## Attestation Manifests

Operators must retain signed documents proving why a jurisdiction or manifest
requires direct-only transport. Encode these artefacts in the
`compliance.attestations` array. Each entry has the following schema:

| Field | Required | Description |
|-------|----------|-------------|
| `jurisdiction` | No | ISO-3166 alpha-2 code covered by the attestation. Omit for global policies. |
| `document_uri` | Yes | Canonical URI (HTTP(S), mailto, norito) pointing to the signed paperwork. |
| `digest_hex` | Yes | Blake2b-256 digest of the attestation payload, written as a 64-character uppercase hex string. |
| `issued_at_ms` | Yes | Milliseconds since Unix epoch when the attestation entered force. |
| `expires_at_ms` | No | Optional expiry timestamp; omit when the attestation remains valid indefinitely. |

Use your standard Blake2b-256 tooling (the same algorithm backing
`iroha_crypto::Hash`) to derive the digest before updating the manifest. Retain
the original attestation documents in your governance archive alongside the hash
log.

## Update Workflow

1. **Draft update**
   - Security & Legal prepare a change proposal describing new opt-outs or
     removals.
   - A maintainer edits `governance/compliance/soranet_opt_outs.json`,
     preserving uppercase ISO-3166 alpha-2 codes and 64-character hex strings
     for blinded CIDs.
   - Operators record updated attestation entries (jurisdictional or global) in
     their deployment-specific manifests, ensuring the digests match the signed
     paperwork.
2. **Validate**
   - Run `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
     to confirm the updated lists parse correctly.
   - Run `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
     to ensure the example configuration (including attestations) remains valid.
   - Run `cargo fmt --all` to keep JSON files consistently formatted.
3. **Review & sign-off**
   - Governance Council reviews the patch and records the approval artefact
     (meeting notes or signed Norito envelope).
   - Record the approval reference in the pull request description.
4. **Publish**
   - Merge the patch and tag the repository with `gar-opt-out-YYYYMMDD`.
   - Notify operators via the standard GAR channel with the new tag, the
     changelog, and the required activation window.

## Operator Rollout Checklist

1. Pull the latest tag and read `governance/compliance/soranet_opt_outs.json`.
2. Compose a local compliance block:

   ```jsonc
   {
     "compliance": {
       "operator_jurisdictions": ["US", "CA", "DE"],
       "jurisdiction_opt_outs": ["US", "CA"],
       "blinded_cid_opt_outs": [
         "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
         "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
       ],
       "audit_contacts": [
         "mailto:governance@soranet.org",
         "https://status.soranet.org/gar-opt-outs"
       ],
       "attestations": [
         {
           "jurisdiction": "US",
           "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
           "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
           "issued_at_ms": 1805313600000,
           "expires_at_ms": 1822248000000
         },
         {
           "jurisdiction": "CA",
           "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
           "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
           "issued_at_ms": 1805313600000,
           "expires_at_ms": 1822248000000
         },
         {
           "jurisdiction": "EU",
           "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
           "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
           "issued_at_ms": 1805313600000
         }
       ]
     }
   }
   ```

3. Merge the compliance block into the orchestrator or SDK configuration (see
   the updated developer guide).
4. Record the activation in the local GAR logbook, including the timestamp,
   operator sign-off, and git tag.

## Audit Procedure

Governance audits verify that opt-outs are applied consistently:

1. Inspect the production configuration to confirm the compliance block matches
   the latest repository tag.
2. Replay orchestrator logs to ensure `compliance_jurisdiction_opt_out` or
   `compliance_blinded_cid_opt_out` events are emitted when expected and absent
   otherwise.
3. Verify that `compliance.attestations` references the signed paperwork and the
   recorded digests match the governance archive.
4. Review the operator GAR logbook entry for the activation window.
5. File the audit artefact (findings + supporting hashes) in the governance
   knowledge base.

## References

- [SoraFS Orchestrator Configuration](../sorafs/developer/orchestrator.md)
- [Gateway Compliance & Transparency Module](../sorafs_gateway_compliance_plan.md)
- [SoraNet Privacy Metrics Pipeline](privacy_metrics_pipeline.md)
- [GAR Jurisdictional Review](gar_jurisdictional_review.md)
- [GAR Operator Onboarding Brief](gar_operator_onboarding_brief.md)
