---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76bc260f4ded8db1d71077f5da60831396d42cf7b63a6998b7e4b11e523b380a
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: node-operations
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas as versoes sincronizadas ate que o conjunto Sphinx seja retirado.
:::

## Visao geral

Este runbook guia operadores na validacao de uma implantacao `sorafs-node` embutida no Torii. Cada secao se conecta diretamente aos entregaveis SF-3: ciclos pin/fetch, recuperacao apos reinicio, rejeicao por quota e amostragem PoR.

## 1. Pre-requisitos

- Habilite o worker de storage em `torii.sorafs.storage`:

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

- Garanta que o processo Torii tem acesso de leitura/escrita a `data_dir`.
- Confirme que o no anuncia a capacidade esperada via `GET /v1/sorafs/capacity/state` quando uma declaracao for registrada.
- Quando o smoothing esta habilitado, dashboards expõem contadores GiB·hour/PoR brutos e suavizados para destacar tendencias sem jitter ao lado de valores instantaneos.

### Dry run de CLI (opcional)

Antes de expor endpoints HTTP, voce pode validar o backend de storage com a CLI embarcada.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos imprimem resumos Norito JSON e recusam divergencias de chunk-profile ou digest, tornando-os uteis para smoke checks de CI antes do wiring do Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de prova PoR

Operadores agora podem reproduzir artefatos PoR emitidos pela governanca localmente antes de envia-los ao Torii. A CLI reutiliza o mesmo caminho de ingestao `sorafs-node`, portanto execucoes locais expõem exatamente os erros de validacao que a API HTTP retornaria.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

O comando emite um resumo JSON (digest do manifest, id do provedor, digest da prova, contagem de amostras e veredito opcional). Forneca `--manifest-id=<hex>` para garantir que o manifest armazenado corresponde ao digest do desafio, e `--json-out=<path>` quando quiser arquivar o resumo com os artefatos originais como evidencia de auditoria. Incluir `--verdict` permite ensaiar o fluxo completo desafio → prova → veredito offline antes de chamar a API HTTP.

Depois que o Torii estiver ativo voce pode recuperar os mesmos artefatos via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos os endpoints sao servidos pelo worker de storage embutido, portanto smoke tests de CLI e sondas de gateway permanecem sincronizados.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Ciclo Pin → Fetch

1. Gere um bundle de manifest + payload (por exemplo com `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie o manifest com codificacao base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   O JSON da requisicao deve conter `manifest_b64` e `payload_b64`. Uma resposta de sucesso retorna `manifest_id_hex` e o digest do payload.
3. Busque os dados fixados:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifique o campo `data_b64` em base64 e verifique se corresponde aos bytes originais.

## 3. Exercicio de recuperacao apos reinicio

1. Fixe pelo menos um manifest como acima.
2. Reinicie o processo Torii (ou o no inteiro).
3. Reenvie a requisicao de fetch. O payload deve continuar disponivel e o digest retornado deve coincidir com o valor anterior ao reinicio.
4. Inspecione `GET /v1/sorafs/storage/state` para confirmar que `bytes_used` reflete os manifests persistidos apos o reboot.

## 4. Teste de rejeicao por quota

1. Reduza temporariamente `torii.sorafs.storage.max_capacity_bytes` para um valor pequeno (por exemplo o tamanho de um unico manifest).
2. Fixe um manifest; a requisicao deve ter sucesso.
3. Tente fixar um segundo manifest de tamanho semelhante. O Torii deve rejeitar a requisicao com HTTP `400` e uma mensagem de erro contendo `storage capacity exceeded`.
4. Restaure o limite de capacidade normal ao finalizar.

## 5. Sonda de amostragem PoR

1. Fixe um manifest.
2. Solicite uma amostra PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se a resposta contem `samples` com a contagem solicitada e se cada prova valida contra a raiz do manifest armazenado.

## 6. Ganchos de automacao

- CI / smoke tests podem reutilizar as verificacoes direcionadas adicionadas em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cobrem `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Dashboards devem acompanhar:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - contadores de sucesso/falha PoR expostos via `/v1/sorafs/capacity/state`
  - tentativas de publicacao de settlement via `sorafs_node_deal_publish_total{result=success|failure}`

Seguir esses exercicios garante que o worker de storage embutido consiga ingerir dados, sobreviver a reinicios, respeitar quotas configuradas e gerar provas PoR deterministicas antes de o no anunciar capacidade para a rede mais ampla.
