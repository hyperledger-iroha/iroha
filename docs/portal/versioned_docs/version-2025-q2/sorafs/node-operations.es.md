---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: node-operations-es
slug: /sorafs/node-operations-es
---

:::nota Fuente canónica
Espejos `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenga ambas copias alineadas entre versiones.
:::

## Descripción general

Este runbook guía a los operadores en la validación de una implementación integrada de `sorafs-node` dentro de Torii. Cada sección se asigna directamente a los entregables del SF-3: viajes de ida y vuelta de pin/fetch, recuperación de reinicio, rechazo de cuotas y muestreo de PoR.

## 1. Requisitos previos

- Habilitar el trabajador de almacenamiento en `torii.sorafs.storage`:

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

- Asegúrese de que el proceso Torii tenga acceso de lectura/escritura a `data_dir`.
- Confirme que el nodo anuncia la capacidad esperada a través de `GET /v2/sorafs/capacity/state` una vez que se registra una declaración.
- Cuando el suavizado está habilitado, los paneles exponen los contadores de horas GiB/PoR sin procesar y suavizados para resaltar tendencias sin fluctuaciones junto con los valores puntuales.

### Ejecución en seco de CLI (opcional)

Antes de exponer los puntos finales HTTP, puede verificar la integridad del backend de almacenamiento con la CLI incluida.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Los comandos imprimen resúmenes JSON de Norito y rechazan las discrepancias de resumen o perfil de fragmentos, lo que los hace útiles para comprobaciones de humo de CI antes del cableado de Torii.【crates/sorafs_node/tests/cli.rs#L1】

Una vez que Torii esté activo, puede recuperar los mismos artefactos a través de HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos puntos finales son atendidos por el trabajador de almacenamiento integrado, por lo que las pruebas de humo de CLI y las sondas de puerta de enlace permanecen sincronizadas.

## 2. Pin → Obtener ida y vuelta

1. Produzca un paquete de manifiesto + carga útil (por ejemplo, con `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envíe el manifiesto con codificación base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   El JSON de solicitud debe contener `manifest_b64` e `payload_b64`. Una respuesta exitosa devuelve `manifest_id_hex` y el resumen de la carga útil.
3. Obtenga los datos fijados:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifica en Base64 el campo `data_b64` y verifica que coincida con los bytes originales.

## 3. Reiniciar el simulacro de recuperación

1. Fije al menos un manifiesto como se indica arriba.
2. Reinicie el proceso Torii (o todo el nodo).
3. Vuelva a enviar la solicitud de recuperación. La carga útil aún debe poder recuperarse y el resumen devuelto debe coincidir con el valor previo al reinicio.
4. Inspeccione `GET /v2/sorafs/storage/state` para confirmar que `bytes_used` refleja los manifiestos persistentes después del reinicio.

## 4. Prueba de rechazo de cuota

1. Reduzca temporalmente `torii.sorafs.storage.max_capacity_bytes` a un valor pequeño (por ejemplo, el tamaño de un único manifiesto).
2. Fijar un manifiesto; la solicitud debería tener éxito.
3. Intente fijar un segundo manifiesto de tamaño similar. Torii debe rechazar la solicitud con HTTP `400` y un mensaje de error que contenga `storage capacity exceeded`.
4. Restaure el límite de capacidad normal cuando termine.

## 5. Sonda de muestreo PoR

1. Fijar un manifiesto.
2. Solicite una muestra de PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique que la respuesta contenga `samples` con el recuento solicitado y que cada prueba se valide con la raíz del manifiesto almacenado.

## 6. Ganchos de automatización

- Las pruebas de humo/CI pueden reutilizar las comprobaciones específicas agregadas en:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```que cubre `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Los paneles deben realizar un seguimiento de:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - Los contadores de éxito/fracaso de PoR aparecieron a través de `/v2/sorafs/capacity/state`
  - Intentos de publicación de liquidación a través de `sorafs_node_deal_publish_total{result=success|failure}`

Seguir estos simulacros garantiza que el trabajador del almacenamiento integrado pueda ingerir datos, sobrevivir a los reinicios, respetar las cuotas configuradas y generar pruebas de PoR deterministas antes de que el nodo anuncie la capacidad a la red más amplia.
