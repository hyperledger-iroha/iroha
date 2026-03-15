---
lang: pt
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificação de controles de segurança FISC – Android SDK

| Campo | Valor |
|-------|-------|
| Versão | 0,1 (12/02/2026) |
| Escopo | Ferramentas de operador Android SDK + usadas em implantações financeiras japonesas |
| Proprietários | Conformidade e Jurídico (Daniel Park), líder do programa Android |

## Matriz de Controle

| Controle FISC | Detalhe de implementação | Evidências/Referências | Estado |
|--------------|-----------------------|------------|--------|
| **Integridade da configuração do sistema** | `ClientConfig` impõe hash de manifesto, validação de esquema e acesso de tempo de execução somente leitura. Falhas de recarga de configuração emitem eventos `android.telemetry.config.reload` documentados no runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Implementado |
| **Controle de acesso e autenticação** | O SDK respeita as políticas TLS Torii e as solicitações assinadas `/v1/pipeline`; referência de fluxos de trabalho do operador Support Playbook §4–5 para escalonamento e substituição de controle por meio de artefatos Norito assinados. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (substituir fluxo de trabalho). | ✅ Implementado |
| **Gerenciamento de chaves criptográficas** | Provedores preferenciais do StrongBox, validação de atestado e cobertura de matriz de dispositivos garantem a conformidade com KMS. Saídas de chicote de atestado arquivadas em `artifacts/android/attestation/` e rastreadas na matriz de prontidão. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Implementado |
| **Registro, monitoramento e retenção** | A política de redação de telemetria faz hash de dados confidenciais, agrupa atributos do dispositivo e impõe retenção (janelas de 30/07/90/365 dias). O Support Playbook §8 descreve os limites do painel; substituições registradas em `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Implementado |
| **Operações e gerenciamento de mudanças** | O procedimento de transferência do GA (Support Playbook §7.2) mais as atualizações `status.md` rastreiam a prontidão do lançamento. Evidência de liberação (pacotes SBOM, Sigstore) vinculadas via `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Implementado |
| **Resposta e relatórios de incidentes** | O Playbook define a matriz de gravidade, as janelas de resposta do SLA e as etapas de notificação de conformidade; substituições de telemetria + ensaios de caos garantem a reprodutibilidade antes dos pilotos. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Implementado |
| **Residência/localização de dados** | Coletores de telemetria para implantações JP são executados na região aprovada de Tóquio; Pacotes de atestado StrongBox armazenados na região e referenciados em tickets de parceiros. O plano de localização garante a disponibilidade de documentos em japonês antes da versão beta (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Em andamento (localização em andamento) |

## Notas do revisor

- Verifique as entradas da matriz do dispositivo para Galaxy S23/S24 antes da integração regulamentada do parceiro (consulte as linhas do documento de preparação `s23-strongbox-a`, `s24-strongbox-a`).
- Garanta que os coletores de telemetria em implantações JP apliquem a mesma lógica de retenção/substituição definida no DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Obtenha a confirmação dos auditores externos assim que os parceiros bancários analisarem esta lista de verificação.