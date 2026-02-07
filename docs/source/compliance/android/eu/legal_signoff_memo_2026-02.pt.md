---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Memorando de aprovação legal da UE - 2026.1 GA (Android SDK)

## Resumo

- **Lançamento/Treinamento:** 2026.1 GA (Android SDK)
- **Data de revisão:** 15/04/2026
- **Conselheira / Revisora:** Sofia Martins — Compliance & Jurídico
**Escopo:** meta de segurança ETSI EN 319 401, resumo GDPR DPIA, atestado SBOM, evidência de contingência de laboratório de dispositivo AND6
- **Tíquetes associados:** `_android-device-lab` / AND6-DR-202602, rastreador de governança AND6 (`GOV-AND6-2026Q1`)

## Lista de verificação de artefatos

| Artefato | SHA-256 | Localização / Link | Notas |
|----------|------------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Corresponde aos identificadores da versão 2026.1 GA e aos deltas do modelo de ameaça (adições Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Referências à política de telemetria AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | Pacote `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | Proveniência CycloneDX + revisada; corresponde ao trabalho Buildkite `android-sdk-release#4821`. |
| Registro de evidências | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (linha `android-device-lab-failover-20260220`) | Confirma hashes de pacote capturados em log + instantâneo de capacidade + entrada de memorando. |
| Pacote de contingência de laboratório de dispositivos | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hash retirado de `bundle-manifest.json`; ticket AND6-DR-202602 registrou transferência para Jurídico/Conformidade. |

## Descobertas e exceções

- Nenhum problema de bloqueio identificado. Os artefatos estão alinhados com os requisitos do ETSI/GDPR; Paridade de telemetria AND7 observada no resumo da DPIA e nenhuma mitigação adicional necessária.
- Recomendação: monitorar o exercício agendado DR-2026-05-Q2 (ticket AND6-DR-202605) e anexar o pacote resultante ao registro de evidências antes do próximo ponto de verificação de governança.

## Aprovação

- **Decisão:** Aprovada
- **Assinatura / Carimbo de data/hora:** _Sofia Martins (assinada digitalmente via portal de governança, 15/04/2026 14:32 UTC)_
- **Proprietários de acompanhamento:** Operações de laboratório de dispositivos (entregar pacote de evidências DR-2026-05-Q2 antes de 31/05/2026)