---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 302b74b4022656e57c2b876a8f15bf5301a593030a18ad1b93780061e5d783ef
source_last_modified: "2026-01-21T19:17:13.232211+00:00"
translation_last_reviewed: 2026-02-07
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
---

:::nota Fonte Canônica
Espelhos `docs/source/sorafs_repair_plan.md`. Mantenha as duas versões sincronizadas até que o conjunto Sphinx seja retirado.
:::

## Ciclo de vida da decisão de governança
1. Reparos escalonados criam um rascunho de proposta de barra e abrem a janela de disputa.
2. Os eleitores da governança enviam votos de aprovação/rejeição durante a janela de disputa.
3. Em `escalated_at_unix + dispute_window_secs` a decisão é computada de forma determinística: mínimo de eleitores, aprovações superam as rejeições e a taxa de aprovação atende ao limite de quórum.
4. Decisões aprovadas abrem janela de recurso; recursos registrados antes de `approved_at_unix + appeal_window_secs` marcam a decisão como apelada.
5. Limites de penalidade se aplicam a todas as propostas; submissões acima do limite são rejeitadas.

## Política de escalonamento de governança
A política de escalonamento é proveniente de `governance.sorafs_repair_escalation` em `iroha_config` e é aplicada para cada proposta de barra de reparo.

| Configuração | Padrão | Significado |
|--------|---------|---------|
| `quorum_bps` | 6667 | Proporção mínima de aprovação (pontos base) entre os votos apurados. |
| `minimum_voters` | 3 | Número mínimo de eleitores distintos necessários para resolver uma decisão. |
| `dispute_window_secs` | 86400 | Tempo após a escalada antes da finalização dos votos (segundos). |
| `appeal_window_secs` | 604800 | Tempo após a aprovação durante o qual as apelações são aceitas (segundos). |
| `max_penalty_nano` | 1.000.000.000 | Penalidade de barra máxima permitida para escalonamentos de reparo (nano-XOR). |

- As propostas geradas pelo agendador são limitadas a `max_penalty_nano`; as submissões do auditor acima do limite são rejeitadas.
- Os registros de votação são armazenados em `repair_state.to` com ordenação determinística (classificação `voter_id`) para que todos os nós obtenham o mesmo carimbo de data/hora de decisão e resultado.