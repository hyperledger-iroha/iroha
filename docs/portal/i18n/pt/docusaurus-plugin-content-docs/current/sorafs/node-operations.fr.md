---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: Runbook d’exploitation du noeud
sidebar_label: Runbook d’exploitation du noeud
description: Valide a implantação embarcada de `sorafs-node` em Torii.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Gardez as duas versões sincronizadas até o conjunto Sphinx ser retirado.
:::

## Vista do conjunto

Este runbook guia os operadores para validar uma implantação `sorafs-node` embarcada em Torii. Cada seção corresponde diretamente aos livrables SF-3: boucles pin/fetch, reprise après redémarrage, rejet de quotas et échantillonnage PoR.

## 1. Pré-requisitos

- Ative o trabalhador de armazenamento em `torii.sorafs.storage`:

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

- Certifique-se de que o processo Torii tenha acesso a uma leitura/escrita em `data_dir`.
- Confirme se o novo aviso de capacidade foi atendido via `GET /v1/sorafs/capacity/state` após a declaração registrada.
- Quando a lissagem está ativa, os painéis expõem à medida que os números de GiB·hour/PoR brutos e lissés são mostrados em evidência de tendências sem jitter ao nível dos valores instantâneos.

### Execução em branco da CLI (opcional)

Antes de expor os endpoints HTTP, você pode validar o backend de armazenamento com a instalação CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos imprimem currículos Norito JSON e recusam divergências de perfil de pedaço ou resumo, o que os torna úteis para verificações de fumaça CI antes da fiação de Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Répetição de teste PoR

Os operadores podem perder a localização dos artefatos PoR émis para o governo antes do televisor ver Torii. A CLI utiliza o mesmo caminho de ingestão `sorafs-node`, de modo que as execuções locais expõem exatamente os erros de validação que a API HTTP gera.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

O comando contém um currículo JSON (digest de manifesto, id fournisseur, digest de preuve, nombre d’échantillons, verdict optionnel). Forneça `--manifest-id=<hex>` para garantir que o manifesto armazenado corresponda ao resumo do desafio, e `--json-out=<path>` se você deseja arquivar o currículo com os artefatos de origem como pré-auditoria. Inclua `--verdict` para reproduzir o conjunto do ciclo desafio → prova → veredicto local antes de solicitar a API HTTP.

Uma vez Torii on-line, você pode recuperar os mesmos artefatos via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Os dois endpoints são servidos pelo trabalhador de armazenamento embarcado, após os testes de fumaça CLI e as sondas de gateway permanecerem alinhadas.

## 2. Boucle Pin → Buscar

1. Produza um manifesto de pacote + carga útil (por exemplo, via `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Insira o manifesto em base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```O JSON de solicitação deve conter `manifest_b64` e `payload_b64`. Uma resposta rápida foi enviada `manifest_id_hex` e o resumo da carga útil.
3. Recuperar dados gravados:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Decodifique em base64 o campo `data_b64` e verifique se ele corresponde aos octetos originais.

## 3. Exercício de reprise após redemarrage

1. Envie pelo menos um manifesto como ci-dessus.
2. Reinicie o processo Torii (ou o nœud completo).
3. Reenvoyez la requête fetch. A carga útil deve ser recuperada e o resumo reenviado corresponde ao valor do reencassamento.
4. Inspecione `GET /v1/sorafs/storage/state` para confirmar se `bytes_used` reflete os manifestos persistentes após a reinicialização.

## 4. Teste de rejeição de cota

1. Abaissez temporariamente `torii.sorafs.storage.max_capacity_bytes` para um valor faível (por exemplo, la taille d’un seul manifest).
2. Épinglez un manifest ; la requête doit réussir.
3. Tentez d'épingler um segundo manifesto semelhante. Torii deve rejeitar a solicitação com HTTP `400` e uma mensagem de erro contendo `storage capacity exceeded`.
4. Restaure o limite de capacidade normal após o teste.

## 5. Sondage d'échantillonnage PoR

1. Envie um manifesto.
2. Exija um encantamento PoR:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se a resposta contém `samples` com o nome solicitado e se tudo o que for válido é válido na faixa do manifesto armazenado.

## 6. Ganchos de automação

- Os testes CI / smoke podem reutilizar os controles ciblés adicionados em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  aqui cobre `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` e `por_sampling_returns_verified_proofs`.
- Os painéis devem seguir:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - les compteurs de sucesso/échec PoR expostos via `/v1/sorafs/capacity/state`
  - as tentativas de publicação do acordo via `sorafs_node_deal_publish_total{result=success|failure}`

Seguir essas instruções garante que o trabalhador de estoque embarcado possa permanecer com dinheiro, sobreviver a re-marragens, respeitar as cotas configuradas e gerar as precauções que serão determinadas antes que o novo não anuncie sua capacidade no descanso da rede.