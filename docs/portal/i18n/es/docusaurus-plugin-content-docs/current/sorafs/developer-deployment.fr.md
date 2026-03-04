---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: Notas de implementación SoraFS
sidebar_label: Notas de implementación
descripción: Lista de verificación para promover el pipeline SoraFS de la CI versus la producción.
---

:::nota Fuente canónica
:::

# Notas de implementación

El flujo de trabajo de embalaje SoraFS refuerza el determinismo, ya que el paso del CI a la producción es necesario sobre operaciones cuidadosas. Utilice esta lista de verificación para el despliegue de la salida en las puertas de enlace y proveedores de carretes de almacenamiento.

## Pre-vol

- **Alineación del registro**: confirme que los perfiles de fragmentador y los manifiestos hacen referencia a la misma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politique d'admission** — revoyez les adverts de fournisseurs signés et les aliasproofs nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin registro** — gardez `docs/source/sorafs/runbooks/pin_registry_ops.md` à portée pour les scénarios de reprise (rotation d'alias, échecs de replication).

## Configuración del entorno- Las puertas de enlace deben activar el punto final de transmisión de prueba (`POST /v1/sorafs/proof/stream`) para que la CLI pueda reproducir currículums de televisión.
- Configure la política `sorafs_alias_cache` utilizando los valores predeterminados de `iroha_config` o la CLI auxiliar (`sorafs_cli manifest submit --alias-*`).
- Proporcionar tokens de transmisión (o identificadores Torii) a través de un administrador de secretos seguros.
- Active los exportadores de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y envíelos a su pila Prometheus/OTel.

## Estrategia de implementación1. **Se manifiesta azul/verde**
   - Utilice `manifest submit --summary-out` para archivar las respuestas de cada implementación.
   - Vigile `torii_sorafs_gateway_refusals_total` para detectar discrepancias de capacidad.
2. **Validación de pruebas**
   - Traitez les échecs de `sorafs_cli proof stream` como los bloqueadores de implementación; Las imágenes de latencia indican que el proveedor está acelerando o que los niveles están mal configurados.
   - `proof verify` haga parte de la prueba de humo posterior al pin para asegurarse de que el CAR Hébergé por los proveedores corresponda siempre al resumen del manifiesto.
3. **Paneles de televisión**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` en Grafana.
   - Ajuste los paneles para la salud del registro de pines (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y las estadísticas de rango de fragmentos.
4. **Activación multifuente**
   - Siga las etapas de implementación progresiva en `docs/source/sorafs/runbooks/multi_source_rollout.md` durante la activación del orquestador y archive los artefactos/télémétrie para las auditorías.

## Gestión de incidentes- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` para los paneles de puerta de enlace y el escape de tokens de flujo.
  - `dispute_revocation_runbook.md` lors de litiges de replication.
  - `sorafs_node_ops.md` para el mantenimiento del nivel de nudos.
  - `multi_source_rollout.md` para anular las grabaciones del orquestador, incluir en la lista negra a los compañeros y realizar implementaciones por etapas.
- Registre las pruebas de prueba y las anomalías de latencia en GovernanceLog a través de la API de PoR tracker existentes para que la gobernanza pueda evaluar el desempeño de los proveedores.

## Prochaines étapes

- Intégrez la automatización del orquestador (`sorafs_car::multi_fetch`) cuando la búsqueda de fuentes múltiples del orquestador (SF-6b) estará disponible.
- Suivez les mises à jour PDP/PoTR sous SF-13/SF-14; La CLI y los documentos evolucionan para exponer los plazos y la selección de niveles una vez que estas pruebas se estabilicen.

Al combinar estas notas de implementación con el inicio rápido y las recetas CI, los equipos pueden pasar de experimentos locales a las tuberías SoraFS en producción con un proceso repetido y observable.