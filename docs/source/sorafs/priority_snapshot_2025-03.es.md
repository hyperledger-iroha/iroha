---
lang: es
direction: ltr
source: docs/source/sorafs/priority_snapshot_2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bacc9e157599e0abb78346e5300d16f6062ddbad81207e55097698940fb9b14
source_last_modified: "2025-11-14T18:19:35.970535+00:00"
translation_last_reviewed: "2026-01-30"
---

# Por que existe este snapshot
- Capturar las prioridades exactas de docs/content-network solicitadas por el grupo directivo de Nexus antes de la sesion de governance de marzo.
- Proveer un unico link referenciado por `roadmap.md` (tabla Near-Term Execution) para que los ACKs de Networking, Storage y Docs queden trazables.
- Conectar los empujes de documentacion con entregables SoraFS (SF-3, SF-6b, SF-9) que siguen en curso.

# Hilos de foco

## 1. Circular el snapshot de prioridades
- **Owners:** Program Management + Docs.
- **Accion:** Compartir este archivo en el canal de steering de Nexus, recolectar ✅ emojis de ACK de Networking TL, Storage TL y Docs/DevRel lead, y registrar screenshots/links en las notas de la reunion de governance.
- **Deadline:** 2025-03-04 12:00 UTC (48 h antes de la sesion de steering).
- **Evidencia:** Pegar el permalink del canal y la lista de ACKs en `docs/source/sorafs/council_minutes_2025-03-05.md` una vez cierre la sesion.

## 2. Cierre del kickoff Gateway/DNS
- **Owners:** Networking TL, Ops Automation lead.
- **Accion:** Usar la nueva Seccion 6 “Session facilitation & evidence hand-off” en `docs/source/sorafs_gateway_dns_design_runbook.md` para ejecutar el dry-run, documentar ownership de la plantilla de actas y prellenar el manifiesto de artefactos antes del workshop 2025-03-03.
- **Dependencias:** Lista de asistentes actualizada + snapshot de telemetria GAR (ver archivos `docs/source/sorafs_gateway_dns_design_*`).

## 3. Migracion de runbook de operador
- **Owners:** Docs/DevRel.
- **Accion:** Publicar el `Runbook Index` consolidado en el portal de docs (`docs/portal/docs/sorafs/runbooks-index.md`) para que reviewers tengan un ancla unica de navegacion; marcar la fila de migracion en `roadmap.md` completa una vez el indice este live y conectado al sidebar.
- **Follow-up:** ✅ Completed — la ola DocOps ahora anuncia el host beta preview en `https://docs.iroha.tech/` dentro del indice del portal para que reviewers accedan al snapshot con checksum gateado una vez cierre el onboarding.【docs/portal/docs/sorafs/runbooks-index.md:1】

## 4. Hilos de entrega SoraFS

| Item | Alcance | Ultima accion | Proximo blocker |
|------|--------|---------------|-----------------|
| **SF-3 — `sorafs-node`** | Conectar el plumbing de ingesta PoR a `PorCoordinatorRuntime` y exponer la superficie de API de ingestion de proofs de almacenamiento. | Se agrego la seccion “Remaining integration tasks” a `docs/source/sorafs/sorafs_node_plan.md`. | Implementar el worker de ingesta PoR + endpoint de estado Norito, luego actualizar el checklist del roadmap. |
| **SF-6b — pulido CLI/SDK** | Alinear los bindings del orquestador en Rust/JS/Swift, incluyendo retries/errores expuestos en help de CLI + definiciones TypeScript. | Checklist de pulido de bindings aterrizo en `docs/source/sorafs_orchestrator_plan.md`. | Seguir PRs downstream de SDK y registrar estado de paridad en el checklist del roadmap. |
| **SF-9 — integracion runtime de PoR coordinator** | Hilar `PorCoordinatorRuntime` en el loop de runtime de Torii, publicar el plan de wiring runtime y documentar eventos Norito para GovernanceLog. | `docs/source/sorafs_por_plan.md` ahora incluye una seccion “Operational integration” cubriendo hooks de runtime, storage y alertas. | Implementar el wiring runtime y actualizar el checklist SF-9 del roadmap (quedan dos boxes). |

# Checklist de distribucion
- [x] Publicar permalink del snapshot en `#nexus-steering` con bullets de resumen por hilo. *(Helper copy/paste: `docs/examples/nexus_steering_snapshot_post_2025-03.md`; ACKs aun pendientes.)*
- [ ] Capturar ACKs de Networking, Storage, Docs leads y actualizar `roadmap.md` (Near-Term Execution).
- [ ] Espejar este archivo en el portal de docs tras sign-off (tracked bajo ticket DocOps `PORTAL-218`).

## Log de distribucion

| Reviewer | Rol | Estado | Notas |
|----------|-----|--------|-------|
| @networking-tl | Networking TL | ⏳ Pendiente | Esperando ✅ acknowledgment en el hilo `#nexus-steering` (link para pegar en council minutes). |
| @storage-tl | Storage TL | ⏳ Pendiente | Requiere confirmacion de que items SF-3/SF-9 estan reflejados en el sprint board; agregar permalink al llegar el ACK. |
| @docs-devrel | Docs/DevRel Lead | ⏳ Pendiente | Debe confirmar plan de indice de runbooks + exposicion de preview; agregar screenshot/permalink en `docs/source/sorafs/council_minutes_2025-03-05.md`. |

> Recordatorio: capturar evidencia final de ACK (permalinks, screenshots) dentro
del archivo de council minutes antes de la sesion de governance de marzo.
