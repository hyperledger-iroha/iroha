---
lang: mn
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2025-12-29T18:16:35.942266+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM2/SM3/SM4 Compliance & Export Brief

This brief supplements the architecture notes in `docs/source/crypto/sm_program.md`
and provides actionable guidance for engineering, Ops, and legal teams as the
GM/T algorithm family moves from verify-only preview to broader enablement.

## Summary
- **Regulatory basis:** China’s *Cryptography Law* (2019), *Cybersecurity Law*, and
  *Data Security Law* classify SM2/SM3/SM4 as “commercial cryptography” when deployed
  onshore. Operators must file usage reports, and certain sectors require accredited
  testing prior to production use.
- **International controls:** Outside China the algorithms fall under US EAR Category
  5 Part 2, EU 2021/821 Annex 1 (5D002), and similar national regimes. Open-source
  publication typically qualifies for license exceptions (ENC/TSU), but binaries
  shipped to embargoed regions remain controlled exports.
- **Project policy:** SM features remain disabled by default. Signing functionality
  will only be enabled after external audit closure, deterministic perf/telemetry
  gating, and operator documentation (this brief) land.

## Required Actions by Function
| Team | Responsibilities | Artifacts | Owners |
|------|------------------|-----------|--------|
| Crypto WG | Track GM/T spec updates, coordinate third-party audits, maintain deterministic policy (nonce derivation, canonical r∥s). | `sm_program.md`, audit reports, fixture bundles. | Crypto WG lead |
| Release Engineering | Gate SM features behind explicit config, maintain verify-only default, manage feature rollout checklist. | `release_dual_track_runbook.md`, release manifests, rollout ticket. | Release TL |
| Ops / SRE | Provide SM enablement checklist, telemetry dashboards (usage, error rates), incident response plan. | Runbooks, Grafana dashboards, onboarding tickets. | Ops/SRE |
| Legal Liaison | File PRC development/usage reports when nodes run in mainland China; review export posture for each bundle. | Filing templates, export statements. | Legal contact |
| SDK Program | Surface SM algorithm support consistently, enforce deterministic behaviour, propagate compliance notes to SDK docs. | SDK release notes, docs, CI gating. | SDK leads |

## Documentation & Filing Requirements (China)
1. **Product filing (开发备案):** For onshore development, submit product description,
   source availability statement, dependency list, and deterministic build steps to
   the provincial cryptography administration before release.
2. **Sales/Usage filing (销售/使用备案):** Operators running SM-enabled nodes must
   register usage scope, key management, and telemetry collection with the same
   authority. Provide contact info and incident response SLAs.
3. **Certification (检测/认证):** Critical infrastructure operators may require
   accredited testing. Provide reproducible build scripts, SBOM, and test reports
   so downstream integrators can complete certification without altering code.
4. **Record-keeping:** Archive filings and approvals in the compliance tracker.
   Update `status.md` when new regions or operators complete the process.

## Compliance Checklist

### Before enabling SM features
- [ ] Confirm legal counsel reviewed the target deployment regions.
- [ ] Capture deterministic build instructions, dependency manifests, and SBOM
      exports for inclusion with filings.
- [ ] Align `crypto.allowed_signing`, `crypto.default_hash`, and admission
      policy manifests with the rollout ticket.
- [ ] Produce operator communications describing SM feature scope,
      enablement prerequisites, and fallback plans for disablement.
- [ ] Export telemetry dashboards covering SM verification/signature counters,
      error rates, and perf metrics (`sm3`, `sm4`, syscall timing).
- [ ] Prepare incident response contacts and escalation paths for onshore
      operators and the Crypto WG.

### Filing & audit readiness
- [ ] Select the appropriate filing template (product vs. sales/usage) and fill
      in the release metadata before submission.
- [ ] Attach SBOM archives, deterministic test transcripts, and manifest hashes.
- [ ] Ensure the export-control statement reflects the exact artefacts being
      delivered and cites license exceptions relied upon (ENC/TSU).
