---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: Playbook de manifest y staging
sidebar_label: Manual de estrategias de manifiesto y puesta en escena
descripción: Lista de verificación para habilitar el perfil de fragmentador ratificado por el Parlamento en despliegues Torii de staging.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantén ambas copias sincronizadas.
:::

## Resumen

Este manual describe cómo habilitar el perfil de fragmentador ratificado por el Parlamento en un despliegue Torii de staging antes de promover el cambio a producción. Asume que la carta de gobernanza de SoraFS fue ratificada y que los accesorios canónicos están disponibles en el repositorio.

## 1. Prerrequisitos

1. Sincroniza los fixtures canónicos y las firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare el directorio de sobres de admisión que Torii leerá al iniciar (ruta de ejemplo): `/var/lib/iroha/admission/sorafs`.
3. Asegúrese de que la configuración de Torii habilite el caché de descubrimiento y la aplicación de admisión:

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

2. Reinicia Torii (o envía un SIGHUP si envolviste el cargador con recarga en caliente).
3. Revisa los registros para mensajes de admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar la propagación de descubrimiento1. Publica la carga útil firmada por el anuncio del proveedor (bytes Norito) producida por tu tubería de proveedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consulta el endpoint de descubrimiento y confirma que el anuncio aparece con alias canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Asegúrese de que `profile_aliases` incluya `"sorafs.sf1@1.0.0"` como primera entrada.

## 4. Probar los endpoints de manifest y plan

1. Obtenga los metadatos del manifiesto (requiere un token de transmisión si la admisión está activa):

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
   - `manifest_digest_hex` coincide con el informe de determinismo.
   - `chunk_digests_blake3` se alinean con los accesorios regenerados.

## 5. Comprobaciones de telemetría

- Confirma que Prometheus exponen las nuevas métricas del perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los paneles deben mostrar al proveedor de staging bajo el alias esperado y mantener los contadores de apagón en cero mientras el perfil esté activo.

## 6. Preparación para el lanzamiento

1. Captura un informe corto con las URL, el ID del manifiesto y la instantánea de telemetría.
2. Comparte el reporte en el canal de rollout de Nexus junto con la ventana planificada de activación en producción.
3. Continúa con el checklist de producción (Sección 4 en `chunker_registry_rollout_checklist.md`) una vez que las partes interesadas den el visto bueno.Mantener este playbook actualizado asegura que cada rollout de fragmentador/admisión siga los mismos pasos deterministas entre staging y producción.