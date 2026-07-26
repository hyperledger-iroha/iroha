---
lang: pt
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Modelo de memorando de assinatura legal da UE

Este memorando registra a revisão legal exigida pelo item do roteiro **AND6** antes do
O pacote de artefatos da UE (ETSI/GDPR) é submetido aos reguladores. O advogado deve clonar
este modelo por versão, preencha os campos abaixo e armazene a cópia assinada
ao lado dos artefatos imutáveis mencionados no memorando.

## Resumo

- **Liberação / Trem:** `<e.g., 2026.1 GA>`
- **Data da revisão:** `<YYYY-MM-DD>`
- **Conselheiro/Revisor:** `<name + organisation>`
- **Escopo:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Tíquetes associados:** `<governance or legal issue IDs>`

## Lista de verificação de artefatos

| Artefato | SHA-256 | Localização / Link | Notas |
|----------|------------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + arquivo de governança | Confirme os identificadores de lançamento e os ajustes do modelo de ameaça. |
| `gdpr_dpia_summary.md` | `<hash>` | Mesmo diretório/espelhos de localização | Certifique-se de que as referências da política de redação correspondam a `sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | Mesmo diretório + pacote de fiança no balde de evidências | Verifique as assinaturas de proveniência do CycloneDX +. |
| Linha do registo de provas | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Número da linha `<n>` |
| Pacote de contingência de laboratório de dispositivos | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Confirma o ensaio de failover vinculado a esta versão. |

> Anexe linhas adicionais se o pacote contiver mais arquivos (por exemplo, privacidade
> apêndices ou traduções da DPIA). Todo artefato deve fazer referência ao seu imutável
> carregue o destino e o trabalho do Buildkite que o produziu.

## Descobertas e exceções

- `None.` *(Substituir por lista com marcadores cobrindo riscos residuais, compensando
  controles ou ações de acompanhamento necessárias.)*

## Aprovação

- **Decisão:** `<Approved / Approved with conditions / Blocked>`
- **Assinatura/carimbo de data/hora:** `<digital signature or email reference>`
- **Proprietários de acompanhamento:** `<team + due date for any conditions>`

Carregue o memorando final para o depósito de evidências de governança, copie o SHA-256 em
`docs/source/compliance/android/evidence_log.csv` e vincule o caminho de upload em
`status.md`. Se a decisão for “Bloqueada”, passe para a direção AND6
etapas de correção de documentos e comitês tanto na lista de prioridades do roteiro quanto no
log de contingência do laboratório do dispositivo.