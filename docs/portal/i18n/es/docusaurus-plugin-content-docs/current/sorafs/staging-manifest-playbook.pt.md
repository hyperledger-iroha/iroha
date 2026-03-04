---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: puesta en escena-manifiesto-libro de estrategias
título: Playbook de manifest em puesta en escena
sidebar_label: Manual de estrategias de manifiesto en la puesta en escena
descripción: Lista de verificación para habilitar o perfil de fragmentador ratificado por el Parlamento en implementaciones Torii de staging.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantenha ambas como copias sincronizadas.
:::

## Visao general

Este manual de estrategias describe cómo habilitar el perfil de fragmentador ratificado por el Parlamento en un despliegue Torii de puesta en escena antes de promover un cambio para la producción. Se supone que la carta de gobierno de SoraFS fue ratificada y que los accesorios canónicos están disponibles en ningún repositorio.

## 1. Requisitos previos

1. Sincronizar los dispositivos canónicos y assinaturas:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare el directorio de sobres de admisión que o Torii lera no startup (caminho exemplo): `/var/lib/iroha/admission/sorafs`.
3. Garantía de que la configuración de Torii habilita la caché de descubrimiento y la aplicación de la admisión:

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

## 2. Sobres de admisión públicos

1. Copia de los sobres de admisión de proveedores aprobados para el directorio referenciado por `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie el Torii (envie un SIGHUP si voce embrulhou o loader com hot reload).
3. Acompañar los registros para mensajes de admisión:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validar una propagación de descubrimiento1. Publicidad de carga útil asociada al anuncio del proveedor (bytes Norito) producida por la canalización del proveedor:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Consulte el punto final de descubrimiento y confirme que el anuncio aparece con alias canónicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Garanta que `profile_aliases` incluye `"sorafs.sf1@1.0.0"` como primera entrada.

## 4. Ejercitar puntos finales de manifiesto y plan

1. Busque los metadatos en el manifiesto (el token de flujo exigido se admite en vigor):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspeccione el JSON y verifique:
   - `chunk_profile_handle` e `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` corresponde al relatorio de determinismo.
   - `chunk_digests_blake3` alinham con aparatos regenerados.

## 5. Comprobaciones de telemetría

- Confirme que o Prometheus expone como nuevas métricas del perfil:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Los paneles deben mostrar el proveedor de staging sollozo o alias esperado y mantener los contadores de apagón en cero en cuanto o perfil estiver ativo.

## 6. Preparación para la implementación

1. Capture una URL corta con información, ID de manifiesto y una instantánea de telemetría.
2. Compartilhe o relatorio no canal de rollout do Nexus com a janela planejada de ativacao em producao.
3. Prossiga para o checklist de producao (Sección 4 en `chunker_registry_rollout_checklist.md`) quando as partes interessadas aprovarem.

Manter este playbook actualizado garantiza que cada lanzamiento de fragmentador/admisión siga los mesmos passos deterministas entre puesta en escena y producción.