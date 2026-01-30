---
lang: ur
direction: rtl
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Security Target — ETSI EN 319 401 Alignment

| Field | Value |
|-------|-------|
| Document Version | 0.1 (2026-02-12) |
| Scope | Android SDK (client libraries under `java/iroha_android/` plus supporting scripts/docs) |
| Owner | Compliance & Legal (Sofia Martins) |
| Reviewers | Android Program Lead, Release Engineering, SRE Governance |

## 1. TOE Description

The Target of Evaluation (TOE) comprises the Android SDK library code (`java/iroha_android/src/main/java`), its configuration surface (`ClientConfig` + Norito ingestion), and the operational tooling referenced in `roadmap.md` for milestones AND2/AND6/AND7.

Primary components:

1. **Configuration ingestion** — `ClientConfig` threads Torii endpoints, TLS policies, retries, and telemetry hooks from the generated `iroha_config` manifest and enforces immutability post-initialisation (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`).
2. **Key management / StrongBox** — Hardware-backed signing is implemented via `SystemAndroidKeystoreBackend` and `AttestationVerifier`, with policies documented in `docs/source/sdk/android/key_management.md`. Attestation capture/validation uses `scripts/android_keystore_attestation.sh` and the CI helper `scripts/android_strongbox_attestation_ci.sh`.
3. **Telemetry & redaction** — Instrumentation funnels through the shared schema described in `docs/source/sdk/android/telemetry_redaction.md`, exporting hashed authorities, bucketed device profiles, and override auditing hooks enforced by the Support Playbook.
4. **Operations runbooks** — `docs/source/android_runbook.md` (operator response) and `docs/source/android_support_playbook.md` (SLA + escalation) harden the TOE’s operational footprint with deterministic overrides, chaos drills, and evidence capture.
5. **Release provenance** — Gradle-based builds use the CycloneDX plugin plus reproducible build flags as captured in `docs/source/sdk/android/developer_experience_plan.md` and the AND6 compliance checklist. Release artefacts are signed and cross-referenced in `docs/source/release/provenance/android/`.

## 2. Assets & Assumptions

| Asset | Description | Security Objective |
|-------|-------------|--------------------|
| Configuration manifests | Norito-derived `ClientConfig` snapshots distributed with apps. | Authenticity, integrity, and confidentiality at rest. |
| Signing keys | Keys generated or imported through StrongBox/TEE providers. | StrongBox preference, attestation logging, no key export. |
| Telemetry streams | OTLP traces/logs/metrics exported from SDK instrumentation. | Pseudonymisation (hashed authorities), minimised PII, override auditing. |
| Ledger interactions | Norito payloads, admission metadata, Torii network traffic. | Mutual authentication, replay-resistant requests, deterministic retries. |

Assumptions:

- Mobile OS provides standard sandboxing + SELinux; StrongBox devices implement Google’s keymaster interface.
- Operators provision Torii endpoints with TLS certificates signed by council-trusted CAs.
- Build infrastructure honours reproducible-build requirements before publishing to Maven.

## 3. Threats & Controls

| Threat | Control | Evidence |
|--------|---------|----------|
| Tampered configuration manifests | `ClientConfig` validates manifests (hash + schema) before applying and logs denied reloads via `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| Compromise of signing keys | StrongBox-required policies, attestation harnesses, and device-matrix audits identify drift; overrides documented per incident. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| PII leakage in telemetry | Blake2b-hashed authorities, bucketed device profiles, carrier omission, override logging. | `docs/source/sdk/android/telemetry_redaction.md`; Support Playbook §8. |
| Replay or downgrade on Torii RPC | `/v1/pipeline` request builder enforces TLS pinning, noise channel policy, and retry budgets with hashed authority context. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (planned). |
| Unsigned or non-reproducible releases | CycloneDX SBOM + Sigstore attestations gated by AND6 checklist; release RFCs require evidence in `docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| Incomplete incident handling | Runbook + playbook define overrides, chaos drills, and escalation tree; telemetry overrides require signed Norito requests. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. Evaluation Activities

1. **Design review** — Compliance + SRE verify that configuration, key management, telemetry, and release controls map to ETSI security objectives.
2. **Implementation checks** — Automated tests:
   - `scripts/android_strongbox_attestation_ci.sh` verifies captured bundles for every StrongBox device listed in the matrix.
   - `scripts/check_android_samples.sh` and Managed Device CI ensure sample apps honour `ClientConfig`/telemetry contracts.
3. **Operational validation** — Quarterly chaos drills per `docs/source/sdk/android/telemetry_chaos_checklist.md` (redaction + override exercises).
4. **Evidence retention** — Artefacts stored under `docs/source/compliance/android/` (this folder) and referenced from `status.md`.

## 5. ETSI EN 319 401 Mapping

| EN 319 401 Clause | SDK Control |
|-------------------|-------------|
| 7.1 Security policy | Documented in this security target + Support Playbook. |
| 7.2 Organisational security | RACI + on-call ownership in Support Playbook §2. |
| 7.3 Asset management | Configuration, key, and telemetry asset objectives defined in §2 above. |
| 7.4 Access control | StrongBox policies + override workflow requiring signed Norito artefacts. |
| 7.5 Cryptographic controls | Key generation, storage, and attestation requirements from AND2 (key management guide). |
| 7.6 Operations security | Telemetry hashing, chaos rehearsals, incident response, and release evidence gating. |
| 7.7 Communications security | `/v1/pipeline` TLS policy + hashed authorities (telemetry redaction doc). |
| 7.8 System acquisition / development | Reproducible Gradle builds, SBOMs, and provenance gates in AND5/AND6 plans. |
| 7.9 Supplier relationships | Buildkite + Sigstore attestations recorded alongside third-party dependency SBOMs. |
| 7.10 Incident management | Runbook/Playbook escalation, override logging, telemetry fail counters. |

## 6. Maintenance

- Update this document whenever the SDK introduces new cryptographic algorithms, telemetry categories, or release automation changes.
- Link signed copies in `docs/source/compliance/android/evidence_log.csv` with SHA-256 digests and reviewer sign-offs.
