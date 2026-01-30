---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1dec63b3766ba8637d91dfc74651161fc24fd1273217f0f2701d5dce88c52836
source_last_modified: "2025-11-14T04:43:20.883487+00:00"
translation_last_reviewed: 2026-01-30
---

# Обзор Norito

Norito — бинарный слой сериализации, используемый во всем Iroha: он определяет, как структуры данных кодируются в сети, сохраняются на диске и обмениваются между контрактами и хостами. Каждый crate в workspace опирается на Norito вместо `serde`, чтобы пиры на разном оборудовании производили идентичные байты.

Этот обзор суммирует ключевые части и ссылается на канонические материалы.

## Архитектура в общих чертах

- **Заголовок + payload** – Каждое сообщение Norito начинается с заголовка согласования features (flags, checksum), за которым следует голый payload. Упакованные раскладки и сжатие согласуются через биты заголовка.
- **Детерминированное кодирование** – `norito::codec::{Encode, Decode}` реализуют базовое кодирование. Тот же layout используется при оборачивании payloads в заголовки, поэтому хеширование и подпись остаются детерминированными.
- **Схема + derives** – `norito_derive` генерирует реализации `Encode`, `Decode` и `IntoSchema`. Упакованные структуры/последовательности включены по умолчанию и описаны в `norito.md`.
- **Реестр multicodec** – Идентификаторы хешей, типов ключей и описателей payload находятся в `norito::multicodec`. Авторитетная таблица поддерживается в `multicodec.md`.

## Инструменты

| Задача | Команда / API | Примечания |
| --- | --- | --- |
| Проверить заголовок/секции | `ivm_tool inspect <file>.to` | Показывает версию ABI, flags и entrypoints. |
| Кодировать/декодировать в Rust | `norito::codec::{Encode, Decode}` | Реализовано для всех основных типов data model. |
| Interop JSON | `norito::json::{to_json_pretty, from_json}` | Детерминированный JSON на основе значений Norito. |
| Генерировать docs/specs | `norito.md`, `multicodec.md` | Документация-источник истины в корне репозитория. |

## Процесс разработки

1. **Добавить derives** – Предпочитайте `#[derive(Encode, Decode, IntoSchema)]` для новых структур данных. Избегайте ручных сериализаторов, если это не абсолютно необходимо.
2. **Проверить упакованные layouts** – Используйте `cargo test -p norito` (и матрицу packed features в `scripts/run_norito_feature_matrix.sh`), чтобы убедиться, что новые layouts остаются стабильными.
3. **Перегенерировать docs** – Когда кодирование меняется, обновите `norito.md` и таблицу multicodec, затем обновите страницы портала (`/reference/norito-codec` и этот обзор).
4. **Держать тесты Norito-first** – Интеграционные тесты должны использовать JSON хелперы Norito вместо `serde_json`, чтобы проходить те же пути, что и продакшн.

## Быстрые ссылки

- Спецификация: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Назначения multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Скрипт матрицы features: `scripts/run_norito_feature_matrix.sh`
- Примеры packed layouts: `crates/norito/tests/`

Сочетайте этот обзор с руководством быстрого старта (`/norito/getting-started`) для практического прохождения компиляции и запуска байткода, использующего payloads Norito.
