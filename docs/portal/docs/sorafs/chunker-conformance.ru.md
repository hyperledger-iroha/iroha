<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cbe8d76136bbd69cef52db1332b4d1ab062268bd93b9bb644d9043895b0f6ed8
source_last_modified: "2025-12-19T22:34:31.968869+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: chunker-conformance
title: Руководство по соответствию chunker SoraFS
sidebar_label: Соответствие chunker
description: Требования и workflow для сохранения детерминированного профиля chunker SF1 в fixtures и SDKs.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_conformance.md`. Держите обе версии синхронизированными, пока legacy docs не будут выведены из эксплуатации.
:::

Этот документ фиксирует требования, которым должна следовать каждая реализация, чтобы
оставаться совместимой с детерминированным профилем chunker SoraFS (SF1). Он также
документирует workflow регенерации, политику подписей и шаги верификации, чтобы
потребители fixtures в SDKs оставались синхронизированными.

## Канонический профиль

- Handle профиля: `sorafs.sf1@1.0.0` (legacy alias `sorafs.sf1@1.0.0`)
- Входной seed (hex): `0000000000dec0ded`
- Целевой размер: 262144 bytes (256 KiB)
- Минимальный размер: 65536 bytes (64 KiB)
- Максимальный размер: 524288 bytes (512 KiB)
- Rolling polynomial: `0x3DA3358B4DC173`
- Seed таблицы gear: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Эталонная реализация: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Любое SIMD-ускорение должно выдавать идентичные границы и digests.

## Набор fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` регенерирует
fixtures и выпускает следующие файлы в `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — канонические границы чанков для
  потребителей Rust, TypeScript и Go. Каждый файл объявляет канонический handle как
  первую запись в `profile_aliases`, затем идут legacy alias (например,
  `sorafs.sf1@1.0.0`, затем `sorafs.sf1@1.0.0`). Порядок фиксируется
  `ensure_charter_compliance` и НЕ ДОЛЖЕН изменяться.
- `manifest_blake3.json` — BLAKE3-верифицированный manifest, покрывающий каждый файл fixtures.
- `manifest_signatures.json` — подписи совета (Ed25519) поверх digest манифеста.
- `sf1_profile_v1_backpressure.json` и сырые corpora в `fuzz/` —
  детерминированные streaming сценарии, используемые в тестах back-pressure chunker.

### Политика подписей

Регенерация fixtures **должна** включать валидную подпись совета. Генератор
отклоняет неподписанный вывод, если явно не передан `--allow-unsigned` (предназначен
только для локальных экспериментов). Конверты подписей append-only и
дедуплицируются по подписанту.

Чтобы добавить подпись совета:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Верификация

CI helper `ci/check_sorafs_fixtures.sh` повторно запускает генератор с
`--locked`. Если fixtures расходятся или отсутствуют подписи, job падает. Используйте
этот скрипт в nightly workflows и перед отправкой изменений fixtures.

Шаги ручной проверки:

1. Запустите `cargo test -p sorafs_chunker`.
2. Выполните `ci/check_sorafs_fixtures.sh` локально.
3. Убедитесь, что `git status -- fixtures/sorafs_chunker` чист.

## Плейбук обновления

При предложении нового профиля chunker или обновлении SF1:

См. также: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) для
требований к метаданным, шаблонов предложений и чеклистов валидации.

1. Подготовьте `ChunkProfileUpgradeProposalV1` (см. RFC SF-1) с новыми параметрами.
2. Регенерируйте fixtures через `export_vectors` и зафиксируйте новый digest манифеста.
3. Подпишите манифест требуемым кворумом совета. Все подписи должны быть
   добавлены в `manifest_signatures.json`.
4. Обновите затронутые SDK fixtures (Rust/Go/TS) и обеспечьте паритет между runtime.
5. Регенерируйте fuzz corpora при изменении параметров.
6. Обновите это руководство новым handle профиля, seeds и digest.
7. Отправьте изменения вместе с обновленными тестами и roadmap-обновлениями.

Изменения, которые затрагивают границы чанков или digests без соблюдения этого процесса,
недействительны и не должны быть смержены.
