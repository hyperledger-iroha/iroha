---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: Resumen de Sora Nexus
descripción: Resumen de alto nivel de la arquitectura de Iroha 3 (Sora Nexus) con enlaces a los documentos canónicos del mono-repo.
---

Nexus (Iroha 3) amplia Iroha 2 con ejecucion multi-lane, espacios de datos acotados por gobernanza y herramientas compartidas en cada SDK. Esta página refleja el nuevo resumen `docs/source/nexus_overview.md` del mono-repo para que los lectores del portal entiendan rápidamente como encajan las piezas de la arquitectura.

## Líneas de lanzamiento

- **Iroha 2** - desplegables autoalojados para consorcios o redes privadas.
- **Iroha 3 / Sora Nexus** - la red publica multi-lane donde los operadores registran espacios de datos (DS) y heredan herramientas compartidas de gobernanza, liquidación y observabilidad.
- Ambas líneas compilan desde el mismo espacio de trabajo (IVM + toolchain de Kotodama), por lo que las correcciones de SDK, las actualizaciones de ABI y los dispositivos Norito siguen siendo portátiles. Los operadores descargan el paquete `iroha3-<version>-<os>.tar.zst` para unirse a Nexus; consulte `docs/source/sora_nexus_operator_onboarding.md` para la lista de verificación en pantalla completa.

## Bloques de construcción| Componente | Resumen | Ganchos del portal |
|-----------|---------|--------------|
| Espacio de datos (DS) | Dominio de ejecución/almacenamiento definido por gobernanza que posee una o más líneas, declara conjuntos de validadores, clase de privacidad y política de tarifas + DA. | Consulta [Nexus spec](./nexus-spec) para el esquema del manifiesto. |
| Carril | Fragmento determinista de ejecución; emite compromisos que ordena el anillo global NPoS. Las clases de carril incluyen `default_public`, `public_custom`, `private_permissioned` y `hybrid_confidential`. | El [modelo de carril](./nexus-lane-model) captura geometría, prefijos de almacenamiento y retención. |
| Plan de transición | Identificadores de posición, fases de enrutamiento y empaquetado de doble perfil siguen como los despliegues de un solo carril evolucionan hacia Nexus. | Las [notas de transición](./nexus-transition-notes) documentan cada fase de migración. |
| Directorio de espacios | Contrato de registro que almacena manifiestos + versiones de DS. Los operadores concilian las entradas del catálogo contra este directorio antes de unirse. | El rastreador de diferencias de manifiestos vive en `docs/source/project_tracker/nexus_config_deltas/`. || Catálogo de carriles | Sección `[nexus]` de configuración que asigna IDs de carril a alias, políticas de enrutamiento y umbrales DA. `irohad --sora --config ... --trace-config` imprime el catálogo resuelto para auditorias. | Usa `docs/source/sora_nexus_operator_onboarding.md` para el recorrido de CLI. |
| Enrutador de liquidación | Orquestador de transferencias XOR que conecta líneas CBDC privadas con líneas de liquidez públicas. | `docs/source/cbdc_lane_playbook.md` detalla los botones de política y compuertas de telemetría. |
| Telemetría/SLO | Paneles + alertas bajo `dashboards/grafana/nexus_*.json` capturan altura de carriles, backlog DA, latencia de liquidación y profundidad de la cola de gobernanza. | El [plan de remediacion de telemetria](./nexus-telemetry-remediation) detalla los paneles, alertas y evidencia de auditoria. |

## Instantanea de despliegue| Fase | Enfoque | Criterios de salida |
|-------|-------|-----------------------|
| N0 - Beta cerrada | Registrador gestionado por el consejo (`.sora`), incorporación manual de operadores, catálogo de carriles estáticos. | Manifiestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lanzamiento público | Anade sufijos `.nexus`, subastas, registrador de autoservicio, cableado de liquidación XOR. | Pruebas de sincronización de resolutores/gateways, paneles de reconciliación de facturación, simulacros de disputas. |
| N2 - Ampliación | Introduzca `.dao`, API de reventa, analítica, portal de disputas, cuadros de mando de stewards. | Artefactos de cumplimiento versionados, kit de herramientas de jurado de políticas en línea, informes de transparencia del tesoro. |
| Puerta NX-13/12/14 | El motor de cumplimiento, paneles de telemetría y documentación deben salir juntos antes de pilotos con socios. | [Nexus resumen](./nexus-overview) + [Nexus operaciones](./nexus-operations) publicados, paneles conectados, motor de políticas fusionado. |

## Responsabilidades del operador1. **Higiene de configuración** - manten `config/config.toml` sincronizado con el catálogo publicado de carriles y espacios de datos; archiva la salida de `--trace-config` con cada ticket de liberación.
2. **Seguimiento de manifiestos** - concilia las entradas del catálogo con el paquete más reciente del Space Directory antes de unirte o actualizar nodos.
3. **Cobertura de telemetría** - expone los paneles `nexus_lanes.json`, `nexus_settlement.json` y los paneles relacionados del SDK; conecta alertas a PagerDuty y ejecuta revisiones trimestrales según el plan de remediación de telemetría.
4. **Reporte de incidentes** - sigue la matriz de severidad en [Nexus operaciones](./nexus-operations) y presenta RCAs dentro de cinco días habiles.
5. **Preparacion de gobernanza** - asiste a las votaciones del consejo Nexus que impactan tus carriles y ensaya instrucciones de rollback trimestralmente (seguido en `docs/source/project_tracker/nexus_config_deltas/`).

## Ver tambien

- Resumen canónico: `docs/source/nexus_overview.md`
- Especificación detallada: [./nexus-spec](./nexus-spec)
- Geometría de carriles: [./nexus-lane-model](./nexus-lane-model)
- Plan de transición: [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediacion de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook de operaciones: [./nexus-operatives](./nexus-operations)
- Guía de incorporación de operadores: `docs/source/sora_nexus_operator_onboarding.md`