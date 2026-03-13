---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de nodo
título: Runbook de operacoes no
sidebar_label: Runbook de operaciones no lo hace
descripción: Valide a implantacao embutida de `sorafs-node` dentro de Torii.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas as versos sincronizadas ate que o conjunto Sphinx seja retirado.
:::

## Visao general

Este runbook guía operadores na validacao de uma implantacao `sorafs-node` embutida no Torii. Cada seco se conecta directamente a los entregaveis SF-3: ciclos pin/fetch, recuperacao apos reinicio, rejeicao por cuota y amostragem PoR.

## 1. Requisitos previos

- Habilitación del trabajador de almacenamiento en `torii.sorafs.storage`:

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

- Garantía de que el proceso Torii tiene acceso a lectura/escrita en `data_dir`.
- Confirme que o no anuncia a capacidade esperada via `GET /v2/sorafs/capacity/state` quando uma declaracao for registrada.
- Cuando el suavizado está habilitado, los paneles de control exponen contadores GiB·hour/PoR brutos y suavizados para destacar tendencias sin jitter al lado de valores instantáneos.

### Ejecución en seco de CLI (opcional)

Antes de exportar los puntos finales HTTP, puede validar el backend de almacenamiento con una CLI embarcada.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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
```Los comandos imprimen los resúmenes Norito JSON y recusam divergencias de chunk-profile o digest, tornando-os uteis para smoke checks de CI antes de hacer el cableado de Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de prueba PoR

Operadores agora podem reproduzir artefatos PoR emitidos pelagobernanza localmente antes de enviar-los ao Torii. A CLI reutiliza el mismo camino de ingesta `sorafs-node`, el puerto ejecuta localmente detecta exactamente los errores de validación que retornaría una API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

El comando emite un resumen JSON (resumen del manifiesto, identificación del proveedor, resumen de la prueba, contagio de demostraciones y veredito opcional). Forneca `--manifest-id=<hex>` para garantizar que el manifiesto armado corresponde al resumen del desafío, e `--json-out=<path>` cuando quiere archivar el currículum con los artefatos originales como evidencia de auditoría. Incluir `--verdict` permite probar el flujo completo desafio → probar → veredito offline antes de chamar una API HTTP.

Después de que o Torii estiver ativo voce pode recuperar os mesmos artefactos a través de HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos os endpoints son servidos por el trabajador de almacenamiento embutido, portanto smoke tests de CLI y sondas de gateway permanecen sincronizados.

## 2. Ciclo Pin → Recuperar1. Utilice un paquete de manifiesto + carga útil (por ejemplo, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie el manifiesto con codificación base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   El JSON requerido debe contener `manifest_b64` e `payload_b64`. Una respuesta de éxito retorna `manifest_id_hex` y el resumen de la carga útil.
3. Busque los dados fijos:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifique el campo `data_b64` en base64 y verifique si corresponden a los bytes originales.

## 3. Ejercicio de recuperación apos reiniciado

1. Fixe pelo menos um manifest como acima.
2. Reinicia el proceso Torii (o no entero).
3. Vuelva a enviar la solicitud de recuperación. La carga útil debe continuar disponible y el resumen retornado debe coincidir con el valor anterior al reinicio.
4. Inspeccione `GET /v2/sorafs/storage/state` para confirmar que `bytes_used` refleja los manifiestos persistentes después del reinicio.

## 4. Teste de rejeicao por cuota

1. Reduza temporalmente `torii.sorafs.storage.max_capacity_bytes` para un valor pequeño (por ejemplo o tamanho de un único manifiesto).
2. Arreglar un manifiesto; a requisicao deve ter sucesso.
3. Tente fixar um segundo manifest de tamanho semelhante. O Torii debe solicitar un pedido con HTTP `400` y un mensaje de error que indica `storage capacity exceeded`.
4. Restaure o limite de capacidade normal al finalizar.

## 5. Sonda de amstragem PoR

1. Arreglar un manifiesto.
2. Solicite uma amostra PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. Verifique se a resposta contem `samples` com a contagem solicitada e se cada prova valida contra a raiz do manifest armazenado.

## 6. Ganchos de automacao

- Las pruebas de CI/humo pueden reutilizarse según las verificacoes direcionadas adicionadas em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cobrem `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Los paneles de control deben acompañarse:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - contadores de éxito/falha PoR expostos vía `/v2/sorafs/capacity/state`
  - tentativas de publicación de acuerdo vía `sorafs_node_deal_publish_total{result=success|failure}`

Seguir esses ejercicios garantizados que o trabajador de almacenamiento embutido consiga ingerir datos, sobreviver a reinicios, respetar cuotas configuradas y gerar provas PoR determinísticas antes de o no anunciar capacidade para a rede mais ampla.