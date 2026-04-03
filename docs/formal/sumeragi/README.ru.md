<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Формальная модель (TLA+ / Apalache)

Этот каталог содержит ограниченную формальную модель безопасности и жизнеспособности пути фиксации Sumeragi.

## Область применения

Модель фиксирует:
- прогрессия фаз (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- пороги голосования и кворума (`CommitQuorum`, `ViewQuorum`),
- кворум взвешенной доли (`StakeQuorum`) для защиты фиксации в стиле NPoS,
- Причинность эритроцитов (`Init -> Chunk -> Ready -> Deliver`) с доказательствами заголовка/дайджеста,
- GST и слабые предположения о справедливости в отношении честных действий по прогрессу.

Он намеренно абстрагирует форматы проводов, сигнатуры и полную информацию о сети.

## Файлы

- `Sumeragi.tla`: модель и свойства протокола.
- `Sumeragi_fast.cfg`: меньший набор параметров, совместимый с CI.
- `Sumeragi_deep.cfg`: больший набор параметров напряжения.

## Свойства

Инварианты:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Временное свойство:
- `EventuallyCommit` (`[] (gst => <> committed)`), с кодировкой справедливости после GST
  в рабочем режиме в `Next` (тайм-аут/защита от сбоев включена)
  прогрессивные действия). Это позволяет проверить модель с помощью Apalache 0.52.x, который
  не поддерживает операторы справедливости `WF_` внутри проверенных временных свойств.

## Бег

Из корня репозитория:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Воспроизводимая локальная настройка (Docker не требуется)Установите закрепленную локальную цепочку инструментов Apalache, используемую в этом репозитории:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Раннер автоматически обнаруживает эту установку по адресу:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
После установки `ci/check_sumeragi_formal.sh` должен работать без дополнительных переменных окружения:

```bash
bash ci/check_sumeragi_formal.sh
```

Если Apalache нет в `PATH`, вы можете:

- установите `APALACHE_BIN` в путь к исполняемому файлу или
- использовать резервный вариант Docker (включен по умолчанию, когда доступен `docker`):
  - изображение: `APALACHE_DOCKER_IMAGE` (по умолчанию `ghcr.io/apalache-mc/apalache:latest`)
  - требуется работающий демон Docker
  - отключить резервный вариант с помощью `APALACHE_ALLOW_DOCKER=0`.

Примеры:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Примечания

- Эта модель дополняет (не заменяет) исполняемые тесты модели Rust в
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  и
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Проверки ограничены постоянными значениями в файлах `.cfg`.
- PR CI запускает эти проверки в `.github/workflows/pr.yml` через
  `ci/check_sumeragi_formal.sh`.