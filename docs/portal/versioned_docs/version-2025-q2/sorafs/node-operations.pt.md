---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

:::nota Fonte Canônica
Espelhos `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambas as cópias alinhadas entre os lançamentos.
:::

## Visão geral

Este runbook orienta os operadores na validação de uma implantação `sorafs-node` incorporada dentro de Torii. Cada seção é mapeada diretamente para os resultados do SF-3: viagens de ida e volta de pin/fetch, recuperação de reinicialização, rejeição de cota e amostragem PoR.

## 1. Pré-requisitos

- Habilite o trabalhador de armazenamento em `torii.sorafs.storage`:

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

- Certifique-se de que o processo Torii tenha acesso de leitura/gravação a `data_dir`.
- Confirme se o nó anuncia a capacidade esperada via `GET /v1/sorafs/capacity/state` assim que uma declaração for registrada.
- Quando a suavização está habilitada, os painéis expõem os contadores GiB·hora/PoR brutos e suavizados para destacar tendências sem jitter junto com valores pontuais.

### Teste de CLI (opcional)

Antes de expor os endpoints HTTP, você pode verificar a integridade do back-end de armazenamento com o CLI incluído.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos imprimem resumos JSON Norito e recusam incompatibilidades de perfil de bloco ou resumo, tornando-os úteis para verificações de fumaça de CI antes da fiação Torii.【crates/sorafs_node/tests/cli.rs#L1】

Assim que Torii estiver ativo, você poderá recuperar os mesmos artefatos via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos os endpoints são atendidos pelo trabalhador de armazenamento incorporado, portanto, os testes de fumaça CLI e as sondagens de gateway permanecem sincronizados.

## 2. Fixar → Buscar ida e volta

1. Produza um pacote de manifesto + carga útil (por exemplo, com `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie o manifesto com codificação base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   O JSON da solicitação deve conter `manifest_b64` e `payload_b64`. Uma resposta bem-sucedida retorna `manifest_id_hex` e o resumo da carga útil.
3. Obtenha os dados fixados:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifique em Base64 o campo `data_b64` e verifique se ele corresponde aos bytes originais.

## 3. Reinicie o exercício de recuperação

1. Fixe pelo menos um manifesto conforme acima.
2. Reinicie o processo Torii (ou o nó inteiro).
3. Reenvie a solicitação de busca. A carga útil ainda deve ser recuperável e o resumo retornado deve corresponder ao valor pré-reinicialização.
4. Inspecione `GET /v1/sorafs/storage/state` para confirmar que `bytes_used` reflete os manifestos persistentes após a reinicialização.

## 4. Teste de rejeição de cota

1. Reduza temporariamente `torii.sorafs.storage.max_capacity_bytes` para um valor pequeno (por exemplo, o tamanho de um único manifesto).
2. Fixe um manifesto; a solicitação deve ser bem-sucedida.
3. Tente fixar um segundo manifesto de tamanho semelhante. Torii deve rejeitar a solicitação com HTTP `400` e uma mensagem de erro contendo `storage capacity exceeded`.
4. Restaure o limite de capacidade normal quando terminar.

## 5. Sonda de amostragem PoR

1. Fixe um manifesto.
2. Solicite uma amostra de PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se a resposta contém `samples` com a contagem solicitada e se cada prova é validada na raiz do manifesto armazenada.

## 6. Ganchos de automação

- Os testes de CI/fumaça podem reutilizar as verificações direcionadas adicionadas em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```que abrange `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Os painéis devem rastrear:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  -`torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - Contadores de sucesso/falha PoR surgiram via `/v1/sorafs/capacity/state`
  - Tentativas de publicação de liquidação via `sorafs_node_deal_publish_total{result=success|failure}`

Seguir esses exercícios garante que o trabalhador de armazenamento incorporado possa ingerir dados, sobreviver a reinicializações, respeitar cotas configuradas e gerar provas determinísticas de PoR antes que o nó anuncie capacidade para a rede mais ampla.