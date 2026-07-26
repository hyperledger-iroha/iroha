---
lang: zh-hant
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-12-29T18:16:36.206648+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraGlobal Gateway hardening (SNNet-15H)

The hardening helper captures security/privacy evidence before promoting
Gateway builds.

## Command
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Outputs
- `gateway_hardening_summary.json` — status per input (SBOM, vuln report, HSM policy, sandbox profile) plus retention signal. Missing inputs show `warn` or `error`.
- `gateway_hardening_summary.md` — human-readable rollup for governance packets.

## Acceptance notes
- SBOM and vuln reports must exist; absent inputs downgrade the status.
- Retention over 30 days marks `warn` for review; supply stricter defaults before GA.
- Use the summary artefacts as attachments for GAR/SOC review and incident runbooks.