- [ ] Verify that audit reports, remediation tracking, and operator runbooks
      are linked from the filing packet.
- [ ] Store signed filings, approvals, and correspondence in the compliance
      tracker with versioned references.

### Post-approval operations
- [ ] Update `status.md` and the rollout ticket once the filing is accepted.
- [ ] Re-run telemetry validation to confirm observability coverage matches
      the filing inputs.
- [ ] Schedule periodic review (at least annually) of filings, audit reports,
      and export statements to capture spec/regulatory updates.
- [ ] Trigger filing addendums whenever configuration, feature scope, or hosting
      footprint changes materially.

## Export & Distribution Guidance
- Include a short export statement in release notes/manifests referencing reliance
  on ENC/TSU. Example:
  > “This release contains SM2/SM3/SM4 implementations. Distribution follows ENC
  > (15 CFR Part 742) / EU 2021/821 Annex 1 5D002. Operators must ensure compliance
  > with local export/import laws.”
- For builds hosted inside China, coordinate with Ops to publish artifacts from
  onshore infrastructure; avoid cross-border transfer of SM-enabled binaries unless
  the appropriate licenses are in place.
- When mirroring to package repositories, record which artefacts include SM features
  to simplify compliance reporting.

## Operator Checklist
- [ ] Confirm release profile (`scripts/select_release_profile.py`) + SM feature flag.
- [ ] Review `sm_program.md` and this brief; ensure legal filings are recorded.
- [ ] Enable SM features by compiling with `sm`, updating `crypto.allowed_signing` to include `sm2`, and switching `crypto.default_hash` to `sm3-256` only after determinism safeguards are in place and audit status is green.
- [ ] Update telemetry dashboards/alerts to include SM counters (verification failures,
      signing requests, perf metrics).
- [ ] Keep manifests, hash/signature proofs, and filing confirmations attached to
      the rollout ticket.

## Sample Filing Templates

Templates live under `docs/source/crypto/attachments/` for easy inclusion in
filing packets. Copy the relevant Markdown template into the operator’s change
log or export it to PDF as required by local authorities.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) —
  provincial product filing (开发备案) form capturing release metadata, algorithms,
  SBOM references, and support contacts.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  operator sales/usage filing (销售/使用备案) outlining deployment footprint,
  key management, telemetry, and incident response procedures.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) —
  export-control declaration suitable for release notes, manifests, or legal
  correspondence relying on ENC/TSU license exceptions.

## Standards & Citations
- **GM/T 0002-2012 / GB/T 32907-2016** — SM4 block cipher and AEAD parameters (ECB/GCM/CCM). Matches the vectors captured in `docs/source/crypto/sm_vectors.md`.
- **GM/T 0003-2012 / GB/T 32918.x-2016** — SM2 public-key cryptography, curve parameters, signature/verification process, and Annex D known-answer tests.
- **GM/T 0004-2012 / GB/T 32905-2016** — SM3 hash function specification and conformance vectors.
- **RFC 8998** — SM2 key exchange and signature use in TLS; cite when documenting interop with OpenSSL/Tongsuo.
- **Cryptography Law of the People’s Republic of China (2019)**, **Cybersecurity Law (2017)**, **Data Security Law (2021)** — Legal basis for the filing workflow noted above.
- **US EAR Category 5 Part 2** and **EU Regulation 2021/821 Annex 1 (5D002)** — Export-control regimes governing SM-enabled binaries.
- **Iroha artefacts:** `scripts/sm_interop_matrix.sh` and `scripts/sm_openssl_smoke.sh` provide deterministic interop transcripts that auditors can replay before signing compliance reports.

## References
- `docs/source/crypto/sm_program.md` — technical architecture and policy.
- `docs/source/release_dual_track_runbook.md` — release gating and rollout process.
- `docs/source/sora_nexus_operator_onboarding.md` — sample operator onboarding flow.
- GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, GB/T 32918 series, RFC 8998.

Questions? Contact the Crypto WG or Legal liaison via the SM rollout tracker.
