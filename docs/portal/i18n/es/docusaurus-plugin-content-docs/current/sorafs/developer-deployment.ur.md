---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de desarrollador
título: notas de implementación SoraFS
sidebar_label: notas de implementación
descripción: Lista de verificación de promoción de canalización de CI سے producción تک SoraFS کرنے کی
---

:::nota مستند ماخذ
:::

# Notas de implementación

SoraFS Determinismo del flujo de trabajo de empaquetado مضبوط کرتا ہے، اس لئے CI سے Production پر جانا بنیادی طور پر operacional guardrails مانگتا ہے۔ جب آپ ٹولنگ کو حقیقی gateways اور proveedores de almacenamiento پر implementación کریں تو یہ lista de verificación استعمال کریں۔

## Pre-vuelo

- **Alineación del registro** — تصدیق کریں کہ perfiles fragmentados اور manifiestos ایک ہی `namespace.name@semver` tupla کو refer کرتے ہیں (`docs/source/sorafs/chunker_registry.md`).
- **Política de admisión** — `manifest submit` کے لئے درکار anuncios de proveedores firmados اور alias pruebas کا جائزہ لیں (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de registro de PIN**: escenarios de recuperación (rotación de alias, fallas de replicación) کے لئے `docs/source/sorafs/runbooks/pin_registry_ops.md` قریب رکھیں۔

## Configuración del entorno

- El punto final de transmisión a prueba de puertas de enlace (`POST /v1/sorafs/proof/stream`) habilita la emisión de resúmenes de telemetría CLI de کر سکے۔
- Política `sorafs_alias_cache` y valores predeterminados de `iroha_config` y ayudante de CLI (`sorafs_cli manifest submit --alias-*`) y configuración de configuración
- Tokens de transmisión (credenciales Torii) کو ایک محفوظ administrador secreto سے فراہم کریں۔
- Los exportadores de telemetría (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) habilitan کریں اور انہیں اپنے Prometheus/OTel stack میں ship کریں۔

## Estrategia de implementación1. **Manifiestos azul/verde**
   - ہر lanzamiento کے لئے archivo de respuestas کرنے کے لئے `manifest submit --summary-out` استعمال کریں۔
   - `torii_sorafs_gateway_refusals_total` پر نظر رکھیں تاکہ discrepancias de capacidad جلدی پکڑ لیں۔
2. **Validación de prueba**
   - `sorafs_cli proof stream` Errores de configuración y bloqueadores de implementación picos de latencia, limitación del proveedor, niveles mal configurados, کی نشاندہی کرتے ہیں۔
   - Prueba de humo posterior al pin میں `proof verify` شامل کریں تاکہ یقینی ہو کہ proveedores پر CAR alojado اب بھی resumen de manifiesto سے coincidencia کرتا ہے۔
3. **Paneles de telemetría**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` کو Grafana میں importación کریں۔
   - Estado del registro de pines (`docs/source/sorafs/runbooks/pin_registry_ops.md`) y estadísticas de rango de fragmentos para paneles de control
4. **Habilitación de múltiples fuentes**
   - Orchestrator آن کرتے وقت `docs/source/sorafs/runbooks/multi_source_rollout.md` کے pasos de implementación por etapas فالو کریں، اور auditorías کے لئے archivo de artefactos de telemetría/marcador de resultados کریں۔

## Manejo de incidentes

- `docs/source/sorafs/runbooks/` میں rutas de escalada فالو کریں:
  - Interrupciones de la puerta de enlace `sorafs_gateway_operator_playbook.md` y agotamiento del token de transmisión کے لئے۔
  - `dispute_revocation_runbook.md` جب disputas de replicación ہوں۔
  - Mantenimiento a nivel de nodo `sorafs_node_ops.md` کے لئے۔
  - Anulaciones del orquestador `multi_source_rollout.md`, listas negras de pares, implementaciones por etapas
- Fallos de prueba, anomalías de latencia, GovernanceLog, API de seguimiento de PoR, registros, evaluación del rendimiento del proveedor de gobernanza, evaluación del rendimiento.

## Próximos pasos- Orquestador de búsqueda de fuentes múltiples (SF-6b) y automatización del orquestador (`sorafs_car::multi_fetch`) integran کریں۔
- Actualizaciones de PDP/PoTR کو SF-13/SF-14 کے تحت track کریں؛ جب یہ pruebas estabilizan ہوں تو CLI اور documentos plazos اور superficie de selección de niveles کریں گے۔

Notas de implementación, inicio rápido, recetas de CI, experimentos locales, tuberías SoraFS de grado de producción, procesos repetibles y observables. ہیں۔