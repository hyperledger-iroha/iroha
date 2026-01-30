---
lang: ar
direction: rtl
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2026-01-03T18:07:57.068055+00:00"
translation_last_reviewed: 2026-01-30
---

% SM2/SM3/SM4 Sales & Usage Filing (销售/使用备案) Template
% Hyperledger Iroha Compliance Working Group
% 2026-05-06

# Instructions

Use this template when filing deployment usage with an SCA office for onshore
operators. Provide one submission per deployment cluster or data space. Update
the placeholders with operator-specific details and attach the evidence listed
in the checklist.

# 1. Operator & Deployment Summary

| Field | Value |
|-------|-------|
| Operator name | {{ OPERATOR_NAME }} |
| Business registration ID | {{ REG_ID }} |
| Registered address | {{ ADDRESS }} |
| Primary contact (name / title / email / phone) | {{ CONTACT }} |
| Deployment identifier | {{ DEPLOYMENT_ID }} |
| Deployment location(s) | {{ LOCATIONS }} |
| Filing type | Sales / Usage (销售/使用备案) |
| Filing date | {{ YYYY-MM-DD }} |

# 2. Deployment Details

- Software build ID / hash: `{{ BUILD_HASH }}`
- Build source: {{ BUILD_SOURCE }} (e.g., operator-built from source, vendor-provided binary).
- Activation date: {{ ACTIVATION_DATE }}
- Planned maintenance windows: {{ MAINTENANCE_CADENCE }}
- Node roles participating in SM signing:
  | Node | Role | SM features enabled | Key vault location |
  |------|------|---------------------|--------------------|
  | {{ NODE_ID }} | {{ ROLE }} | {{ FEATURES }} | {{ VAULT }} |

# 3. Cryptographic Controls

- Allowed algorithms: {{ ALGORITHMS }} (ensure SM set matches configuration).
- Key lifecycle summary:
  | Stage | Description |
  |-------|-------------|
  | Generation | {{ KEY_GENERATION }} |
  | Storage | {{ KEY_STORAGE }} |
  | Rotation | {{ KEY_ROTATION }} |
  | Revocation | {{ KEY_REVOCATION }} |
- Distinct identity (`distid`) policy: {{ DISTID_POLICY }}
- Configuration excerpt (`crypto` section): provide Norito/JSON snapshot with hashes.

# 4. Telemetry & Audit Trails

- Monitoring endpoints: {{ METRICS_ENDPOINTS }} (`/metrics`, dashboards).
- Logged metrics: `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  latency histograms, error counters.
- Log retention policy: {{ LOG_RETENTION }} (≥ three years recommended).
- Audit log storage location: {{ AUDIT_STORAGE }}

# 5. Incident Response & Contacts

| Role | Name | Phone | Email | SLA |
|------|------|-------|-------|-----|
| Security operations lead | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} | {{ SLA }} |
| Crypto on-call | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} | {{ SLA }} |
| Legal / compliance | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} | {{ SLA }} |
| Vendor support (if applicable) | {{ NAME }} | {{ PHONE }} | {{ EMAIL }} | {{ SLA }} |

# 6. Attachments Checklist

- [ ] Configuration snapshot (Norito + JSON) with hashes.
- [ ] Proof of deterministic build (hashes, SBOM, reproducibility notes).
- [ ] Telemetry dashboard exports and alert definitions.
- [ ] Incident response plan and on-call rotation document.
- [ ] Operator training acknowledgement or runbook receipt.
- [ ] Export-control statement mirroring delivered artefacts.
- [ ] Copies of relevant contractual agreements or policy waivers.

# 7. Operator Declaration

> We confirm that the deployment listed above complies with PRC commercial
> cryptography regulations, that SM-enabled services follow the documented
> incident response and telemetry policies, and that audit artefacts will be
> retained for at least three years.

- Authorised signer: ________________________
- Date: ________________________

