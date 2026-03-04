---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: Runbook de operações do no
sidebar_label: Runbook de operações não
descrição: Valide a implantação embutida de `sorafs-node` dentro do Torii.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Mantenha ambos os versos sincronizados até que o conjunto Sphinx seja retirado.
:::

## Visão geral

Este runbook guia operadores na validação de uma implantação `sorafs-node` embutida no Torii. Cada sessão se conecta diretamente às entregas SF-3: ciclos pin/fetch, recuperação após reinício, rejeição por cota e amostragem PoR.

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

- Garanta que o processo Torii tenha acesso de leitura/escrita a `data_dir`.
- Confirme que o não anuncia a capacidade esperada via `GET /v1/sorafs/capacity/state` quando uma declaração de marca registrada.
- Quando a suavização está habilitada, os dashboards expõem contadores GiB·hour/PoR brutos e suavizados para destacar tendências sem jitter ao lado dos valores instantâneos.

### Teste de CLI (opcional)

Antes de exportar endpoints HTTP, você pode validar o backend de armazenamento com uma CLI embarcada.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos imprimem resumos Norito JSON e recusam divergências de chunk-profile ou digest, tornando-os úteis para smoke checks de CI antes da fiação do Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Ensaio de prova PoR

Operadores agora podem reproduzir artefatos PoR emitidos pela governança local antes de enviá-los ao Torii. A CLI reutiliza o mesmo caminho de ingestão `sorafs-node`, portanto execuções locais expõem exatamente os erros de validação que uma API HTTP retornaria.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

O comando emite um resumo JSON (digest do manifest, id do provedor, digest da prova, contagem de amostras e veredito opcional). Forneca `--manifest-id=<hex>` para garantir que o manifesto armazenado corresponda ao resumo do desafio, e `--json-out=<path>` quando quiser arquivar o resumo com os artefatos originais como evidência de auditoria. Incluir `--verdict` permite ensaiar o fluxo completo desafio → prova → veredito offline antes de chamar a API HTTP.

Depois que o Torii estiver ativo você pode recuperar os mesmos artefatos via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ambos os endpoints são servidos pelo trabalhador de armazenamento embutido, portanto, testes de fumaça de CLI e sondas de gateway permanecem sincronizados.

## 2. Ciclo Pin → Buscar

1. Gere um pacote de manifesto + carga útil (por exemplo, com `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Envie o manifesto com codificação base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   O JSON da requisição deve conter `manifest_b64` e `payload_b64`. Uma resposta de sucesso retorna `manifest_id_hex` e o resumo do payload.
3. Busque os dados definidos:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```Decodifique o campo `data_b64` em base64 e verifique se correspondem aos bytes originais.

## 3. Exercício de recuperação após reinício

1. Fixe pelo menos um manifesto como acima.
2. Reinicie o processo Torii (ou não inteiro).
3. Reenvie uma requisição de busca. O payload deve continuar disponível e o digest retornado deve coincidir com o valor anterior ao reinício.
4. Inspecione `GET /v1/sorafs/storage/state` para confirmar que `bytes_used` reflete os manifestos persistidos após a reinicialização.

## 4. Teste de rejeição por cota

1. Reduza temporariamente `torii.sorafs.storage.max_capacity_bytes` para um valor pequeno (por exemplo o tamanho de um manifesto único).
2. Corrigir um manifesto; uma requisição deve ter sucesso.
3. Tente fixar um segundo manifesto de tamanho semelhante. O Torii deve rejeitar uma requisição com HTTP `400` e uma mensagem de erro contendo `storage capacity exceeded`.
4. Restauração do limite de capacidade normal ao encerrar.

## 5. Sonda de amostragem PoR

1. Corrija um manifesto.
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

3. Verifique se a resposta contém `samples` com a contagem solicitada e se cada prova válida contra a raiz do manifesto armazenado.

## 6. Ganchos de automação

- Testes de CI/fumaça podem ser reutilizados como verificações específicas adicionadas em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  que cobrem `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Dashboards devem acompanhar:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  -`torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - contadores de sucesso/falha PoR expostos via `/v1/sorafs/capacity/state`
  - Tentativa de publicação de liquidação via `sorafs_node_deal_publish_total{result=success|failure}`

Seguir esses exercícios garante que o trabalhador de armazenamento embutido consiga ingerir dados, sobreviver a reinícios, cumprir cotas ajustadas e gerar testes PoR determinísticos antes de não anunciar capacidade para uma rede mais ampla.