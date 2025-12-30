<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adaptado de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Hoja de ruta de migración de SoraFS (SF-1)

Este documento operacionaliza la guía de migración capturada en
`docs/source/sorafs_architecture_rfc.md`. Amplía los entregables de SF-1 en
hitos listos para ejecución, criterios de compuerta y checklists de responsables
para que los equipos de storage, gobernanza, DevRel y SDK coordinen la transición
desde el hosting de artefactos legados hacia publicación respaldada por SoraFS.

La hoja de ruta es intencionalmente determinista: cada hito nombra los artefactos
requeridos, las invocaciones de comandos y los pasos de atestación para que los
pipelines downstream produzcan salidas idénticas y la gobernanza mantenga una
traza auditable.

## Resumen de hitos

| Hito | Ventana | Objetivos principales | Debe entregar | Responsables |
|------|---------|-----------------------|---------------|--------------|
| **M0 - Bootstrap** | Semanas 1-6 | Publicar fixtures deterministas del chunker y publicar artefactos en doble vía (legado + SoraFS). | Fixtures de `sorafs_chunker`, integración CLI de `sorafs_manifest_stub`, entradas del ledger de migración. | Docs, DevRel, Storage |
| **M1 - Enforcement determinista** | Semanas 7-12 | Exigir fixtures firmados y preparar pruebas de alias mientras los pipelines adoptan flags de expectativa. | Verificación nocturna de fixtures, manifiestos firmados por el consejo, entradas de staging en el registro de aliases. | Storage, Governance, SDKs |
| **M2 - Registry primero** | Semanas 13-20 | Enrutar pins a través del registry, congelar bundles legados y exponer telemetría de paridad. | Contrato del Pin Registry + CLI (`sorafs pin propose/approve`), dashboards de observabilidad, runbooks de operadores. | Governance, Ops, Observability |
| **M3 - Solo alias** | Semana 21+ | Desmantelar el hosting legado y exigir pruebas de alias para la recuperación. | Gateways solo-alias, alertas de paridad, defaults del SDK actualizados, aviso de retirada legacy. | Ops, Networking, SDKs |

El estado de los hitos se rastrea en `docs/source/sorafs/migration_ledger.md`. Todos
los cambios a esta hoja de ruta DEBEN actualizar el ledger para mantener la
sincronía entre gobernanza e ingeniería de releases.

## Líneas de trabajo

### 1. Reempaque de datos legados

| Paso | Hito | Descripción | Responsable(s) | Salida |
|------|------|-------------|----------------|--------|
| Inventario y etiquetado | M0 | Exportar digests SHA3-256 de bundles legados y registrarlos en el ledger de migración (append-only). | Docs, DevRel | Entradas del ledger con `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid`. |
| Reconstrucción determinista | M0-M1 | Invocar `sorafs_manifest_stub` para cada artefacto de release y persistir CAR, manifest, envelope de firmas y plan de fetch en `artifacts/<team>/<alias>/<timestamp>/`. | Docs, CI | Bundles reproducibles de CAR + manifest por release. |
| Bucle de validación | M1 | Reproducir `sorafs_fetch` contra gateways de staging para confirmar que los límites/digests de chunk coinciden con fixtures. Registrar pass/fail en comentarios del ledger. | Governance QA | Informe de verificación de staging + issue de GitHub por drift. |
| Cut-over de registry | M2 | Cambiar el estado del ledger a `Pinned` cuando el digest del manifest exista on-chain; el bundle legado pasa a solo lectura (servir pero no modificar). | Governance, Ops | Hash de transacción en registry, ticket de solo lectura para storage legado. |
| Decommission | M3 | Eliminar entradas del CDN legado tras un periodo de gracia de 30 días, archivar aprobaciones de cambios DNS y publicar post-mortem. | Ops | Checklist de decommission, registro de cambios DNS, cierre de ticket de incidente. |

### 2. Adopción de pinning determinista

