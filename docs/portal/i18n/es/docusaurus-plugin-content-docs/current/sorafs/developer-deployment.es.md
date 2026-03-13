---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: Notas de implementación de SoraFS
sidebar_label: Notas de implementación
descripción: Lista de verificación para promover el pipeline de SoraFS de CI a producción.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/deployment.md`. Mantén ambas versiones sincronizadas hasta que los documentos heredados se retiren.
:::

# Notas de despliegue

El flujo de empaquetado de SoraFS refuerza la determinación, por lo que pasar de CI a producción requiere principalmente guardarraíles operativos. Utilice esta lista cuando despliegue la herramienta en gateways y proveedores de almacenamiento reales.

## Preparación previa

- **Alineación del registro** — confirma que los perfiles de chunker y los manifests referencian la misma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admisión** — revisa los anuncios de proveedor firmados y los alias pruebas necesarias para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de pin registro** — mantén `docs/source/sorafs/runbooks/pin_registry_ops.md` a mano para escenarios de recuperación (rotación de alias, fallos de replicación).

## Configuración del entorno- Los gateways deben habilitar el endpoint de streaming de pruebas (`POST /v2/sorafs/proof/stream`) para que el CLI emita resúmenes de telemetría.
- Configure la política `sorafs_alias_cache` usando los valores predeterminados de `iroha_config` o el asistente del CLI (`sorafs_cli manifest submit --alias-*`).
- Proporciona tokens de flujo (o credenciales de Torii) mediante un gestor de secretos seguro.
- Habilita los exportadores de telemetría (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y envíalos a tu pila Prometheus/OTel.

## Estrategia de despliegue1. **Se manifiesta azul/verde**
   - Usa `manifest submit --summary-out` para archivar las respuestas de cada despliegue.
   - Vigile `torii_sorafs_gateway_refusals_total` para detectar desajustes de capacidades temprano.
2. **Validación de pruebas**
   - Trata los fallos en `sorafs_cli proof stream` como bloqueadores del despliegue; Los picos de latencia suelen indicar limitación del proveedor o niveles mal configurados.
   - `proof verify` debe formar parte del smoke test posterior al pin para asegurar que el CAR alojado por los proveedores sigue coincidiendo con el digest del manifest.
3. **Paneles de telemetría**
   - Importa `docs/examples/sorafs_proof_streaming_dashboard.json` y Grafana.
   - Agregue paneles adicionales para la salud del registro de pines (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y estadísticas de rango de fragmentos.
4. **Habilitación multifuente**
   - Siga los pasos de implementación por etapas en `docs/source/sorafs/runbooks/multi_source_rollout.md` al activar el orquestador y archiva los artefactos de marcador/telemetría para auditorías.

##Gestión de incidentes- Sigue las rutas de escalada en `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para caídas de gateway y agotación de stream-token.
  - `dispute_revocation_runbook.md` cuando ocurren disputas de replicación.
  - `sorafs_node_ops.md` para mantenimiento a nivel de nodo.
  - `multi_source_rollout.md` para anulaciones del orquestador, listas negras de pares y despliegues por etapas.
- Registre fallos de pruebas y anomalías de latencia en GovernanceLog mediante las API de PoR tracker existentes para que gobernanza pueda evaluar el rendimiento del proveedor.

## Próximos pasos

- Integra la automatización del orquestador (`sorafs_car::multi_fetch`) cuando llegue el orquestador de multi-source fetch (SF-6b).
- Sigue las actualizaciones de PDP/PoTR bajo SF-13/SF-14; el CLI y los documentos evolucionarán para exponer plazos y selección de niveles cuando esas pruebas se estabilicen.

Al combinar estas notas de implementación con el inicio rápido y las recetas de CI, los equipos pueden pasar de experimentos locales a pipelines SoraFS en producción con un proceso repetible y observable.