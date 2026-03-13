---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: Manual de estrategias del manifiesto de preparación SoraFS
sidebar_label: Guía del manifiesto de preparación SoraFS
descripción: Torii کی implementaciones en preparación پر Perfil fragmentador ratificado por el Parlamento فعال کرنے کے لیے lista de verificación۔
---

:::nota مستند ماخذ
:::

## Descripción general

یہ puesta en escena del libro de jugadas Implementación Torii پر Perfil fragmentador ratificado por el Parlamento فعال کرنے کے مراحل بیان کرتا ہے تاکہ تبدیلی کو producción میں promover کرنے سے پہلے تصدیق ہو سکے۔ یہ فرض کرتا ہے کہ SoraFS carta de gobierno ratificar ہو چکا ہے اور repositorio de accesorios canónicos میں موجود ہیں۔

## 1. Requisitos previos

1. Dispositivos canónicos y sincronización de firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. sobres de admisión کی وہ directorio تیار کریں جسے Torii inicio پر پڑھے گا (ruta de ejemplo): `/var/lib/iroha/admission/sorafs`.
3. یقینی بنائیں کہ Torii caché de descubrimiento de configuración اور aplicación de admisión کو habilitar کرتی ہے:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Publicar sobres de admisión کریں

1. Sobres de admisión de proveedores aprobados کو `torii.sorafs.discovery.admission.envelopes_dir` کے directorio میں copia کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii reinicie کریں (یا اگر cargador recarga en caliente کے ساتھ wrap ہے تو SIGHUP بھیجیں).
3. mensajes de admisión کے لیے registros de cola کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar la propagación del descubrimiento کریں

1. canalización del proveedor سے بنے ہوئے carga útil del anuncio del proveedor firmado (Norito bytes) کو post کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```2. consulta de punto final de descubrimiento کریں اور confirmar کریں کہ alias canónicos del anuncio کے ساتھ نظر آتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   یقینی بنائیں کہ `profile_aliases` میں `"sorafs.sf1@1.0.0"` پہلی entrada کے طور پر شامل ہو۔

## 4. Ejercicio de puntos finales del plan manifiesto کریں

1. recuperación de metadatos del manifiesto (اگر admisión hacer cumplir ہے تو token de transmisión درکار ہوگا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Salida JSON inspeccionar کریں اور verificar کریں:
   - `chunk_profile_handle`, `sorafs.sf1@1.0.0` ہو۔
   - Informe de determinismo `manifest_digest_hex` سے coincidencia کرے۔
   - `chunk_digests_blake3` accesorios regenerados سے alineados ہوں۔

## 5. Comprobaciones de telemetría

- تصدیق کریں کہ Prometheus نئی las métricas del perfil exponen کر رہا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- paneles de control کو proveedor de preparación alias esperado کے تحت دکھانا چاہیے اور perfil فعال ہونے پر contadores de caídas de tensión صفر رہنے چاہییں۔

## 6. Preparación para la implementación

1. URL, ID de manifiesto, instantánea de telemetría کے ساتھ مختصر رپورٹ بنائیں۔
2. رپورٹ کو Nexus canal de implementación میں ventana de activación de producción planificada کے ساتھ شیئر کریں۔
3. las partes interesadas کے aprueban کے بعد lista de verificación de producción پر جائیں (Sección 4 en `chunker_registry_rollout_checklist.md`).

اس playbook کو اپ ڈیٹ رکھنا یقینی بناتا ہے کہ ہر puesta en escena del lanzamiento de fragmentación/admisión اور producción میں ایک جیسے pasos deterministas پر چلتا ہے۔