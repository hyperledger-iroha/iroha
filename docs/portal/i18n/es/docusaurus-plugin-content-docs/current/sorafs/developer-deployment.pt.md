---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: Notas de implementación de SoraFS
sidebar_label: Notas de implementación
descripción: Lista de verificación para promover o tubería da SoraFS de CI para producción.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/developer/deployment.md`. Mantenha ambas como copias sincronizadas.
:::

# Notas de implementación

El flujo de trabajo de empaquetado de SoraFS fortalece el determinismo, entre ellos un pasaje de CI para producir que requiera principalmente barreras operativas. Utilice esta lista de verificación para levantar como herramientas para puertas de enlace y proveedores de armazenamento real.

## Pre-vuelo

- **Alinhamento do registro** - confirme que os perfis de chunker e manifests referenciam a mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admisión** - revisar los anuncios de proveedores assinados y alias pruebas necesarias para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin registro** - mantenha `docs/source/sorafs/runbooks/pin_registry_ops.md` por perto para escenarios de recuperación (rotacao de alias, falhas de replicacao).

## Configuración del ambiente- Las puertas de enlace deben habilitar el punto final de transmisión de prueba (`POST /v1/sorafs/proof/stream`) para que la CLI emita resúmenes de telemetría.
- Configure una política `sorafs_alias_cache` usando los padroes en `iroha_config` o el ayudante de CLI (`sorafs_cli manifest submit --alias-*`).
- Tokens de flujo Forneca (o credenciales Torii) a través de un administrador secreto seguro.
- Habilite los exportadores de telemetría (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y envie para su pila Prometheus/OTel.

## Estrategia de implementación

1. **Se manifiesta azul/verde**
   - Utilice `manifest submit --summary-out` para archivar respuestas de cada implementación.
   - Observe `torii_sorafs_gateway_refusals_total` para captar desajustes de capacidade cedo.
2. **Validación de pruebas**
   - Trate falhas em `sorafs_cli proof stream` como bloqueadores de implementación; Los picos de latencia personalizados indican aceleración del proveedor o niveles mal configurados.
   - `proof verify` debe hacer parte de la prueba de humo pos-pin para garantizar que el CAR hospedado pelos provedores ainda corresponda al resumen del manifiesto.
3. **Paneles de telemetría**
   - Importar `docs/examples/sorafs_proof_streaming_dashboard.json` no Grafana.
   - Adicione Paineis para saude do pin registro (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y estadísticas de rango de fragmentos.
4. **Habilitacao de múltiples fuentes**
   - Siga los pasos de implementación en etapas en `docs/source/sorafs/runbooks/multi_source_rollout.md` para activar el orquestador y archivar artefatos de marcador/telemetría para auditorios.

## Tratamiento de incidentes- Siga los caminos de escalamiento en `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para quedas de gateway y almacenamiento de stream-token.
  - `dispute_revocation_runbook.md` cuando se detectan disputas de replicación.
  - `sorafs_node_ops.md` para manutencao no nivel de nodo.
  - `multi_source_rollout.md` para anular el orquestador, incluir en listas negras de pares y desplegar en etapas.
- Registre falhas de pruebas y anomalías de latencia en GovernanceLog a través de las API de PoR tracker existentes para que un gobierno avalie o desempenho dos proveedores.

## Próximos pasos

- Integre el automático del orquestador (`sorafs_car::multi_fetch`) cuando compruebe el orquestador de búsqueda de fuentes múltiples (SF-6b).
- Actualizaciones complementarias de PDP/PoTR en SF-13/SF-14; El CLI y la documentación van a evolucionar para exportar precios y seleccionar niveles cuando esas pruebas se estabilizan.

Además de combinar estas notas de implementación con inicio rápido y recetas de CI, los equipos pueden pasar experimentos ubicados para tuberías SoraFS en producción con un proceso repetitivo y observable.