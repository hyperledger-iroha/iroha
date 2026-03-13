---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: Runbook de operações do nó
sidebar_label: Runbook de operações do nó
description: Valida o despliegue integrado de `sorafs-node` dentro de Torii.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas as versões sincronizadas até que o conjunto de Sphinx seja retirado.
:::

## Resumo

Este runbook guia os operadores na validação de um aplicativo `sorafs-node` incorporado em Torii. Cada seção corresponde diretamente às entregas SF-3: corridas de pin/fetch, recuperação de retorno, rechazo por cuota e muestreo PoR.

## 1. Pré-requisitos

- Habilita o trabalhador de almacenamiento em `torii.sorafs.storage`:

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

- Certifique-se de que o processo Torii tenha acesso à leitura/escritura em `data_dir`.
- Confirme que o nó anuncia a capacidade esperada via `GET /v2/sorafs/capacity/state` uma vez que uma declaração foi registrada.
- Quando a suavização está habilitada, os painéis expõem tanto os contadores GiB·hour/PoR em bruto quanto os suavizados para realçar tendências sem jitter junto com os valores instantâneos.

### Execução em seco de CLI (opcional)

Antes de expor endpoints HTTP, você pode fazer uma verificação rápida do backend de armazenamento com a CLI integrada.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos imprimem resumos Norito JSON e rechaçam discrepâncias de perfil de pedaço ou resumo, o que os torna úteis para verificações de fumaça de CI antes de cabear Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de testes PoR

Os operadores agora podem reproduzir artefatos PoR emitidos pela governança de forma local antes de subirem para Torii. A CLI reutiliza a mesma rota de ingestão `sorafs-node`, fazendo com que as tarefas locais exponham exatamente os erros de validação que devolveriam a API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

O comando emite um resumo JSON (digest do manifesto, id do provedor, resumo da verificação, quantidade de demonstrações e resultado do veredicto opcional). Proporciona `--manifest-id=<hex>` para garantir que o manifesto armazenado coincida com o resumo do desafio, e `--json-out=<path>` quando você deseja arquivar o currículo com os artefatos originais como prova de auditoria. Incluir `--verdict` permite testar todo o fluxo de trabalho → teste → veredicto offline antes de ligar para a API HTTP.

Uma vez que Torii estiver ativo, você poderá recuperar os mesmos artefatos via HTTP:

```bash
curl -s http://$TORII/v2/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v2/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos os endpoints são servidos pelo trabalhador de armazenamento embebido, assim como os testes de fumaça de CLI e as sondas do gateway permanecem sincronizados.

## 2. Pin Recorrido → Buscar

1. Gere um pacote de manifestação + carga útil (por exemplo, com `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie o manifesto com codificação base64:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```O JSON da solicitação deve conter `manifest_b64` e `payload_b64`. Uma resposta exitosa retorna `manifest_id_hex` e o resumo da carga útil.
3. Recupere os dados fixados:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifica em base64 o campo `data_b64` e verifica se coincide com os bytes originais.

## 3. Simulação de recuperação após reinício

1. Faça pelo menos uma manifestação como arriba.
2. Reinicie o processo Torii (ou o nó completo).
3. Reenvie a solicitação de busca. A carga útil deve ser recuperável e o resumo deve coincidir com o valor anterior ao reinício.
4. Inspecione `GET /v2/sorafs/storage/state` para confirmar que `bytes_used` reflete as manifestações persistentes após o reinício.

## 4. Teste de reembolso por conta

1. Reduza temporariamente `torii.sorafs.storage.max_capacity_bytes` para um valor pequeno (por exemplo, o tamanho de uma única manifestação).
2. Faça uma manifestação; la solicitud debe tener exito.
3. Tente fazer uma segunda manifestação de tamanho semelhante. Torii deve solicitar a solicitação HTTP `400` e uma mensagem de erro que inclui `storage capacity exceeded`.
4. Restaure o limite de capacidade normal ao finalizar.

## 5. Sonda de museu PoR

1. Faça uma manifestação.
2. Solicite um museu PoR:

   ```bash
   curl -X POST http://$TORII/v2/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se a resposta contém `samples` com o conteúdo solicitado e se cada teste é válido contra a razão do manifesto armazenado.

## 6. Ganchos de automação

- CI / testes de fumaça podem reutilizar as provas dirigidas añadidas em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que contém `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Os painéis devem ser seguidos:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - contadores de sucesso/queda de PoR expostos via `/v2/sorafs/capacity/state`
  - intenções de publicação de liquidação via `sorafs_node_deal_publish_total{result=success|failure}`

A seguir esses exercícios garantem que o trabalhador de almacenamiento embebido possa inserir dados, sobreviver a reinícios, respeitar cotas definidas e gerar testes deterministas PoR antes que o nodo anuncie capacidade para a rede mais amplia.