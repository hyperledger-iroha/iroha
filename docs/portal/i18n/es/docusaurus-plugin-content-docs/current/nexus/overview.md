---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-overview
title: Resumen de Sora Nexus
description: Resumen de alto nivel de la arquitectura de Iroha 3 (Sora Nexus) con enlaces a los documentos canonicos del mono-repo.
---

Nexus (Iroha 3) amplia Iroha 2 con ejecucion multi-lane, espacios de datos acotados por gobernanza y herramientas compartidas en cada SDK. Esta pagina refleja el nuevo resumen `docs/source/nexus_overview.md` del mono-repo para que los lectores del portal entiendan rapidamente como encajan las piezas de la arquitectura.

## Lineas de lanzamiento

- **Iroha 2** - despliegues autoalojados para consorcios o redes privadas.
- **Iroha 3 / Sora Nexus** - la red publica multi-lane donde los operadores registran espacios de datos (DS) e heredan herramientas compartidas de gobernanza, liquidacion y observabilidad.
- Ambas lineas compilan desde el mismo workspace (IVM + toolchain de Kotodama), por lo que las correcciones de SDK, las actualizaciones de ABI y los fixtures Norito siguen siendo portables. Los operadores descargan el paquete `iroha3-<version>-<os>.tar.zst` para unirse a Nexus; consulta `docs/source/sora_nexus_operator_onboarding.md` para la lista de verificacion en pantalla completa.

## Bloques de construccion

| Componente | Resumen | Ganchos del portal |
|-----------|---------|--------------|
| Espacio de datos (DS) | Dominio de ejecucion/almacenamiento definido por gobernanza que posee una o mas lanes, declara conjuntos de validadores, clase de privacidad y politica de tarifas + DA. | Consulta [Nexus spec](./nexus-spec) para el esquema del manifiesto. |
| Lane | Fragmento determinista de ejecucion; emite compromisos que ordena el anillo global NPoS. Las clases de lane incluyen `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | El [modelo de lane](./nexus-lane-model) captura geometria, prefijos de almacenamiento y retencion. |
| Plan de transicion | Identificadores placeholder, fases de enrutamiento y empaquetado de doble perfil siguen como los despliegues de un solo lane evolucionan hacia Nexus. | Las [notas de transicion](./nexus-transition-notes) documentan cada fase de migracion. |
| Directorio de espacios | Contrato de registro que almacena manifiestos + versiones de DS. Los operadores concilian las entradas del catalogo contra este directorio antes de unirse. | El rastreador de diffs de manifiestos vive en `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogo de lanes | Seccion `[nexus]` de configuracion que asigna IDs de lane a alias, politicas de enrutamiento y umbrales DA. `irohad --sora --config ... --trace-config` imprime el catalogo resuelto para auditorias. | Usa `docs/source/sora_nexus_operator_onboarding.md` para el recorrido de CLI. |
| Router de liquidacion | Orquestador de transferencias XOR que conecta lanes CBDC privadas con lanes de liquidez publicas. | `docs/source/cbdc_lane_playbook.md` detalla los knobs de politica y compuertas de telemetria. |
| Telemetria/SLOs | Paneles + alertas bajo `dashboards/grafana/nexus_*.json` capturan altura de lanes, backlog DA, latencia de liquidacion y profundidad de la cola de gobernanza. | El [plan de remediacion de telemetria](./nexus-telemetry-remediation) detalla los paneles, alertas y evidencia de auditoria. |

## Instantanea de despliegue

| Fase | Enfoque | Criterios de salida |
|-------|-------|---------------|
| N0 - Beta cerrada | Registrar gestionado por el consejo (`.sora`), incorporacion manual de operadores, catalogo de lanes estatico. | Manifiestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lanzamiento publico | Anade sufijos `.nexus`, subastas, registrar de autoservicio, cableado de liquidacion XOR. | Pruebas de sincronizacion de resolvers/gateways, paneles de reconciliacion de facturacion, simulacros de disputas. |
| N2 - Expansion | Introduce `.dao`, APIs de reventa, analitica, portal de disputas, scorecards de stewards. | Artefactos de cumplimiento versionados, toolkit de jurado de politicas en linea, informes de transparencia del tesoro. |
| Puerta NX-12/13/14 | El motor de cumplimiento, paneles de telemetria y documentacion deben salir juntos antes de pilotos con socios. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) publicados, paneles conectados, motor de politicas fusionado. |

## Responsabilidades del operador

1. **Higiene de configuracion** - manten `config/config.toml` sincronizado con el catalogo publicado de lanes y dataspaces; archiva la salida de `--trace-config` con cada ticket de release.
2. **Seguimiento de manifiestos** - concilia las entradas del catalogo con el paquete mas reciente del Space Directory antes de unirte o actualizar nodos.
3. **Cobertura de telemetria** - expon los paneles `nexus_lanes.json`, `nexus_settlement.json` y los dashboards relacionados del SDK; conecta alertas a PagerDuty y ejecuta revisiones trimestrales segun el plan de remediacion de telemetria.
4. **Reporte de incidentes** - sigue la matriz de severidad en [Nexus operations](./nexus-operations) y presenta RCAs dentro de cinco dias habiles.
5. **Preparacion de gobernanza** - asiste a las votaciones del consejo Nexus que impactan tus lanes y ensaya instrucciones de rollback trimestralmente (seguido en `docs/source/project_tracker/nexus_config_deltas/`).

## Ver tambien

- Resumen canonico: `docs/source/nexus_overview.md`
- Especificacion detallada: [./nexus-spec](./nexus-spec)
- Geometria de lanes: [./nexus-lane-model](./nexus-lane-model)
- Plan de transicion: [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediacion de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operaciones: [./nexus-operations](./nexus-operations)
- Guia de onboarding de operadores: `docs/source/sora_nexus_operator_onboarding.md`
