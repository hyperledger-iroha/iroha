---
lang: pt
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2026-01-03T18:07:59.250950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Registro de contingência do laboratório de dispositivos

Registre aqui todas as ativações do plano de contingência do laboratório de dispositivos Android.
Inclua detalhes suficientes para análises de conformidade e futuras auditorias de preparação.

| Data | Gatilho | Ações tomadas | Acompanhamentos | Proprietário |
|------|--------|---------------|------------|-------|
| 11/02/2026 | A capacidade caiu para 78% após a interrupção da pista do Pixel8 Pro e atraso na entrega do Pixel8a (consulte `android_strongbox_device_matrix.md`). | Promoveu a pista Pixel7 para alvo primário de CI, emprestou frota compartilhada de Pixel6, agendou testes de fumaça do Firebase Test Lab para amostra de carteira de varejo e contratou laboratório externo StrongBox de acordo com o plano AND6. | Substitua o hub USB-C com defeito para Pixel8 Pro (com vencimento em 15/02/2026); confirme a chegada do Pixel8a e o relatório de capacidade de rebaseline. | Líder de laboratório de hardware |
| 13/02/2026 | Hub Pixel8 Pro substituído e GalaxyS24 aprovado, restaurando a capacidade para 85%. | Faixa Pixel7 retornada para o trabalho `android-strongbox-attestation` Buildkite secundário e reativado com tags `pixel8pro-strongbox-a` e `s24-strongbox-a`, matriz de prontidão atualizada + registro de evidências. | Monitore o ETA de entrega do Pixel8a (ainda pendente); mantenha o inventário de hubs sobressalentes documentado. | Líder de laboratório de hardware |