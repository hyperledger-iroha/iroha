---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a38d81de4262ff4cbc08aff355ce68856f01915c82c73e59ea7febdb5a5f340f
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: staging-manifest-playbook
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantén ambas copias sincronizadas.
:::

## Resumen

Este playbook describe cómo habilitar el perfil de chunker ratificado por el Parlamento en un despliegue Torii de staging antes de promover el cambio a producción. Asume que la carta de gobernanza de SoraFS fue ratificada y que los fixtures canónicos están disponibles en el repositorio.

## 1. Prerrequisitos

1. Sincroniza los fixtures canónicos y las firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepara el directorio de sobres de admisión que Torii leerá al iniciar (ruta de ejemplo): `/var/lib/iroha/admission/sorafs`.
3. Asegura que la configuración de Torii habilite la caché de discovery y la aplicación de admisión:

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

## 2. Publicar sobres de admisión

1. Copia los sobres de admisión aprobados al directorio referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicia Torii (o envía un SIGHUP si envolviste el loader con recarga en caliente).
3. Revisa los logs para mensajes de admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar la propagación de discovery

1. Publica el payload firmado de provider advert (bytes Norito) producido por tu pipeline de proveedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Consulta el endpoint de discovery y confirma que el advert aparece con aliases canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Asegura que `profile_aliases` incluya `"sorafs.sf1@1.0.0"` como primera entrada.

## 4. Probar los endpoints de manifest y plan

1. Obtén la metadata del manifest (requiere un stream token si la admisión está activa):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspecciona la salida JSON y verifica:
   - `chunk_profile_handle` es `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` coincide con el reporte de determinismo.
   - `chunk_digests_blake3` se alinean con los fixtures regenerados.

## 5. Comprobaciones de telemetría

- Confirma que Prometheus expone las nuevas métricas del perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los dashboards deben mostrar el proveedor de staging bajo el alias esperado y mantener los contadores de brownout en cero mientras el perfil esté activo.

## 6. Preparación para el rollout

1. Captura un reporte corto con las URLs, el ID del manifest y el snapshot de telemetría.
2. Comparte el reporte en el canal de rollout de Nexus junto con la ventana planificada de activación en producción.
3. Continúa con el checklist de producción (Sección 4 en `chunker_registry_rollout_checklist.md`) una vez que las partes interesadas den el visto bueno.

Mantener este playbook actualizado asegura que cada rollout de chunker/admisión siga los mismos pasos deterministas entre staging y producción.
