---
lang: ru
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Автоматизация модели угроз Data Availability (DA-1)

Пункт DA-1 дорожной карты и `status.md` требуют детерминированного цикла
автоматизации, который выпускает сводки модели угроз Norito PDP/PoTR,
опубликованные в `docs/source/da/threat_model.md` и зеркале Docusaurus. Этот
каталог фиксирует артефакты, на которые ссылаются:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (запускает `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Поток

1. **Сгенерируйте отчет**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON-сводка фиксирует симулированную долю отказов репликации, пороги chunker и
   любые нарушения политик, обнаруженные harness PDP/PoTR в
   `integration_tests/src/da/pdp_potr.rs`.
2. **Сгенерируйте таблицы Markdown**
   ```bash
   make docs-da-threat-model
   ```
   Это запускает `scripts/docs/render_da_threat_model_tables.py`, чтобы
   перезаписать `docs/source/da/threat_model.md` и `docs/portal/docs/da/threat-model.md`.
3. **Архивируйте артефакт**, скопировав JSON-отчет (и опциональный CLI-лог) в
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Когда
   решение управления зависит от конкретного запуска, добавьте хеш коммита и
   seed симулятора в соседний `<timestamp>-metadata.md`.

## Ожидания по доказательствам

- JSON-файлы должны быть <100 KiB, чтобы оставаться в git. Более крупные трассы
  должны храниться во внешнем хранилище; при необходимости укажите подписанный
  хеш в метаданных.
- Каждый архивированный файл должен содержать seed, путь конфигурации и версию
  симулятора, чтобы перезапуски были воспроизводимы.
- Ссылайтесь на архивированный файл из `status.md` или записи в roadmap при
  каждом продвижении критериев DA-1, чтобы проверяющие могли подтвердить
  бейзлайн без повторного запуска harness.

## Сверка обязательств (пропуск секвенсора)

Используйте `cargo xtask da-commitment-reconcile`, чтобы сравнить квитанции
ingest DA с записями обязательств DA и выявить пропуски или подмены секвенсора:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Принимает квитанции в Norito или JSON и обязательства из `SignedBlockWire`,
  `.norito` или JSON-бандлов.
- Завершается ошибкой, если какой-либо тикет отсутствует в журнале блоков или
  хеши расходятся; `--allow-unexpected` игнорирует тикеты, присутствующие только
  в блоках, когда набор квитанций намеренно ограничен.
- Приложите сформированный JSON к пакетам управления/Alertmanager для алертов
  о пропусках; по умолчанию `artifacts/da/commitment_reconciliation.json`.

## Аудит привилегий (квартальная проверка доступа)

Используйте `cargo xtask da-privilege-audit` для сканирования каталогов
manifest/replay DA (и дополнительных путей) на предмет отсутствующих элементов,
элементов не-директорий или world-writable прав:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Читает пути ingest DA из конфигурации Torii и проверяет Unix-права, когда они
  доступны.
- Помечает отсутствующие/не-директории/world-writable пути и возвращает
  ненулевой код выхода при проблемах.
- Подпишите и прикрепите JSON-бандл (`artifacts/da/privilege_audit.json` по
  умолчанию) к пакетам и дашбордам квартального аудита.
