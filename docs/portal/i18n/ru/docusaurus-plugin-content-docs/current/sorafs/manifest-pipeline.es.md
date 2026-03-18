---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Разбивка SoraFS → Конвейер манифестов

Это дополнение к началу, быстрое восстановление экстремального трубопровода и экстремальное преобразование
байты в грубом виде в манифестах Norito, добавленные к реестру контактов SoraFS. Это содержание
Адаптация [`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Проконсультируйтесь с этим документом для уточнения канонических характеристик и журнала изменений.

## 1. Фрагмент детерминированной формы

SoraFS использует Perfil SF-1 (`sorafs.sf1@1.0.0`): вдохновленный хеш-код в FastCDC с
там минимальный фрагмент из 64 КиБ, объект размером 256 КиБ, максимальный из 512 КиБ и одна маска
де корте `0x0000ffff`. Запись зарегистрирована под номером `sorafs_manifest::chunker_registry`.

### Помощники ржавчины

- `sorafs_car::CarBuildPlan::single_file` – Выделение смещений кусков, долготы и дайджестов.
  BLAKE3 временно подготавливает метаданные CAR.
- `sorafs_car::ChunkStore` – полезные нагрузки Streamea, сохранение метаданных фрагментов и получение данных из арбола.
  Муэстрео «Доказательство возможности восстановления» (PoR) размером 64 КиБ / 4 КиБ.
- `sorafs_chunker::chunk_bytes_with_digests` – Помощник библиотеки по интерфейсу командной строки.

### Herramientas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON содержит орденадные смещения, долготы и дайджесты фрагментов. Гуарда
план составления манифестов или спецификаций по выбору оркестадора.

### Тестигос PoR

`ChunkStore` демонстрирует `--por-proof=<chunk>:<segment>:<leaf>` и `--por-sample=<count>` для того, что
лос-аудиторы могут ходатайствовать о соглашении с детерминистами. Комбинация флагов esos
`--por-proof-out` или `--por-sample-out` для регистрации JSON.

## 2. Охватить и проявить

`ManifestBuilder` объединяет метаданные фрагментов с адъюнктами управления:

- CID raíz (dag-cbor) и компрометация CAR.
- Пруэбас де псевдонимов и претензий на полномочия доверенных лиц.
- Firmas del consejo y Metadatos optionales (стр. ej., IDs de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Важное замечание:

- `payload.manifest` – Байты манифеста, закодированные в Norito.
- `payload.report.json` – разборчивое резюме для людей/автоматизации, включая
  `chunk_fetch_specs`, `payload_digest_hex`, обрабатывает CAR и метаданные псевдонима.
- `payload.manifest_signatures.json` – Что содержит дайджест BLAKE3 манифеста,
  переварить SHA3 плана блоков и фирм Ed25519 ordenadas.

США `--manifest-signatures-in` для проверки пропорциональности внешних фирм
перед тем, как написать, и `--chunker-profile-id` или `--chunker-profile=<handle>` для
выберите вариант регистрации.

## 3. Публикация и сосна1. **Отправка губернатору** – Пропорция дайджеста манифеста и того, что есть у фирмы
   Совет, чтобы булавка могла быть допущена. Los Audires externos deben almacenar el
   дайджест SHA3 плана фрагментов вместе с дайджестом манифеста.
2. **Pinear payloads** – Отправьте архив CAR (и индекс CAR необязательно) по ссылке
   отображает реестр контактов. Убедитесь, что он манифестирует и АВТОМОБИЛЬ сопоставляется с миссо CID.
3. **Регистратор телеметрии** – сохранение отчетов в формате JSON, результатов PoR и других показателей.
   получить и выпустить артефакты. Мы регистрируем питание информационных панелей
   Операции и помощь в воспроизведении инцидентов без большой загрузки полезной нагрузки.

## 4. Симуляция выборки с несколькими поставщиками

`пробег груза -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` увеличивает параллелизм для проверки (`#4` доступно).
- `@<weight>` отрегулируйте это планирование; по дефекту это 1.
- `--max-peers=<n>` ограничение количества программных средств для выброса, когда он находится
  описание производит больше кандидатов на желаемое.
- `--expect-payload-digest` и `--expect-payload-len` защита от тихой коррупции.
- `--provider-advert=name=advert.to` проверка возможностей проверки перед использованием
  в симуляции.
- `--retry-budget=<n>` повторно замените восстановленный фрагмент (по дефекту: 3) для того, чтобы
  CI может привести к более быстрому возврату к проверочным сценариям падения.

`fetch_report.json` совокупные метрики (`chunk_retry_total`, `provider_failure_rate`,
и т. д.) adecuadas para aserciones de CI y observabilidad.

## 5. Актуализация реестра и управления

Al proponer nuevos perfiles de chunker:

1. Отредактируйте дескриптор `sorafs_manifest::chunker_registry_data`.
2. Актуализация `docs/source/sorafs/chunker_registry.md` и соответствие уставу.
3. Приспособления Regenera (`export_vectors`) и фирменные захваты.
4. Отправка информации о накопленном уставе губернаторских фирм.

Автоматизация должна предпочитать обрабатывать canonicos (`namespace.name@semver`) и повторять идентификаторы
только числовые значения, когда это необходимо для регистрации.