---
lang: ba
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-05T09:28:12.004169+00:00"
translation_last_reviewed: 2026-02-07
---

% SM Compliance Brief — Chinese Cryptography Law Obligations
% Iroha Compliance & Crypto Working Groups
% 2026-02-12

# Prompt

> You are LLM acting as a compliance analyst for the Hyperledger Iroha crypto and platform teams.  
> Background:  
> - Hyperledger Iroha is a Rust-based permissioned blockchain that now supports the Chinese GM/T SM2 (signatures), SM3 (hash), and SM4 (block cipher) primitives.  
> - Operators in mainland China must comply with the PRC Cryptography Law (2019), the Multi-Level Protection Scheme (MLPS 2.0), State Cryptography Administration (SCA) filing rules, and import/export controls overseen by the Ministry of Commerce (MOFCOM) and the Customs Administration.  
> - Iroha distributes open-source software internationally. Some operators will compile SM-enabled binaries domestically, while others may import pre-built artefacts.  
> Requested analysis: Summarise the key legal obligations triggered by shipping SM2/SM3/SM4 support in open-source blockchain software, including: (a) classification under the commercial vs. core/common cryptography buckets; (b) filing/approval requirements for software that implements state commercial cryptography; (c) export controls for binaries and source; (d) operational obligations on network operators (key management, logging, incident response) under MLPS 2.0. Outline concrete action items for the Iroha project (documentation, manifests, compliance statements) and for operators deploying SM-enabled nodes inside China.

# Executive Summary

- **Classification:** SM2/SM3/SM4 implementations fall under “state commercial cryptography” (商业密码) rather than “core” or “common” cryptography because they are published public algorithms sanctioned for civilian/commercial usage. Open-source distribution is permitted but subject to filing when used in commercial products or services offered in China.
- **Project obligations:** Provide algorithm provenance, deterministic build instructions, and a compliance statement noting that binaries implement state commercial cryptography. Maintain Norito manifests that flag SM capability so downstream integrators can complete filings.
- **Operator obligations:** Chinese operators must file products/services using SM algorithms with the provincial SCA bureau, complete MLPS 2.0 registration (likely Level 3 for financial networks), deploy approved key-management and logging controls, and ensure export/import declarations align with MOFCOM catalogue exemptions.

# Regulatory Landscape

| Regulation | Scope | Impact on Iroha SM support |
|------------|-------|----------------------------|
| **Cryptography Law of the PRC (2019)** | Defines core/common/commercial cryptography, mandates management system, filing, and certification. | SM2/SM3/SM4 are “commercial cryptography” and must follow filing/certification rules when provided as products/services in China. |
| **SCA Administrative Measures for Commercial Cryptography Products** | Governs production, sale, and service provision; requires product filing or certification. | Open-source software that implements SM algorithms needs operator filings when used in commercial offerings; developers should provide documentation to assist filings. |
| **MLPS 2.0 (Cybersecurity Law + MLPS regulations)** | Requires operators to classify information systems and implement security controls; Level 3 or above needs cryptography compliance evidence. | Blockchain nodes handling financial/identity data typically register at MLPS Level 3; operators must document SM usage, key management, logging, and incident handling. |
| **MOFCOM Export Control Catalogue & Customs Import Rules** | Controls export of cryptographic products, requires permits for certain algorithms/hardware. | Source code publication generally exempt under “public domain” provisions, but exporting compiled binaries with SM capability may trigger the catalogue unless shipped to approved recipients; importers must declare state commercial cryptography. |

# Key Obligations

## 1. Product & Service Filing (State Cryptography Administration)

- **Who files:** The entity providing the product/service in China (e.g., operator, SaaS provider). Open-source maintainers are not required to file, but packaging guidance must enable downstream filings.
- **Deliverables:** Algorithm description, security design docs, testing evidence, supply-chain provenance, and contact details.
- **Iroha action:** Publish an “SM cryptography statement” including algorithm coverage, deterministic build steps, dependency hashes, and contact for security inquiries.

## 2. Certification & Testing

- Certain sectors (finance, telecom, critical infrastructure) may require accredited lab testing or certification (e.g., CC-Grade/OSCCA certification).
- Include regression test artefacts demonstrating compliance with GM/T specifications.