| Paso | Hito | Descripción | Responsable(s) | Salida |
|------|------|-------------|----------------|--------|
| Ensayos de fixtures | M0 | Dry-runs semanales comparando digests locales de chunk contra `fixtures/sorafs_chunker`. Publicar informe bajo `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` con matriz de pass/fail. |
| Exigir firmas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` fallan si firmas o manifests derivan. Overrides de desarrollo requieren waiver de gobernanza adjunto al PR. | Tooling WG | Log de CI, enlace a ticket de waiver (si aplica). |
| Flags de expectativa | M1 | Los pipelines llaman `sorafs_manifest_stub` con expectativas explícitas para fijar salidas: | Docs CI | Scripts actualizados referenciando flags de expectativa (ver bloque de comando abajo). |
| Pinning registry-first | M2 | `sorafs pin propose` y `sorafs pin approve` envuelven los envíos de manifest; el CLI por defecto usa `--require-registry`. | Governance Ops | Log de auditoría del CLI de registry, telemetría de propuestas fallidas. |
| Paridad de observabilidad | M3 | Dashboards Prometheus/Grafana alertan cuando los inventarios de chunks difieren de los manifests del registry; alertas cableadas al on-call de ops. | Observability | Enlace de dashboard, IDs de reglas de alerta, resultados de GameDay. |

#### Comando canónico de publicación

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Reemplaza los valores de digest, tamaño y CID con las referencias esperadas
registradas en la entrada del ledger de migración para el artefacto.

### 3. Transición de alias y comunicaciones

| Paso | Hito | Descripción | Responsable(s) | Salida |
|------|------|-------------|----------------|--------|
| Pruebas de alias en staging | M1 | Registrar claims de alias en el Pin Registry de staging y adjuntar pruebas Merkle a los manifests (`--alias`). | Governance, Docs | Bundle de pruebas almacenado junto al manifest + comentario del ledger con el nombre del alias. |
| DNS dual + notificación | M1-M2 | Operar DNS legado y Torii/SoraDNS en paralelo; publicar avisos de migración a operadores y canales de SDK. | Networking, DevRel | Post de anuncio + ticket de cambio DNS. |
| Enforcement de pruebas | M2 | Gateways rechazan manifests sin headers `Sora-Proof` recientes; CI añade un paso `sorafs alias verify` para obtener pruebas. | Networking | Parche de configuración de gateway + salida de CI con verificación exitosa. |
| Rollout solo alias | M3 | Eliminar DNS legado, actualizar defaults del SDK para usar Torii/SoraDNS + pruebas de alias, documentar ventana de rollback. | SDK Maintainers, Ops | Notas de release del SDK, actualización de runbook de ops, plan de rollback. |

### 4. Comunicación y auditoría

- **Disciplina del ledger:** cada cambio de estado (deriva de fixtures, envío al registry,
  activación de alias) debe añadir una nota fechada en
  `docs/source/sorafs/migration_ledger.md`.
- **Minutas de gobernanza:** las sesiones del consejo que aprueben cambios del pin registry o
  políticas de alias deben referenciar esta hoja de ruta y el ledger.
- **Comunicaciones externas:** DevRel publica actualizaciones de estado en cada hito (blog +
  extracto de changelog) destacando garantías deterministas y calendarios de alias.

## Dependencias y riesgos

| Dependencia | Impacto | Mitigación |
|-------------|---------|------------|
| Disponibilidad del contrato Pin Registry | Bloquea el rollout M2 pin-first. | Preparar el contrato antes de M2 con pruebas de replay; mantener fallback de envelope hasta que no haya regresiones. |
| Claves de firma del consejo | Requeridas para envelopes de manifest y aprobaciones del registry. | Ceremonia de firmas documentada en `docs/source/sorafs/signing_ceremony.md`; rotar claves con superposición y nota en el ledger. |
| Tooling de paridad de gateway | Necesario para exigir pruebas de alias y paridad de chunks. | Enviar actualizaciones del gateway en M1, mantener comportamiento legacy detrás de feature flag hasta cumplir criterios M2. |
| Cadencia de releases de SDK | Los clientes deben respetar pruebas de alias antes de M3. | Alinear ventanas de release de SDK con los gates de los hitos; añadir checklists de migración a las plantillas de release. |

Los riesgos residuales y mitigaciones se reflejan en `docs/source/sorafs_architecture_rfc.md`
y deben cruzarse cuando se hagan ajustes.

## Checklist de criterios de salida

| Hito | Criterios |
|------|-----------|
| M0 | - Todos los artefactos objetivo reconstruidos vía `sorafs_manifest_stub` con expectation flags. <br /> - Ledger de migración poblado para cada familia de artefactos. <br /> - Publicación dual (legado + SoraFS) activa. |
| M1 | - Job nocturno de fixtures en verde durante siete días consecutivos. <br /> - Pruebas de alias en staging verificadas en CI. <br /> - Gobernanza ratifica la política de expectation flags. |
| M2 | - 100% de los nuevos manifests enroutados a través del Pin Registry. <br /> - Storage legado marcado como solo lectura; playbook de incidentes aprobado. <br /> - Dashboards de observabilidad en línea con umbrales de alerta. |
| M3 | - Gateways solo alias en producción. <br /> - DNS legado eliminado y reflejado en tickets de cambio. <br /> - Defaults de SDK actualizados y releaseados. <br /> - Estado final añadido al ledger de migración. |

## Gestión de cambios

1. Proponer ajustes mediante un PR que actualice este archivo **y**
   `docs/source/sorafs/migration_ledger.md`.
2. Enlazar minutas de gobernanza y evidencia de CI en la descripción del PR.
3. Tras el merge, notificar a la lista de correo de storage + DevRel con un resumen
   y acciones esperadas para operadores.

Seguir este procedimiento garantiza que el rollout de SoraFS se mantenga determinista,
auditable y transparente entre los equipos que participan en el lanzamiento de Nexus.
