---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nó
título: Ранбук операций узла
sidebar_label: Ранбук операций узла
description: Проверка встроенного деплоя `sorafs-node` внутри Torii.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Faça uma cópia da sincronização, mas o Sphinx não está disponível para download.
:::

##Obzor

Este é um procedimento que permite ao operador executar a operação de implementação `sorafs-node`.
Torii. Каждый раздел напрямую соответствует entregas SF-3: циклам pin/fetch,
восстановлению после перезапуска, отказу по квоте и PoR-сэмплингу.

## 1. Uso Privado

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

- Verifique se o processo Torii é fornecido para a tela/exibição para `data_dir`.
- Подтвердите, что узел объявляет ожидаемую ёмкость через
  `GET /v1/sorafs/capacity/state` contém declarações de declaração.
- При включённом сглаживании дашборды показывают как сырые, так и сглаженные
  счётчики GiB·hour/PoR, чтобы подчёркивать тренды без джиттера рядом с
  мгновенными значениями.

### Experimente a CLI (opционально)

Ao abrir o HTTP эндпоинтов можно провести sanity-check backend хранения с
Clique em CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

Os comandos usam Norito JSON-резюме e отвергают несовпадения профиля чанков или
digest, o que é feito para CI smoke-proverok antes de usar Torii.【crates/sorafs_node/tests/cli.rs#L1】

### Repetição PoR-доказательства

Os operadores podem operar localmente com artefactos PoR, governação выпущенные,
Verifique se ele está em Torii. CLI é usado para ingerir `sorafs-node`, também
Este programa local permite que você valide a versão da API HTTP.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

O comando contém JSON-резюме (digest манифеста, id provador, digest доказательства,
количество сэмплов, опциональный результат veredicto). Use `--manifest-id=<hex>`,
чтобы убедиться, что сохранённый манифест совпадает с digest вызова, и
`--json-out=<path>`, que é novo para o arquivo de configuração, com artefactos isolados
как доказательство для аудита. A instalação `--verdict` permite a remoção do polígono
Você pode clicar em → transferir → ver o arquivo off-line para executar a API HTTP.

A configuração Torii pode exigir que os artefatos sejam HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Para obter um ponto de venda específico, você pode usar o trabalhador de armazenamento, usar testes de fumaça CLI e
пробы gateway остаются синхронизированными.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Цикл Pin → Buscar

1. Organize o manifesto do pacote + carga útil (por exemplo, com base em
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Abra o manifesto no código base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON é definido como `manifest_b64` e `payload_b64`. Успешный ответ
   возвращает `manifest_id_hex` e digerir a carga útil.
3. Verifique os danos:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```Descarte o pólo `data_b64` em base64 e verifique se ele é compatível com um banco de dados isolado.

## 3. Treinar seu uso após a transferência

1. Verifique o mínimo do manifesto, conforme sua descrição.
2. Selecione o processo Torii (ou seu uso).
3. Повторно отправьте запрос fetch. Payload é adicionado a um resumo, um resumo em
   ответе должен совпасть с предшествующим перезапуску.
4. Verifique `GET /v1/sorafs/storage/state`, que é usado, que `bytes_used` funciona
   manifestos сохранённые после перезагрузки.

## 4. Teste o valor da chave

1. Verifique rapidamente `torii.sorafs.storage.max_capacity_bytes` para uma maior segurança
   (por exemplo, размера одного manifesto).
2. Manifesto Закрепите один; запрос должен пройти успешно.
3. Abra o seu manifesto através do arquivo размера. Torii должен отклонить
   Abra o HTTP `400` e solicite-o com o código `storage capacity exceeded`.
4. Para abrir a caixa de câmbio.

## 5. Teste PoR-сэмплинга

1. Manifesto Закрепите.
2. Abra o PoR-выборку:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verifique se o `samples` está no local certo com a colisão e este problema
   доказательство валидируется относительно корня сохранённого манифеста.

## 6. Хуки автоматизации

- CI / fumaça-teste pode ser usado principalmente em:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  которые покрывают `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`
  e `por_sampling_returns_verified_proofs`.
- Дашборды должны отслеживать:
  -`torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  -`torii_sorafs_storage_pin_queue_depth` e `torii_sorafs_storage_fetch_inflight`
  - счётчики успехов/неудач PoR, публикуемые через `/v1/sorafs/capacity/state`
  - liquidação pública попытки публикации через `sorafs_node_deal_publish_total{result=success|failure}`

O período de garantia é garantido, o que significa que o trabalhador de armazenamento é contratado
ингестировать данные, переживать перезапуски, соблюдать заданные квоты и генерировать
детерминированные PoR-доказательства до того, как узел объявит ёмкость широкой сети.