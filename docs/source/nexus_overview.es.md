<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: es
direction: ltr
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-11-08T16:26:32.878050+00:00"
translation_last_reviewed: 2026-01-01
---

# Resumen de Nexus y contexto operativo

**Enlace del roadmap:** NX-14 - documentacion de Nexus y runbooks de operaciones  
**Estado:** Borrador 2026-03-24 (se empareja con `docs/source/nexus_operations.md`)  
**Audiencia:** responsables de programa, ingenieros de operaciones y equipos asociados que
necesitan un resumen de una pagina de la arquitectura de Sora Nexus (Iroha 3) antes
de profundizar en las especificaciones detalladas (`docs/source/nexus.md`,
`docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. Lineas de release y tooling compartido

- **Iroha 2** sigue siendo la via autoalojada para despliegues de consorcio.
- **Iroha 3 / Sora Nexus** introduce ejecucion multi-lane, data spaces y
  gobernanza compartida. El mismo repositorio, toolchain y pipelines de CI construyen ambas
  lineas de release, por lo que los arreglos al IVM, al compilador Kotodama o a los SDKs
  se aplican automaticamente a Nexus.
- **Artefactos:** `iroha3-<version>-<os>.tar.zst` bundles y OCI images contienen
  binarios, configs de ejemplo y metadatos del perfil Nexus. Los operadores consultan
  `docs/source/sora_nexus_operator_onboarding.md` para el flujo de validacion de artefactos
  de punta a punta.
- **Interfaz SDK compartida:** Los SDKs de Rust, Python, JS/TS, Swift y Android consumen
  los mismos esquemas Norito y fixtures de direccion (`fixtures/account/address_vectors.json`)
  para que billeteras y automatizacion puedan cambiar entre redes Iroha 2 y Nexus sin
  bifurcar formatos.

## 2. Bloques de construccion arquitectonica

| Componente | Descripcion | Referencias clave |
|-----------|-------------|-------------------|
| **Data Space (DS)** | Dominio de ejecucion con alcance de gobernanza que define membresia de validadores, clase de privacidad, politica de tarifas y perfil de disponibilidad de datos. Cada DS posee una o mas lanes. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | Shard determinista de ejecucion y estado. Los manifiestos de lane declaran conjuntos de validadores, ganchos de settlement, metadatos de telemetria y permisos de ruteo. El anillo de consenso global ordena los commitments de lane. | `docs/source/nexus_lanes.md` |
| **Space Directory** | Contrato de registro (y helpers CLI) que almacena manifiestos de DS, rotaciones de validadores y grants de capacidades. Mantiene manifiestos historicos firmados para que los auditores reconstruyan el estado. | `docs/source/nexus.md#space-directory` |
| **Catalogo de lanes** | Seccion de configuracion (`[nexus]` en `config.toml`) que mapea IDs de lane a alias, politicas de ruteo y knobs de retencion. Los operadores pueden inspeccionar el catalogo efectivo via `irohad --sora --config ... --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | Enruta movimientos XOR entre lanes (por ejemplo, lanes CBDC privadas <-> lanes de liquidez publicas). Las politicas por defecto viven en `docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **Telemetria y SLOs** | Dashboards y reglas de alerta bajo `dashboards/grafana/nexus_*.json` capturan altura de lanes, backlog de DA, latencia de settlement y profundidad de cola de gobernanza. El plan de remediacion se sigue en `docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### Clases de lane y data space

- `default_public` lanes anclan cargas totalmente publicas bajo el Parlamento Sora.
- `public_custom` lanes permiten economias especificas por programa mientras siguen transparentes.
- `private_permissioned` lanes sirven CBDCs o apps de consorcio; exportan solo commitments y proofs.
- `hybrid_confidential` lanes combinan pruebas de conocimiento cero con ganchos de divulgacion selectiva.

Cada lane declara:

1. **Manifiesto de lane:** metadatos aprobados por gobernanza y rastreados en el Space Directory.
2. **Politica de disponibilidad de datos:** parametros de erasure coding, ganchos de recuperacion y requisitos de auditoria.
3. **Perfil de telemetria:** dashboards y runbooks on-call que deben actualizarse cada vez que la gobernanza cambia un lane.

## 3. Resumen del cronograma de despliegue

| Fase | Enfoque | Criterios de salida |
|------|---------|---------------------|
| **N0 - Closed beta** | Registrador gestionado por el consejo, solo namespace `.sora`, onboarding de operadores manual. | Manifiestos de DS firmados, catalogo de lanes estatico, ensayos de gobernanza registrados. |
| **N1 - Public launch** | Agrega sufijos `.nexus`, subastas y registrador de autoservicio. Los settlements se conectan a la tesoreria XOR. | Tests de sincronizacion de resolver/gateway en verde, dashboards de conciliacion de facturacion en vivo, ejercicio de disputa completo. |
| **N2 - Expansion** | Habilita `.dao`, APIs de reseller, analitica, portal de disputas, scorecards de stewards. | Artefactos de cumplimiento versionados, toolkit de policy-jury en vivo, reportes de transparencia de tesoreria publicados. |
| **NX-12/13/14 gate** | El motor de cumplimiento, dashboards de telemetria y documentacion deben aterrizar juntos antes de abrir el piloto de partners. | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` publicados, dashboards cableados con alertas, motor de politica conectado a gobernanza. |

## 4. Responsabilidades del operador

| Responsabilidad | Descripcion | Evidencia |
|----------------|-------------|----------|
| Higiene de config | Mantener `config/config.toml` sincronizado con el catalogo publicado de lanes y dataspaces; registrar cambios en tickets. | Salida de `irohad --sora --config ... --trace-config` archivada con artefactos de release. |
| Seguimiento de manifiestos | Vigilar actualizaciones del Space Directory y refrescar caches/allowlists locales. | Bundle de manifiestos firmado almacenado con el ticket de onboarding. |
| Cobertura de telemetria | Asegurar que los dashboards listados en la Seccion 2 sean accesibles, alertas conectadas a PagerDuty y revisiones trimestrales registradas. | Minutas de guardia + export de Alertmanager. |
| Reporte de incidentes | Seguir la matriz de severidad definida en `docs/source/nexus_operations.md` y presentar reportes post-incidente en cinco dias habiles. | Plantilla post-incidente archivada por ID de incidente. |
| Preparacion de gobernanza | Participar en votos del consejo Nexus cuando cambios de politica de lanes afecten el despliegue; ensayar instrucciones de rollback trimestralmente. | Asistencia del consejo + checklist de ensayo en `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. Mapa de documentacion relacionada

- **Especificacion a fondo:** `docs/source/nexus.md`
- **Geometria de lanes y almacenamiento:** `docs/source/nexus_lanes.md`
- **Plan de transicion y ruteo temporal:** `docs/source/nexus_transition_notes.md`
- **Onboarding para operadores:** `docs/source/sora_nexus_operator_onboarding.md`
- **Politica de lanes CBDC y plan de settlement:** `docs/source/cbdc_lane_playbook.md`
- **Remediacion de telemetria y mapa de dashboards:** `docs/source/nexus_telemetry_remediation_plan.md`
- **Runbook / proceso de incidentes:** `docs/source/nexus_operations.md`

Mantener este resumen alineado con el item de roadmap NX-14 cuando haya cambios sustanciales
en los documentos enlazados o cuando se introduzcan nuevas clases de lanes o flujos de gobernanza.
