---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-11-12T12:39:17.578044+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: priority-snapshot-2025-03
title: Snapshot de prioridades — marzo de 2025 (Beta)
description: Espejo del snapshot de steering de Nexus 2025-03; pendiente de ACKs antes del rollout publico.
---

> Fuente canonica: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Estado: **Beta / esperando ACKs de steering** (Networking, Storage, Docs leads).

## Resumen

El snapshot de marzo mantiene las iniciativas de docs/content-network alineadas
con las pistas de entrega de SoraFS (SF-3, SF-6b, SF-9). Una vez que todos los
leads confirmen el snapshot en el canal de steering de Nexus, elimina la nota
“Beta” de arriba.

### Hilos de enfoque

1. **Circular snapshot de prioridades** — recopilar acknowledgements y
   registrarlos en las minutas del council del 2025-03-05.
2. **Cierre del kickoff de Gateway/DNS** — ensayar el nuevo kit de facilitacion
   (Seccion 6 del runbook) antes del workshop 2025-03-03.
3. **Migracion de runbooks de operadores** — el portal `Runbook Index` esta live;
   expone la URL de beta preview despues del sign-off de onboarding de reviewers.
4. **Hilos de entrega de SoraFS** — alinear el trabajo restante de SF-3/6b/9 con
   el plan/roadmap:
   - Worker de ingesta PoR + endpoint de estado en `sorafs-node`.
   - Pulido de bindings CLI/SDK en integraciones de orchestrator Rust/JS/Swift.
   - Cableado de runtime del coordinador PoR y eventos de GovernanceLog.

Consulta el archivo fuente para la tabla completa, el checklist de distribucion
y las entradas de log.
