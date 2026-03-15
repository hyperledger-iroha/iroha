---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: Implementación del registro fragmentador SoraFS چیک لسٹ
sidebar_label: Lanzamiento de fragmentos چیک لسٹ
descripción: actualizaciones del registro fragmentado کے لیے قدم بہ قدم implementación پلان۔
---

:::nota مستند ماخذ
:::

# SoraFS Implementación del registro چیک لسٹ

یہ چیک لسٹ نئے perfil fragmentario یا paquete de admisión de proveedores کو revisión سے producción
تک promover کرنے کے لیے درکار مراحل کو capturar کرتی ہے جب carta de gobernanza
ratificar ہو چکا ہو۔

> **Alcance:** Lanzamientos de تمام پر لاگو ہے جو
> `sorafs_manifest::chunker_registry`, sobres de admisión de proveedores, یا canónicos
> paquetes de accesorios (`fixtures/sorafs_chunker/*`) میں تبدیلی کریں۔

## 1. Validación previa al vuelo

1. los accesorios دوبارہ generan کریں اور determinismo verifican کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ informe de perfil) میں
   determinismo hashes artefactos regenerados سے coincidencia کریں۔
3. یقینی بنائیں کہ `sorafs_manifest::chunker_registry`،
   `ensure_charter_compliance()` کے ساتھ compilar ہوتا ہے، چلائیں:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. expediente de propuesta اپڈیٹ کریں:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada del acta del consejo `docs/source/sorafs/council_minutes_*.md`
   - Informe de determinismo

## 2. Aprobación de la gobernanza1. Informe del grupo de trabajo sobre herramientas y resumen de la propuesta.
   Panel de Infraestructura del Parlamento de Sora میں پیش کریں۔
2. detalles de aprobación کو
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. Sobre firmado por el Parlamento کو accesorios کے ساتھ publicar کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. ayudante de búsqueda de gobernanza کے ذریعے sobre accesible ہونے کی تصدیق کریں:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Lanzamiento de la puesta en escena

ان pasos کی تفصیلی tutorial کے لیے [manual de estrategias del manifiesto de preparación](./staging-manifest-playbook) دیکھیں۔

1. Torii y `torii.sorafs` descubrimiento habilitado y aplicación de admisión en
   (`enforce_admission = true`) کے ساتھ implementar کریں۔
2. sobres de admisión de proveedores aprobados کو directorio de registro de preparación میں push کریں
   جسے `torii.sorafs.discovery.admission.envelopes_dir` consulte کرتا ہے۔
3. API de descubrimiento کے ذریعے anuncios del proveedor کی verificación de propagación کریں:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. encabezados de gobernanza کے ساتھ ejercicio de puntos finales de manifiesto/plan کریں:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. paneles de telemetría (`torii_sorafs_*`) y reglas de alerta en el perfil کی
   رپورٹنگ بغیر errores کے confirmar کریں۔

## 4. Lanzamiento de la producción1. pasos de preparación para la producción de nodos Torii y repetición de nodos
2. ventana de activación (fecha/hora, período de gracia, plan de reversión) کو operador اور SDK
   canales پر anunciar کریں۔
3. lanzamiento de fusión de relaciones públicas کریں جس میں شامل ہو:
   - accesorios actualizados y sobre
   - cambios en la documentación (referencias a los estatutos, informe de determinismo)
   - hoja de ruta/actualización de estado
4. etiqueta de lanzamiento کریں اور artefactos firmados کو procedencia کے لیے archivo کریں۔

## 5. Auditoría posterior al lanzamiento

1. implementación en 24 horas de métricas finales (recuentos de descubrimiento, tasa de éxito de recuperación, error
   histogramas) capturar کریں۔
2. `status.md` کو مختصر resumen اور determinismo informe کے enlace کے ساتھ actualización کریں۔
3. tareas de seguimiento (مثلاً اضافی guía de creación de perfiles) کو `roadmap.md` میں درج کریں۔