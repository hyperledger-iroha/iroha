---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_design_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7524a4c4d40b54eb27376abafaaf2f5deedf1c7e670fbe695d729fe61b3ad41f
source_last_modified: "2025-11-02T17:18:17.688536+00:00"
translation_last_reviewed: "2026-01-30"
---

# Arranque de diseño de SoraFS Gateway y DNS — Agenda

**Fecha:** 2025-03-03  
**Hora:** 16:00–17:00 UTC (60 minutos)  
**Facilitador:** Networking TL  
**Tipo de reunión:** taller de decisiones (Zoom/Meet + doc de notas compartidas)

## 1. Lista de asistentes

| Rol | Nombre / alias | Responsabilidades |
|------|---------------|------------------|
| Networking TL (facilitador) | `networking.tl@soranet` | Conducir discusión enfocada en resultados, capturar decisiones, asumir seguimientos. |
| Ops Lead | `ops.lead@soranet` | Automatización DNS, runbooks de despliegue, preparación operativa. |
| Storage Team Rep | `storage.rep@sorafs` | Integración de manifiestos, fixtures de chunker, impactos en orquestación de clientes. |
| Tooling WG Rep | `tooling.wg@sorafs` | Mantenimiento del harness de conformidad, cambios de CLI/tooling. |
| Governance Liaison | `governance@sora` | Alineación de política GAR, rutas de escalamiento, archivo de artefactos. |
| QA Guild Lead | `qa.guild@sorafs` | Plan de cobertura de tests, recursos de suites de carga, ownership de regresiones. |
| Docs/DevRel Observer | `docs.devrel@sora` | Runbooks de operadores, actualizaciones Docusaurus, comunicación pública. |
| Torii Platform Rep | `torii.platform@soranet` | Integración de API, pipelines de telemetría, superficies de configuración. |
| Security Engineering Observer | `security@soranet` | Modelado de amenazas de enforcement GAR, requisitos de trazabilidad de auditoría. |

> **Acción:** Confirmar disponibilidad con cada asistente antes del 2025-02-26; sustituir roles si surgen conflictos.

## 2. Revisión del pre-read (0–5 min)
- Confirmación rápida de que todos leyeron:
  - `docs/source/sorafs_gateway_dns_design_pre_read.md`
  - `docs/source/sorafs_gateway_profile.md`
  - `docs/source/sorafs_gateway_conformance.md`
  - `docs/source/sorafs_gateway_deployment_handbook.md`
- Destacar cualquier documento nuevo añadido desde la circulación.

## 3. Alcance DNS determinista (5–20 min)
1. **Reglas de derivación de hosts (5 min):**
   - Esquema canónico propuesto (`<capability>.<lane>.gateway.sora`).
   - Ownership de reservas de namespace y manejo de colisiones.
2. **Pruebas de alias y política TTL (5 min):**
   - Revisar requisitos SF-4a, flujo de invalidación de pruebas en caché.
3. **Ruta de automatización (5 min):**
   - Elección de toolchain: Terraform + RFC2136 vs Torii-managed.
   - Gestión de secretos, logging de auditoría, enlace con GAR.
4. **Captura de decisiones (5 min):**
   - Registrar decisiones finales en notas compartidas.
   - Asignar owner para codificar decisiones en docs/config.

## 4. Enforcement del gateway y runtime (20–40 min)
1. **Motor de política GAR (8 min):**
   - Enfoque de integración (librería vs caché Norito).
   - Knobs de configuración, toggles de despliegue.
2. **Alineación de perfil trustless (5 min):**
   - Confirmar ítems pendientes de `sorafs_gateway_profile.md`.
3. **Modo directo y rate limiting (4 min):**
   - Requisitos para enforcement de capacidades de manifiesto, ganchos de denylist.
4. **Telemetría y alertas (8 min):**
   - Métricas a capturar (`torii_sorafs_gar_violations_total`, histogramas de latencia).
   - Enrutamiento de alertas a governance/on-call.
5. **Captura de decisiones (5 min):**
   - Documentar criterios de aceptación y owner de PRs de implementación.

## 5. Plan del harness de conformidad (40–50 min)
1. **Revisión de cobertura (5 min):**
   - Suites de replay, negativas y de carga según `sorafs_gateway_conformance.md`.
2. **Pipeline de atestación (3 min):**
   - Formato de sobre Norito, responsabilidades de `sorafs-gateway-cert`.
3. **Chequeo de recursos y timeline (2 min):**
   - Staffing de QA Guild, timelines de Tooling WG, integración CI.

## 6. Dependencias y alineación con roadmap (50–55 min)
- Mapear decisiones a ítems del roadmap (SF-4, SF-4a, SF-5, SF-5a).
- Identificar bloqueos provenientes de entregables SF-2/SF-3.
- Confirmar actualizaciones necesarias en `status.md` y el portal Docusaurus.

## 7. Registro de acciones y próximos pasos (55–60 min)
- Resumir decisiones y acciones pendientes.
- Asignar owners, fechas límite y checkpoints de seguimiento.
- Confirmar plan de publicación para notas de reunión y artefactos.

## 8. Checklist de pretrabajo (owners)

| Tarea | Owner | Fecha límite | Notas |
|-------|-------|--------------|-------|
| Confirmar disponibilidad de asistentes | Networking TL | 2025-02-26 | Usar plantilla en `docs/source/sorafs_gateway_dns_design_invite.txt`. |
| Circular agenda y pre-read | Networking TL | 2025-02-27 | Enviar agenda + pre-read en un solo email. |
| Recopilar snapshot actual de métricas GAR | Ops Lead / Torii Rep | 2025-02-28 | Ejecutar variante de `scripts/telemetry/run_schema_diff.sh` (ver acción). |
| Preparar diagramas (flujo DNS, motor de política) | Tooling WG / Security | 2025-03-01 | Guardar en `docs/source/images/sorafs_gateway_kickoff/`. |
| Borrador de doc de notas | Docs/DevRel | 2025-02-28 | Proveer outline de decisiones/acciones. |

## 9. Plantilla de seguimiento post-reunión
- **Dentro de 24h:** Publicar notas + registro de acciones en roadmap/status.
- **Dentro de 48h:** Actualizar todos los docs referenciados con decisiones acordadas.
- **Dentro de 72h:** Abrir issues/PRs de implementación y agendar checkpoints de progreso.

---

Para ajustes o puntos adicionales de agenda, comentar directamente en este documento o escribir a `networking.tl@soranet`. Todos los cambios deben congelarse 24 horas antes de la sesión para mantener el foco en decisiones.
