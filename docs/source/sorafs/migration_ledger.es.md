---
lang: es
direction: ltr
source: docs/source/sorafs/migration_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 335591734454cabeb39826a2cb6ddf778e5bd3e488641d2e90e5bb6079bc75df
source_last_modified: "2025-11-02T04:40:40.138255+00:00"
translation_last_reviewed: "2026-01-30"
---

# Ledger de migracion SoraFS

Este ledger espeja el change log de migracion capturado en el RFC de
Arquitectura SoraFS. Las entradas se agrupan por hito y listan la ventana
efectiva, equipos impactados y acciones requeridas. Las actualizaciones al plan
de migracion DEBEN modificar tanto este archivo como el RFC
(`docs/source/sorafs_architecture_rfc.md`) para mantener alineados a los
consumidores downstream.

| Hito | Ventana efectiva | Resumen de cambios | Equipos impactados | Acciones | Estado |
|------|------------------|-------------------|--------------------|----------|--------|
| M1 | Semanas 7-12 | CI impone fixtures deterministas; pruebas de alias disponibles en staging; el tooling expone flags de expectativa explicita. | Docs, Storage, Governance | Asegurar que los fixtures sigan firmados, registrar aliases en el registro staging, actualizar checklists de release con enforcement `--car-digest/--root-cid`. | ⏳ Pendiente |

Las minutas del control plane de governance que referencian estos hitos se
almacenan bajo `docs/source/sorafs/`. Los equipos deben agregar bullets con
fecha bajo cada fila cuando ocurran eventos notables (p. ej., nuevos registros
de alias, retrospectivas de incidentes del registro) para proveer un rastro
auditado.

## Actualizaciones recientes

- 2025-11-01 — Se circulo `migration_roadmap.md` al council de governance y a
  listas de operadores para revision; se espera sign-off en la proxima sesion
  del council (ref: follow-up `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — El ISI de registro de Pin Registry ahora aplica validacion
  compartida de chunker/politica via helpers `sorafs_manifest`, manteniendo
  alineados los caminos on-chain con los checks de Torii.
- 2026-02-13 — Se agregaron fases de rollout de provider advert (R0–R3) al
  ledger y se publicaron los dashboards y guia de operadores asociados
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
