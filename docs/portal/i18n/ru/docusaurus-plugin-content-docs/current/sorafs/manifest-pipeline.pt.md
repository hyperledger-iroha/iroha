---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Разбивка SoraFS → Конвейер манифестов

Это дополнение к краткому началу работы с конвейером моста и мостом, который преобразует байты.
грубые действия в манифестах Norito, соответствующие реестру контактов SoraFS. О том, как адаптироваться
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
проконсультируйтесь с этим документом для получения подробной информации о канонике и журнале изменений.

## 1. «Фацер» разбивает детерминированную форму

SoraFS USA или Perfil SF-1 (`sorafs.sf1@1.0.0`): um hash rolante inspirado no FastCDC com
там минимальный фрагмент из 64 КиБ, еще 256 КиБ, максимальный из 512 КиБ и маска из одного
`0x0000ffff`. Запись зарегистрирована под номером `sorafs_manifest::chunker_registry`.

### Помощники в Rust

- `sorafs_car::CarBuildPlan::single_file` – Выделение смещений фрагментов, комплиментов и дайджестов.
  BLAKE3 готов к работе с метаданными CAR.
- `sorafs_car::ChunkStore` – быстрая потоковая передача полезных данных, сохранение метаданных фрагментов и их производных.
  Получение доказательства возможности восстановления (PoR) размером 64 КиБ / 4 КиБ.
- `sorafs_chunker::chunk_bytes_with_digests` – Помощник библиотеки для двух интерфейсов командной строки.

### Ферменты CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

В JSON можно компенсировать порядок, комплименты и дайджесты фрагментов. Заповедник о плано ао
составлять манифесты или конкретные инструкции по получению заказов оркестратором.

### Тестемуньяс PoR

O `ChunkStore` показывает `--por-proof=<chunk>:<segment>:<leaf>` и `--por-sample=<count>` для того, что
аудиторы могут ходатайствовать о связях с детерминистическими тестами. Объединить флаги esses com
`--por-proof-out` или `--por-sample-out` для регистратора или JSON.

## 2. Эмпакотар и манифест

`ManifestBuilder` объединяет метаданные фрагментов с приложениями управления:

- CID raiz (dag-cbor) и компромиссы CAR.
- Доказательства псевдонимов и сведения о возможностях проверки.
- Assinaturas do Conselho e Metadados Opcionais (например, идентификаторы сборки).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важная информация:

- `payload.manifest` – Байты кодифицируют манифест в Norito.
- `payload.report.json` – Легальное резюме для человека/автоматизации, включая `chunk_fetch_specs`,
  `payload_digest_hex`, дайджесты CAR и метададо псевдонимов.
- `payload.manifest_signatures.json` – Конверт, содержащий или дайджест BLAKE3, или манифест, o
  переварить SHA3 с использованием блоков и ассинатур Ed25519 ordenadas.

Используйте `--manifest-signatures-in` для проверки конвертов для внешних подписей.
перед серьёзными нововведениями и `--chunker-profile-id` или `--chunker-profile=<handle>` для
исправить выбор регистратуры.

## 3. Публикация и размещение рядом

1. **Envio àgovança** – Forneça o дайджест манифеста и o конверт de assinaturas ao
   совет, что можно сделать, чтобы мы впустили его. Внешние аудиторы должны охранять или обрабатывать SHA3
   сделайте план де кусков, юнто или дайджест, сделайте манифест.
2. **Pinear payloads** – загрузка с экрана для архива CAR (и индекс CAR необязательно), ссылка отсутствует.
   манифест о реестре контактов. Гарантия, содержащаяся в манифесте и CAR, или в сообщении CID.
3. **Регистратор телеметрии** – Сохранение отношения JSON, как тестируемый PoR и quaisquer
   метрики получения наших артефактов выпуска. Информационные панели Esses registros alimentam de
   Операторы и помощники могут решить проблемы с воспроизведением больших объемов полезной нагрузки.## 4. Симуляция выборки нескольких поставщиков

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` увеличение или параллельность для проверки (`#4` acima).
- `@<weight>` настройка повестки дня; падрао é 1.
- `--max-peers=<n>` ограничение количества проверяющих программ для выполнения, когда будет выполнено
  Descoberta Retorna больше кандидатов, чем хотелось бы.
- `--expect-payload-digest` и `--expect-payload-len` защищают от скрытой коррупции.
- `--provider-advert=name=advert.to` проверено, как можно было сделать это до США
  симуляция.
- `--retry-budget=<n>` подставьте заражение экспериментального фрагмента (код: 3) для CI
  Возможна быстрая регрессия в случае возникновения неблагоприятных сценариев.

`fetch_report.json` показывает совокупные показатели (`chunk_retry_total`, `provider_failure_rate`,
и т. д.) Адекваты для подтверждения CI и наблюдения.

## 5. Настройка регистрации и управления

Ao propor novos perfis de chunker:

1. Найдите или опишите `sorafs_manifest::chunker_registry_data`.
2. Актуализировать `docs/source/sorafs/chunker_registry.md` e как уставы.
3. Перегенерируйте фикстуры (`export_vectors`) и запишите собранные манифесты.
4. Зависть от отношений согласованности хартии с ассинатурами управления.

Автоматическая установка предпочтительной обработки канонических файлов (`namespace.name@semver`) и записи идентификаторов.
numéricos apenas quando necessário pelo registro.