---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: Плейбук манифеста для puesta en escena
sidebar_label: Плейбук манифеста для puesta en escena
descripción: Чеклист для включения профиля chunker, ратифицированного парламентом, на staging-развертываниях Torii.
---

:::nota Канонический источник
:::

## Objeto

Este bloque de descripción del perfil de fragmentación, el parlamento actualizado, en la etapa de puesta en escena Torii antes продвижением изменений в прод. Previamente, el dispositivo actualizado SoraFS se actualiza y se encuentran disponibles accesorios canónicos en los repositorios.

## 1. Предварительные условия

1. Sincronización de accesorios y accesorios canónicos:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте каталог sobres de admisión, который Torii читает при старте (пример пути): `/var/lib/iroha/admission/sorafs`.
3. Tenga en cuenta que la configuración Torii incluye caché de descubrimiento y admisión de cumplimiento:

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

## 2. Публикация sobres de admisión

1. Escriba los sobres de admisión de proveedores en el catálogo, disponibles en `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Presione Torii (o activa SIGHUP o activa la recarga en caliente).
3. Следите за логами admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка распространения descubrimiento

1. Publique la carga útil del anuncio del proveedor (байты Norito), сформированный вашим pipeline провайдера:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```2. Prosiga el descubrimiento de puntos finales y siga los anuncios que aparecen con alias canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Tenga en cuenta que `profile_aliases` contiene `"sorafs.sf1@1.0.0"` sin ningún elemento.

## 4. Manifiesto y plan de comprobación de puntos finales

1. Consultar el manifiesto de metadatos (token de flujo, y admisión forzada):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Pruebe el formato JSON y consulte, como:
   - `chunk_profile_handle` parte `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` se activa con el detector de temperatura.
   - `chunk_digests_blake3` se adapta a accesorios regenerativos.

## 5. Telemetros de prueba

- Tenga en cuenta que Prometheus publica nuevos perfiles de parámetros:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los tableros que permiten la puesta en escena, el alias de puesta en escena y la caída de tensión en un perfil activo.

## 6. Lanzamiento de Готовность к

1. Configure las opciones de configuración con direcciones URL, manifiesto de ID y televisores de instantáneas.
2. Utilice el canal Nexus para iniciar sesión en el programa.
3. Siga las instrucciones del producto (Sección 4 de `chunker_registry_rollout_checklist.md`) después de la instalación.

Este paquete se puede implementar en las garantías actuales, cómo se realiza el resumen de implementación/admisión de los archivos y temas que se determinan шагам между puesta en escena y producción.