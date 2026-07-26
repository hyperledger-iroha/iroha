---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f370cbc81b9e8f340a24d561d94f49cf154413781f7194505a712758cd4c85aa
source_last_modified: "2025-11-10T05:16:44.297906+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: developer-deployment
title: Notas de despliegue de SoraFS
sidebar_label: Notas de despliegue
description: Lista de verificación para promover el pipeline de SoraFS de CI a producción.
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/developer/deployment.md`. Mantén ambas versiones sincronizadas hasta que los docs heredados se retiren.
:::

# Notas de despliegue

El flujo de empaquetado de SoraFS refuerza la determinación, por lo que pasar de CI a producción requiere principalmente guardarraíles operativos. Usa esta lista cuando despliegues la herramienta en gateways y proveedores de almacenamiento reales.

## Preparación previa

- **Alineación del registro** — confirma que los perfiles de chunker y los manifests referencian la misma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admisión** — revisa los adverts de proveedor firmados y los alias proofs necesarios para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de pin registry** — mantén `docs/source/sorafs/runbooks/pin_registry_ops.md` a mano para escenarios de recuperación (rotación de alias, fallos de replicación).

## Configuración del entorno

- Los gateways deben habilitar el endpoint de streaming de proofs (`POST /v1/sorafs/proof/stream`) para que el CLI emita resúmenes de telemetría.
- Configura la política `sorafs_alias_cache` usando los valores predeterminados de `iroha_config` o el helper del CLI (`sorafs_cli manifest submit --alias-*`).
- Proporciona stream tokens (o credenciales de Torii) mediante un gestor de secretos seguro.
- Habilita los exportadores de telemetría (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y envíalos a tu stack Prometheus/OTel.

## Estrategia de despliegue

1. **Manifests blue/green**
   - Usa `manifest submit --summary-out` para archivar las respuestas de cada despliegue.
   - Vigila `torii_sorafs_gateway_refusals_total` para detectar desajustes de capacidades temprano.
2. **Validación de proofs**
   - Trata los fallos en `sorafs_cli proof stream` como bloqueadores del despliegue; los picos de latencia suelen indicar throttling del proveedor o tiers mal configurados.
   - `proof verify` debe formar parte del smoke test posterior al pin para asegurar que el CAR alojado por los proveedores sigue coincidiendo con el digest del manifest.
3. **Dashboards de telemetría**
   - Importa `docs/examples/sorafs_proof_streaming_dashboard.json` en Grafana.
   - Añade paneles adicionales para la salud del pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y estadísticas de chunk range.
4. **Habilitación multi-source**
   - Sigue los pasos de despliegue por etapas en `docs/source/sorafs/runbooks/multi_source_rollout.md` al activar el orquestador y archiva los artefactos de scoreboard/telemetría para auditorías.

## Gestión de incidentes

- Sigue las rutas de escalado en `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para caídas de gateway y agotamiento de stream-token.
  - `dispute_revocation_runbook.md` cuando ocurran disputas de replicación.
  - `sorafs_node_ops.md` para mantenimiento a nivel de nodo.
  - `multi_source_rollout.md` para overrides del orquestador, listas negras de peers y despliegues por etapas.
- Registra fallos de proofs y anomalías de latencia en GovernanceLog mediante las APIs de PoR tracker existentes para que gobernanza pueda evaluar el rendimiento del proveedor.

## Próximos pasos

- Integra la automatización del orquestador (`sorafs_car::multi_fetch`) cuando llegue el orquestador de multi-source fetch (SF-6b).
- Sigue las actualizaciones de PDP/PoTR bajo SF-13/SF-14; el CLI y los docs evolucionarán para exponer plazos y selección de tiers cuando esas proofs se estabilicen.

Al combinar estas notas de despliegue con el quickstart y las recetas de CI, los equipos pueden pasar de experimentos locales a pipelines SoraFS en producción con un proceso repetible y observable.
