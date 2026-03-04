---
id: repair-plan
lang: es
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Espejos `docs/source/sorafs_repair_plan.md`. Mantenga ambas versiones sincronizadas hasta que se retire el conjunto Sphinx.
:::

## Ciclo de vida de las decisiones de gobernanza
1. Las reparaciones escaladas crean un borrador de propuesta de barra diagonal y abren la ventana de disputa.
2. Los votantes de gobernanza envían votos de aprobación/rechazo durante el período de disputa.
3. En `escalated_at_unix + dispute_window_secs` la decisión se calcula de manera determinista: votantes mínimos, las aprobaciones superan los rechazos y el índice de aprobación alcanza el umbral de quórum.
4. Las decisiones aprobadas abren una ventana de apelación; Las apelaciones registradas antes de `approved_at_unix + appeal_window_secs` marcan la decisión como apelada.
5. Se aplican límites a las sanciones a todas las propuestas; las presentaciones que superen el límite se rechazan.

## Política de escalada de gobernanza
La política de escalada tiene su origen en `governance.sorafs_repair_escalation` en `iroha_config` y se aplica para cada propuesta de barra diagonal de reparación.

| Configuración | Predeterminado | Significado |
|---------|---------|---------|
| `quorum_bps` | 6667 | Ratio mínimo de aprobación (puntos básicos) entre los votos escrutados. |
| `minimum_voters` | 3 | Número mínimo de votantes distintos necesarios para resolver una decisión. |
| `dispute_window_secs` | 86400 | Tiempo después de la escalada antes de que finalicen las votaciones (segundos). |
| `appeal_window_secs` | 604800 | Tiempo después de la aprobación durante el cual se aceptan apelaciones (segundos). |
| `max_penalty_nano` | 1.000.000.000 | Penalización máxima permitida para escaladas de reparación (nano-XOR). |

- Las propuestas generadas por el programador tienen un límite de `max_penalty_nano`; Las presentaciones del auditor por encima del límite se rechazan.
- Los registros de votación se almacenan en `repair_state.to` con orden determinista (clasificación `voter_id`) para que todos los nodos obtengan la misma marca de tiempo y resultado de decisión.