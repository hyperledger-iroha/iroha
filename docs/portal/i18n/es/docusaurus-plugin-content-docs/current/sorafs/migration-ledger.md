<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> Adaptado de [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Libro mayor de migración de SoraFS

Este ledger refleja el registro de cambios de migración capturado en el RFC de
Arquitectura de SoraFS. Las entradas se agrupan por hito e indican la ventana
vigente, los equipos impactados y las acciones requeridas. Las actualizaciones
al plan de migración DEBEN modificar esta página y el RFC
(`docs/source/sorafs_architecture_rfc.md`) para mantener alineados a los
consumidores aguas abajo.

| Hito | Ventana efectiva | Resumen del cambio | Equipos impactados | Acciones | Estado |
|------|------------------|-------------------|--------------------|----------|--------|
| M0 | Semanas 1–6 | Se publicaron fixtures del chunker; los pipelines emiten bundles CAR + manifest junto a artefactos legados; se crearon entradas del ledger de migración. | Docs, DevRel, SDKs | Adoptar `sorafs_manifest_stub` con flags de expectativas, registrar entradas en este ledger, mantener el CDN legado. | ✅ Activo |
| M1 | Semanas 7–12 | El CI impone fixtures deterministas; las pruebas de alias están disponibles en staging; el tooling expone flags de expectativa explícitas. | Docs, Storage, Governance | Asegurar que los fixtures sigan firmados, registrar aliases en el registry de staging, actualizar las checklists de release con la exigencia `--car-digest/--root-cid`. | ⏳ Pendiente |
| M2 | Semanas 13–20 | El pinning respaldado por registry se vuelve la ruta principal; los artefactos legados pasan a solo lectura; los gateways priorizan pruebas de registry. | Storage, Ops, Governance | Enrutar el pinning por el registry, congelar los hosts legados, publicar avisos de migración para operadores. | ⏳ Pendiente |
| M3 | Semana 21+ | Se impone acceso solo por alias; la observabilidad alerta sobre la paridad de registry; el CDN legado se desmantela. | Ops, Networking, SDKs | Eliminar el DNS legado, rotar URLs en caché, monitorear dashboards de paridad, actualizar defaults del SDK. | ⏳ Pendiente |
| R0–R3 | 2025-03-31 → 2025-07-01 | Fases de cumplimiento de provider advert: R0 observar, R1 advertir, R2 aplicar handles/capacidades canónicas, R3 purgar payloads legados. | Observability, Ops, SDKs, DevRel | Importar `grafana_sorafs_admission.json`, seguir la checklist de operadores en `provider_advert_rollout.md`, preparar renovaciones de advert 30+ días antes del gate R2. | ⏳ Pendiente |

Las minutas del plano de control de gobernanza que referencian estos hitos viven en
`docs/source/sorafs/`. Los equipos deben añadir viñetas con fecha debajo de cada fila
cuando ocurran eventos relevantes (p. ej., nuevos registros de alias, retrospectivas de
incidentes del registry) para ofrecer una trazabilidad auditable.

## Actualizaciones recientes

- 2025-11-01 — Se distribuyó `migration_roadmap.md` al consejo de gobernanza y a las
  listas de operadores para revisión; se espera la aprobación en la próxima sesión
  del consejo (ref: seguimiento en `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — El ISI de registro del Pin Registry ahora aplica la validación compartida
  de chunker/política vía helpers de `sorafs_manifest`, manteniendo alineadas las rutas
  on-chain con las comprobaciones de Torii.
- 2026-02-13 — Se añadieron las fases de rollout de provider advert (R0–R3) al ledger y
  se publicaron los dashboards y la guía de operadores asociada
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
