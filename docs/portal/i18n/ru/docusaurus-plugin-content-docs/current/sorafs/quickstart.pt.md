---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Быстрый запуск SoraFS

Это практический способ выполнения или определения параметров блока SF-1,
ассинатура де манифестов и о потоке автобусов с несколькими поставщиками, которые поддерживаются или
трубопровод вооружения до SoraFS. Комбинат-о ком о
[mergulho profundo без конвейера манифестов] (manifest-pipeline.md)
для заметок о дизайне и ссылок на флаги CLI.

## Предварительные требования

- Toolchain do Rust (`rustup update`), локальное клонирование рабочей области.
- Необязательно: [пароль Ed25519, совместимый с OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  пара ассинарских манифестов.
- Необязательно: Node.js ≥ 18, если вы хотите предварительно визуализировать портал Docusaurus.

Defina `export RUST_LOG=info` на время тестирования для экспорта сообщений из CLI.

## 1. Настройте детерминированные параметры ОС.

Новые возможности для канонического разделения SF-1. О comando também emite
конверты с манифестами, которые были убиты, когда `--signing-key` é fornecido; использовать
`--allow-unsigned` не применяется локально.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Саидас:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (убрано)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Фрагмент полезной нагрузки и проверка плана

Используйте `sorafs_chunker` для фрагментации архива или произвольного компактного архива:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Кампос-Чаве:

- `profile` / `break_mask` – подтверждение параметров `sorafs.sf1@1.0.0`.
- `chunks[]` – смещает порядки, комплименты и дайджесты BLAKE3-кусков.

Для более крупных светильников выполните регрессию, чтобы гарантировать, что
фрагментация потоковой передачи и их синхронизация навсегда:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Создание манифеста

Соберите фрагменты плана, псевдонимы и ассинатуры управления в манифесте.
usando `sorafs-manifest-stub`. O comando abaixo Mostra um payload de arquivo unico; прошло
um caminho de diretório para empacotar uma arvore (CLI percorre em ordem lexicográfica).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Пересмотрите пункт `/tmp/docs.report.json`:

- `chunking.chunk_digest_sha3_256` – дайджест SHA3 смещений/комплиментов, соответствующий aos
  светильники делают чанкеры.
- `manifest.manifest_blake3` – дайджест BLAKE3 assinado без конверта в манифест.
- `chunk_fetch_specs[]` – инструкции по выполнению поручений для оркестраторов.

Когда я начну действовать быстро, чтобы совершить реальное убийство, примите аргументы
`--signing-key` и `--signer`. O comando verifica cada assinatura Ed25519 до начала работы
о конверт.

## 4. Моделирование рекуперации с несколькими поставщиками

Используйте интерфейс командной строки для извлечения данных для воспроизведения или планирования фрагментов напротив или
лучшие поставщики. Это идеальное решение для дымовых тестов CI и прототипов организаторов.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Проверки:- `payload_digest_hex` является корреспондентом по отношению к манифесту.
- `provider_reports[]` вызывает заражение успеха/повреждение.
- `chunk_retry_total` отличается от нуля и регулирует противодавление.
- Passe `--max-peers=<n>` для ограничения количества программных средств для выполнения.
  Имитируйте моделирование CI для наших главных кандидатов.
- `--retry-budget=<n>` подставьте заразный участок пробного фрагмента (3) для экспорта
  регрессионные процессы могут быть быстрыми или быстрыми.

Добавьте `--expect-payload-digest=<hex>` и `--expect-payload-len=<bytes>` для замены
быстро, когда или полезная нагрузка реконструируется, отклоняясь от манифеста.

## 5. Проксимос пассос

- **Интеграция управления** – зависть или дайджест манифеста и `manifest_signatures.json`
  Чтобы получить совет о том, что реестр контактов может быть объявлен недоступным.
- **Переговоры о регистрации** – проконсультируйтесь с [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  перед новым регистратором. Автоматически выбираются канонические идентификаторы.
  (`namespace.name@semver`) с числовыми идентификаторами.
- **Автоматизация CI** – добавление команд, выполняемых в конвейеры выпуска, для того, чтобы
  документация, приспособления и публичные манифесты, детерминированные junto com
  метададос ассинадос.