## 3. MLPS 2.0 Operational Controls

Operators must:

1. **Register the blockchain system** with the Public Security Bureau, including cryptography usage summaries.
2. **Implement key management policies**: key generation, distribution, rotation, destruction aligned with SM2/SM4 requirements; log key lifecycle events.
3. **Enable security auditing**: capture SM-enabled transaction logs, cryptographic operation events, and anomaly detection; retain logs ≥6 months.
4. **Incident response:** maintain documented response plans that include cryptography compromise procedures and reporting timelines.
5. **Vendor management:** ensure upstream software providers (Iroha project) can supply vulnerability notifications and patches.

## 4. Import/Export Considerations

- **Open-source source code:** Typically exempt under public-domain exception, but maintainers should host downloads on servers that track access logs and include licence/disclaimer referencing state commercial cryptography.
- **Pre-built binaries:** Exporters shipping SM-enabled binaries into/out of China should confirm whether the item is covered by the “Commercial Cryptography Export Control Catalogue”. For general-purpose software without specialised hardware, a simple dual-use declaration may suffice; maintainers should not distribute binaries from jurisdictions with stricter controls unless local counsel approves.
- **Operator import:** Entities bringing binaries into China must declare cryptography usage. Provide hash manifests and SBOM to simplify customs inspection.

# Recommended Project Actions

1. **Documentation**
   - Add a compliance appendix to `docs/source/crypto/sm_program.md` noting state commercial cryptography status, filing expectations, and contact points.
   - Publish a Norito manifest field (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) that operators can use when preparing filings.
   - Ensure the Torii `/v1/node/capabilities` advert (and the `iroha runtime capabilities` CLI alias) ships with every release so operators can capture the `crypto.sm` manifest snapshot for MLPS/密评 evidence.
   - Provide bilingual (EN/ZH) compliance quickstart summarising obligations.
2. **Release Artefacts**
   - Ship SBOM/CycloneDX files for SM-enabled builds.
   - Include deterministic build scripts and reproducible Dockerfiles.
3. **Support Operator Filings**
   - Offer template letters attesting algorithm compliance (e.g., GM/T references, test coverage).
   - Maintain a security advisories mailing list to satisfy vendor-notification requirements.
4. **Internal Governance**
   - Track SM compliance checkpoints in release checklist (audit complete, documentation updated, manifest fields in place).

# Operator Action Items (China)

1. Determine if the deployment constitutes a “commercial cryptography product/service” (most enterprise networks do).
2. File product/service with provincial SCA bureau; attach Iroha compliance statement, SBOM, test reports.
3. Register blockchain system under MLPS 2.0, target Level 3 controls; integrate Iroha logs into security monitoring.
4. Establish SM key lifecycle procedures (use approved KMS/HSM where required).
5. Include cryptography compromise scenarios in incident response drills; set escalation contacts with Iroha maintainers.
6. For cross-border data flow, confirm additional CAC (Cyberspace Administration) filings if personal data is exported.

# Standalone Prompt (Copy/Paste)

> You are LLM acting as a compliance analyst for the Hyperledger Iroha crypto and platform teams.  
> Background: Hyperledger Iroha is a Rust-based permissioned blockchain that now supports the Chinese GM/T SM2 (signatures), SM3 (hash), and SM4 (block cipher) primitives. Operators in mainland China must comply with the PRC Cryptography Law (2019), the Multi-Level Protection Scheme (MLPS 2.0), State Cryptography Administration (SCA) filing rules, and import/export controls overseen by MOFCOM and the Customs Administration. The Iroha project distributes SM-enabled open-source software internationally; some operators compile binaries domestically, while others import pre-built artefacts.  
> Task: Summarise the legal obligations triggered by shipping SM2/SM3/SM4 support in open-source blockchain software. Cover the classification of these algorithms (commercial vs. core/common cryptography), required filings or certifications for software products, export/import controls relevant to source and binaries, and operational duties for network operators under MLPS 2.0 (key management, logging, incident response). Provide concrete action items for the Iroha project (documentation, manifests, compliance statements) and for operators deploying SM-enabled nodes inside China.
