<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd986adf52d15dfb82f4396cfa6891efd54c78f528d7621c355dd6d8624f0a02
source_last_modified: "2025-11-10T17:36:53.333906+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-overview
title: Resumen de Sora Nexus
description: Resumen de alto nivel de la arquitectura de Iroha 3 (Sora Nexus) con enlaces a los documentos canónicos del mono-repo.
---

Nexus (Iroha 3) amplía Iroha 2 con ejecución multi-lane, espacios de datos acotados por gobernanza y herramientas compartidas en cada SDK. Esta página refleja el nuevo resumen `docs/source/nexus_overview.md` del mono-repo para que los lectores del portal entiendan rápidamente cómo encajan las piezas de la arquitectura.

## Líneas de lanzamiento

- **Iroha 2** - despliegues autoalojados para consorcios o redes privadas.
- **Iroha 3 / Sora Nexus** - la red pública multi-lane donde los operadores registran espacios de datos (DS) e heredan herramientas compartidas de gobernanza, liquidación y observabilidad.
- Ambas líneas compilan desde el mismo workspace (IVM + toolchain de Kotodama), por lo que las correcciones de SDK, las actualizaciones de ABI y los fixtures Norito siguen siendo portables. Los operadores descargan el paquete `iroha3-<version>-<os>.tar.zst` para unirse a Nexus; consulta `docs/source/sora_nexus_operator_onboarding.md` para la lista de verificación en pantalla completa.

## Bloques de construcción

| Componente | Resumen | Ganchos del portal |
|-----------|---------|--------------|
| Espacio de datos (DS) | Dominio de ejecución/almacenamiento definido por gobernanza que posee una o más lanes, declara conjuntos de validadores, clase de privacidad y política de tarifas + DA. | Consulta [Nexus spec](./nexus-spec) para el esquema del manifiesto. |
| Lane | Fragmento determinista de ejecución; emite compromisos que ordena el anillo global NPoS. Las clases de lane incluyen `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | El [modelo de lane](./nexus-lane-model) captura geometría, prefijos de almacenamiento y retención. |
| Plan de transición | Identificadores placeholder, fases de enrutamiento y empaquetado de doble perfil siguen cómo los despliegues de un solo lane evolucionan hacia Nexus. | Las [notas de transición](./nexus-transition-notes) documentan cada fase de migración. |
| Directorio de espacios | Contrato de registro que almacena manifiestos + versiones de DS. Los operadores concilian las entradas del catálogo contra este directorio antes de unirse. | El rastreador de diffs de manifiestos vive en `docs/source/project_tracker/nexus_config_deltas/`. |
| Catálogo de lanes | Sección `[nexus]` de configuración que asigna IDs de lane a alias, políticas de enrutamiento y umbrales DA. `irohad --sora --config … --trace-config` imprime el catálogo resuelto para auditorías. | Usa `docs/source/sora_nexus_operator_onboarding.md` para el recorrido de CLI. |
| Router de liquidación | Orquestador de transferencias XOR que conecta lanes CBDC privadas con lanes de liquidez públicas. | `docs/source/cbdc_lane_playbook.md` detalla los knobs de política y compuertas de telemetría. |
| Telemetría/SLOs | Paneles + alertas bajo `dashboards/grafana/nexus_*.json` capturan altura de lanes, backlog DA, latencia de liquidación y profundidad de la cola de gobernanza. | El [plan de remediación de telemetría](./nexus-telemetry-remediation) detalla los paneles, alertas y evidencia de auditoría. |

## Instantánea de despliegue

| Fase | Enfoque | Criterios de salida |
|-------|-------|---------------|
| N0 - Beta cerrada | Registrar gestionado por el consejo (`.sora`), incorporación manual de operadores, catálogo de lanes estático. | Manifiestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lanzamiento público | Añade sufijos `.nexus`, subastas, registrar de autoservicio, cableado de liquidación XOR. | Pruebas de sincronización de resolvers/gateways, paneles de reconciliación de facturación, simulacros de disputas. |
| N2 - Expansión | Introduce `.dao`, APIs de reventa, analítica, portal de disputas, scorecards de stewards. | Artefactos de cumplimiento versionados, toolkit de jurado de políticas en línea, informes de transparencia del tesoro. |
| Puerta NX-12/13/14 | El motor de cumplimiento, paneles de telemetría y documentación deben salir juntos antes de pilotos con socios. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) publicados, paneles conectados, motor de políticas fusionado. |

## Responsabilidades del operador

1. **Higiene de configuración** - mantén `config/config.toml` sincronizado con el catálogo publicado de lanes y dataspaces; archiva la salida de `--trace-config` con cada ticket de release.
2. **Seguimiento de manifiestos** - concilia las entradas del catálogo con el paquete más reciente del Space Directory antes de unirte o actualizar nodos.
3. **Cobertura de telemetría** - expón los paneles `nexus_lanes.json`, `nexus_settlement.json` y los dashboards relacionados del SDK; conecta alertas a PagerDuty y ejecuta revisiones trimestrales según el plan de remediación de telemetría.
4. **Reporte de incidentes** - sigue la matriz de severidad en [Nexus operations](./nexus-operations) y presenta RCAs dentro de cinco días hábiles.
5. **Preparación de gobernanza** - asiste a las votaciones del consejo Nexus que impactan tus lanes y ensaya instrucciones de rollback trimestralmente (seguido en `docs/source/project_tracker/nexus_config_deltas/`).

## Ver también

- Resumen canónico: `docs/source/nexus_overview.md`
- Especificación detallada: [./nexus-spec](./nexus-spec)
- Geometría de lanes: [./nexus-lane-model](./nexus-lane-model)
- Plan de transición: [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediación de telemetría: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operaciones: [./nexus-operations](./nexus-operations)
- Guía de onboarding de operadores: `docs/source/sora_nexus_operator_onboarding.md`
