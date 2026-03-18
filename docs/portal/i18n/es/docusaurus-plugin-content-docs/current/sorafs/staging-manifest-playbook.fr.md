---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: Playbook de manifest y staging
sidebar_label: Manual de estrategias de manifiesto y puesta en escena
descripción: Lista de verificación para activar el perfil fragmentador ratificado por el Parlamento sobre las implementaciones Torii de staging.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Guarde la copia Docusaurus y el legado de Markdown se alinea justo al retraite complète du set Sphinx.
:::

## Vista del conjunto

Este libro de jugadas describe la activación del perfil fragmentador ratificado por el Parlamento en un despliegue Torii de puesta en escena antes de promover el cambio en producción. Supongo que la carta de gobierno SoraFS está ratificada y que los accesorios canónicos están disponibles en el depósito.

## 1. Requisitos previos

1. Sincronice los dispositivos canónicos y las firmas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare el repertorio de sobres de admisión que Torii lira au démarrage (camino de ejemplo): `/var/lib/iroha/admission/sorafs`.
3. Asegúrese de que la configuración Torii active el descubrimiento de caché y la aplicación de admisión:

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

## 2. Publicar los sobres de admisión

1. Copie los sobres de admisión aprobados en el repertorio referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```2. Redémarrez Torii (o envíe un SIGHUP si tiene el cargador encapsulado con una recarga en caliente).
3. Suivez los registros para los mensajes de admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar el descubrimiento de propagación

1. Publique la carga útil firmada por el proveedor del anuncio (octetos Norito) del producto par su proveedor de tuberías:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Interrogue el descubrimiento de puntos finales y confirme que el anuncio aparece con los alias canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Asegúrese de que `profile_aliases` incluya `"sorafs.sf1@1.0.0"` en primer plato principal.

## 4. Ejercer el manifiesto y plan de puntos finales

1. Recupérez les métadonnées du manifest (necesitará un token de transmisión si la admisión está aplicada):

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
   - `manifest_digest_hex` corresponden a la relación de determinismo.
   - `chunk_digests_blake3` está alineado con los accesorios regénérées.

## 5. Verificaciones de télémétrie

- Confirme que Prometheus expone las nuevas medidas de perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los paneles de control deben controlar el proveedor de preparación bajo el alias asistente y guardar los ordenadores de apagón a cero mientras el perfil esté activo.

## 6. Preparación del lanzamiento1. Capturez un informe judicial con las URL, el ID de manifiesto y la instantánea de televisión.
2. Partagez le rapport dans le canal de rollout Nexus con la ventana de activación planificada de producción.
3. Passez à la checklist de production (Sección 4 en `chunker_registry_rollout_checklist.md`) una vez que las partes embarazadas no estén de acuerdo.

Mantener este libro de jugadas al día garantiza que cada lanzamiento/admisión se adapte a las mismas etapas determinadas entre la puesta en escena y la producción.