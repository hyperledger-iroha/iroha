# SoraNet Compliance Catalog

This directory tracks governance-approved SoraNet compliance artefacts that feed
into the SNNet-9 rollout:

- `soranet_opt_outs.json` — canonical jurisdiction and blinded-CID opt-out
  lists, signed off by the Governance Council.
- `attestations/` — jurisdictional review memos (Markdown companions to the
  signed PDFs) with Blake2b-256 digests referenced by the onboarding brief and
  compliance configs.
- `manifest.json` — audit manifest (Blake2b-512) covering the opt-out catalog
  and each jurisdictional attestation. If no signed PDF exists, the manifest
  sets `pdf_digest_b2_hex = "UNSIGNED_NO_PDF"` to make the absence explicit.

Operators merge these catalogues with their own `operator_jurisdictions` list
before wiring the resulting `compliance` block into the orchestrator or SDK
configuration. Attestation manifests remain deployment-specific and should be
recorded in the `compliance.attestations` array alongside the signed paperwork.
See `docs/source/soranet/gar_compliance_playbook.md` for the end-to-end workflow
and audit procedures.

To refresh the manifest digests:

```bash
cd governance/compliance
b2sum soranet_opt_outs.json > /tmp/opt_out_hash.txt
cd attestations
b2sum eu-2027-q2.md ca-2027-q2.md us-2027-q2.md > /tmp/attestation_hashes.txt
```

Copy the updated Blake2b-512 values into `manifest.json`, set the
`pdf_digest_b2_hex` fields to the signed PDF digests (or leave
`UNSIGNED_NO_PDF` if PDFs are not produced), and adjust `effective_from`/
`expires_at` as required by the latest council approval.
