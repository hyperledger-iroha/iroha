<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a8347c1f10b45cc72fadd830e246b9fedcf7804cf43631ea85998743bc1f044
source_last_modified: "2025-11-12T17:23:43.290245+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: node-operations
title: Runbook de operaciones del nodo
sidebar_label: Runbook de operaciones del nodo
description: Valida el despliegue integrado de `sorafs-node` dentro de Torii.
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantén ambas versiones sincronizadas hasta que se retire el conjunto de Sphinx.
:::

## Resumen

Este runbook guía a los operadores en la validación de un despliegue `sorafs-node` embebido en Torii. Cada sección se corresponde directamente con los entregables SF-3: recorridos de pin/fetch, recuperación tras reinicio, rechazo por cuota y muestreo PoR.

## 1. Prerrequisitos

- Habilita el worker de almacenamiento en `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Asegúrate de que el proceso Torii tenga acceso de lectura/escritura a `data_dir`.
- Confirma que el nodo anuncia la capacidad esperada vía `GET /v1/sorafs/capacity/state` una vez que se haya registrado una declaración.
- Cuando el suavizado está habilitado, los dashboards exponen tanto los contadores GiB·hour/PoR en bruto como los suavizados para resaltar tendencias sin jitter junto a los valores instantáneos.

### Ejecución en seco de CLI (opcional)

Antes de exponer endpoints HTTP puedes hacer una verificación rápida del backend de almacenamiento con la CLI integrada.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Los comandos imprimen resúmenes Norito JSON y rechazan discrepancias de perfil de chunk o digest, lo que los hace útiles para smoke checks de CI antes de cablear Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensayo de pruebas PoR

Los operadores ahora pueden reproducir artefactos PoR emitidos por gobernanza de forma local antes de subirlos a Torii. La CLI reutiliza la misma ruta de ingesta `sorafs-node`, por lo que las ejecuciones locales exponen exactamente los errores de validación que devolvería la API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

El comando emite un resumen JSON (digest del manifiesto, id del proveedor, digest de prueba, cantidad de muestras y resultado de veredicto opcional). Proporciona `--manifest-id=<hex>` para asegurar que el manifiesto almacenado coincide con el digest del desafío, y `--json-out=<path>` cuando quieras archivar el resumen con los artefactos originales como evidencia de auditoría. Incluir `--verdict` te permite ensayar todo el flujo desafío → prueba → veredicto offline antes de llamar a la API HTTP.

Una vez que Torii está activo puedes recuperar los mismos artefactos vía HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos endpoints son servidos por el worker de almacenamiento embebido, así que los smoke tests de CLI y las sondas del gateway permanecen sincronizados.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Recorrido Pin → Fetch

1. Genera un paquete de manifiesto + payload (por ejemplo con `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envía el manifiesto con codificación base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   El JSON de la solicitud debe contener `manifest_b64` y `payload_b64`. Una respuesta exitosa devuelve `manifest_id_hex` y el digest del payload.
3. Recupera los datos fijados:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifica en base64 el campo `data_b64` y verifica que coincida con los bytes originales.

## 3. Simulacro de recuperación tras reinicio

1. Fija al menos un manifiesto como arriba.
2. Reinicia el proceso Torii (o el nodo completo).
3. Reenvía la solicitud de fetch. El payload debe seguir siendo recuperable y el digest devuelto debe coincidir con el valor previo al reinicio.
4. Inspecciona `GET /v1/sorafs/storage/state` para confirmar que `bytes_used` refleja los manifiestos persistidos tras el reinicio.

## 4. Prueba de rechazo por cuota

1. Reduce temporalmente `torii.sorafs.storage.max_capacity_bytes` a un valor pequeño (por ejemplo el tamaño de un solo manifiesto).
2. Fija un manifiesto; la solicitud debe tener éxito.
3. Intenta fijar un segundo manifiesto de tamaño similar. Torii debe rechazar la solicitud con HTTP `400` y un mensaje de error que incluya `storage capacity exceeded`.
4. Restaura el límite de capacidad normal al finalizar.

## 5. Sonda de muestreo PoR

1. Fija un manifiesto.
2. Solicita un muestreo PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifica que la respuesta contiene `samples` con el conteo solicitado y que cada prueba valida contra la raíz del manifiesto almacenado.

## 6. Ganchos de automatización

- CI / smoke tests pueden reutilizar las comprobaciones dirigidas añadidas en:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cubren `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` y `por_sampling_returns_verified_proofs`.
- Los dashboards deben seguir:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - contadores de éxito/fallo de PoR expuestos vía `/v1/sorafs/capacity/state`
  - intentos de publicación de settlement vía `sorafs_node_deal_publish_total{result=success|failure}`

Seguir estos ejercicios garantiza que el worker de almacenamiento embebido pueda ingerir datos, sobrevivir reinicios, respetar cuotas configuradas y generar pruebas PoR deterministas antes de que el nodo anuncie capacidad a la red más amplia.
