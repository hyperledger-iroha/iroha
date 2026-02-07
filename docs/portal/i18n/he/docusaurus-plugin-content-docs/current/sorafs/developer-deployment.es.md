---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פריסת מפתח
כותרת: Notas de despliegue de SoraFS
sidebar_label: Notas de despliegue
תיאור: Lista de Verificación for Promotor el pipeline de SoraFS de CI a producción.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/developer/deployment.md`. Mantén ambas versiones sincronizadas hasta que los docs heredados se retiren.
:::

# Notas de despliegue

El flujo de empaquetado de SoraFS refuerza la determinación, por lo que pasar de CI a producción requiere principalmente guardarraíles operativos. Usa esta list cuando despliegues la herramienta en gateways y proveedores de almacenamiento reales.

## Preparación previa

- **Alineación del registro** - confirma que los perfiles de chunker y los manifests referencian la misma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admisión** — revisa los adverts de proveedor firmados y los alias proofs necesarios para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de pin registry** — mantén `docs/source/sorafs/runbooks/pin_registry_ops.md` a mano para escenarios de recuperación (rotación de alias, fallos de replicación).

## הגדרת התצורה

- Los gateways deben habilitar el point end de streaming de proofs (`POST /v1/sorafs/proof/stream`) para que el CLI emita resúmenes de telemetria.
- Configura la política `sorafs_alias_cache` usando los valores predeterminados de `iroha_config` o el helper del CLI (`sorafs_cli manifest submit --alias-*`).
- אסימוני זרם פרופורציונה (או credenciales de Torii) מתקדמים ל-gestor de secretos seguro.
- Habilita los exportadores de telemetría (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) y envíalos a tu stack Prometheus/OTel.

## Estrategia de despliegue

1. **מתבטא בכחול/ירוק**
   - ארה"ב `manifest submit --summary-out` עבור ארכיון לאס תשובות קודש.
   - Vigila `torii_sorafs_gateway_refusals_total` עבור זיהוי מכשירי חשמל.
2. **validation de proofs**
   - Trata los fallos en `sorafs_cli proof stream` como bloqueadores del despliegue; los picos de latencia suelen מצביע על מצערת del proveedor o tiers mal configurados.
   - `proof verify` debe formar parte del del עשן מבחן אחורי אל pin para asegurar que el CAR alojado por los proveedores sigue coincidendo con el digest del manifest.
3. **לוחות מחוונים של טלמטריה**
   - Importa `docs/examples/sorafs_proof_streaming_dashboard.json` en Grafana.
   - פאנלים נוספים עבור רישום סיכות (`docs/source/sorafs/runbooks/pin_registry_ops.md`) וטווח נתחים.
4. **Habilitación multi-source**
   - Sigue los pasos de despliegue por etapas en `docs/source/sorafs/runbooks/multi_source_rollout.md` al activar el orquestador y archiva los artefactos de scoreboard/telemetría para auditorías.

## Gestión de incidentes- Sigue las rutas de escalado en `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` עבור שער ה-gateway ו-Agotamiento de stream-token.
  - `dispute_revocation_runbook.md` קיים מחלוקת של העתק.
  - `sorafs_node_ops.md` para mantenimiento a nivel de nodo.
  - `multi_source_rollout.md` לעקוף את ה-orquestador, lists negras de peers y despliegues por etapas.
- Registra fallos de proofs y anomalías de latencia en GovernanceLog mediaante las APIs de PoR tracker existentes para que gobernanza pueda evaluar el rendimiento del proveedor.

## פרוקסימוס פסוס

- Integra la automatización del orquestador (`sorafs_car::multi_fetch`) cuando llegue el orquestador של ריבוי מקורות אחזור (SF-6b).
- Sigue las actualizaciones de PDP/PoTR bajo SF-13/SF-14; el CLI y los docs evolucionarán para exponer plazos y selección de tiers cuando esas proofs se estabilicen.

זה שילוב של מסמכים דה ספיליגו עם התחלה מהירה y las recetas de CI, לוס אקוופוס פועדן פאסאר ניסויים מקומיים של צינורות SoraFS בהפקה עם un processo repetible y צפייה.