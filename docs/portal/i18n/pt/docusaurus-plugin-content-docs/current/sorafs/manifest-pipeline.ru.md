---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Alterando SoraFS → Manipuladores de pipeline

Este material fornece início rápido e descreve o pacote completo que você precisa
Clique no manual Norito, conectado ao Pin Registry SoraFS. Texto adaptado de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
leia este documento de especificação canônica e informações do diário.

## 1. Análise de determinação

SoraFS usa o perfil SF-1 (`sorafs.sf1@1.0.0`): роллинг-хэш, вдохновленный FastCDC, с
A capacidade mínima é de 64 KiB, a capacidade de 256 KiB, a capacidade máxima de 512 KiB e a capacidade máxima
`0x0000ffff`. Perfil de registro em `sorafs_manifest::chunker_registry`.

### Ajudantes Ferrugem

- `sorafs_car::CarBuildPlan::single_file` – Выдает смещения, длины e BLAKE3-дайджесты чанков
  при подготовке метаданных CAR.
- `sorafs_car::ChunkStore` – Стримит payloads, сохраняет метаданные чанков e выводит дерево
  выборки Prova de Recuperabilidade (PoR) 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Ajudante de biblioteca, disponível para CLI.

### CLI de instrumentos

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON é uma configuração de configuração, длины e дайджесты чанков. Planeje um plano de trabalho por сборке
manuais ou instruções específicas para o orquestrador.

### Свидетели PoR

`ChunkStore` substitui `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>`,
Esses auditores podem determinar a determinação da situação. Сочетайте эти флаги с
`--por-proof-out` ou `--por-sample-out`, ele contém JSON.

## 2. Manifesto aberto

`ManifestBuilder` объединяет метаданные чанков com вложениями governança:

- Корневой CID (dag-cbor) e коммитменты CAR.
- Доказательства alias e reivindicações возможностей провайдеров.
- Подписи совета и опциональные метаданные (por exemplo, build IDs).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Qual é o seu dia:

- `payload.manifest` – Norito-кодированные байты манифеста.
- `payload.report.json` – Porta para luz/automático, включая `chunk_fetch_specs`,
  `payload_digest_hex`, дайджесты CAR e метаданные alias.
- `payload.manifest_signatures.json` – Conversor, código BLAKE3-дайджест манифеста,
  SHA3-дайджест plano чанков e отсортированные подписи Ed25519.

Use `--manifest-signatures-in`, isso fornecerá conversões de uma maneira segura
teste, e `--chunker-profile-id` ou `--chunker-profile=<handle>` para fotos
реестра.

## 3. Publicação e fixação

1. **Exercício de governança** – Faça uma manifestação de sucesso e converta uma declaração soviética, чтобы
   pin pode ser usado. Внешним аудиторам следует хранить SHA3-дайджест plano чанков рядом с
   дайджестом манифеста.
2. **Cargas úteis** – Загрузите архив CAR (e опциональный индекс CAR), указанный в
   Manifesto, no Pin Registry. Убедитесь, что манифест и CAR используют один и тот же корневой CID.
3. **Explorar telemetria** – Salvar JSON-отчет, exibir PoR e localizar métricas fetch em
   artefactos originais. Эти записи питают операторские дашборды и помогают воспроизводить
   problemas com cargas úteis maiores.

## 4. Simulação de testes de teste inúteis`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`

- `#<concurrency>` é compatível com o testador (`#4` é usado).
- `@<weight>` настраивает смещение планирования; por умолчанию 1.
- `--max-peers=<n>` ограничивает число провайдеров, запланированных на запуск, когда
  обнаружение возвращает больше кандидатов, чем нужно.
- `--expect-payload-digest` e `--expect-payload-len` são removidos por conta própria.
- `--provider-advert=name=advert.to` fornece um teste de segurança antes do uso
  na simulação.
- `--retry-budget=<n>` переопределяет число повторов на чанк (por умолчанию: 3), чтобы CI
  быстрее выявляла регрессии при тестировании отказов.

`fetch_report.json` possui métricas de agregação (`chunk_retry_total`, `provider_failure_rate`,
et. д.), подходящие для CI-ассертов и наблюдаемости.

## 5. Reestruturação e governança

Para obter o novo perfil chunker:

1. Insira o descritor em `sorafs_manifest::chunker_registry_data`.
2. Abra `docs/source/sorafs/chunker_registry.md` e insira as chaves.
3. Altere as configurações (`export_vectors`) e feche os manuais.
4. Отправьте отчет о соответствии чартеру с подписями governança.

A chave automática inclui alças canônicas (`namespace.name@semver`) e
возвращаться к числовым ID только при необходимости обратной совместимости.