---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-es
slug: /sorafs/staging-manifest-playbook-es
---

:::nota Fuente canónica
Espejos `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenga ambas copias alineadas entre versiones.
:::

## Descripción general

Este manual explica cómo habilitar el perfil fragmentador ratificado por el Parlamento en una implementación provisional de Torii antes de promover el cambio a producción. Se supone que se ha ratificado el estatuto de gobernanza SoraFS y que los accesorios canónicos están disponibles en el repositorio.

## 1. Requisitos previos

1. Sincroniza los aparatos canónicos y las firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare el directorio del sobre de admisión que Torii leerá al inicio (ruta de ejemplo): `/var/lib/iroha/admission/sorafs`.
3. Asegúrese de que la configuración Torii habilite la caché de descubrimiento y la aplicación de admisión:

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

1. Copie los sobres de admisión de proveedores aprobados en el directorio al que hace referencia `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie Torii (o envíe un SIGHUP si envolvió el cargador con recarga sobre la marcha).
3. Siga los registros de mensajes de admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar la propagación del descubrimiento

1. Publique la carga útil del anuncio del proveedor firmado (Norito bytes) producido por su
   canalización de proveedores:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Consulta el punto final de descubrimiento y confirma que el anuncio aparece con alias canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Asegúrese de que `profile_aliases` incluya `"sorafs.sf1@1.0.0"` como primera entrada.

## 4. Puntos finales del plan y manifiesto del ejercicio

1. Obtenga los metadatos del manifiesto (requiere un token de transmisión si se aplica la admisión):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspeccione la salida JSON y verifique:
   - `chunk_profile_handle` es `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` coincide con el informe de determinismo.
   - `chunk_digests_blake3` alineado con los accesorios regenerados.

## 5. Comprobaciones de telemetría

- Confirme que Prometheus expone las nuevas métricas del perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los paneles deben mostrar el proveedor provisional con el alias esperado y mantener los contadores de caídas de tensión en cero mientras el perfil esté activo.

## 6. Preparación para la implementación

1. Capture un informe breve con las URL, el ID del manifiesto y la instantánea de telemetría.
2. Comparta el informe en el canal de implementación Nexus junto con la ventana de activación de producción planificada.
3. Continúe con la lista de verificación de producción (Sección 4 en `chunker_registry_rollout_checklist.md`) una vez que las partes interesadas aprueben.

Mantener este manual actualizado garantiza que cada lanzamiento de fragmentos/admisiones siga los mismos pasos deterministas en la puesta en escena y la producción.
