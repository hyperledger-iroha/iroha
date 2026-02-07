---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-conformance
כותרת: Руководство по соответствию chunker SoraFS
sidebar_label: Соответствие chunker
תיאור: Требования и זרימת עבודה עבור сохранения детерминированного профиля chunker SF1 в fixtures и SDKs.
---

:::note Канонический источник
:::

Этот документ фиксирует требования, кототорым должна следовать каждая реализация, чтобы
оставаться совместимой с детерминированным профилем chunker SoraFS (SF1). Он также
תקשורת זרימת עבודה регенерации, политику подписей и шаги верификации, чтобы
מתקני потребители ב-SDKs оставались синхронизированными.

## Канонический профиль

- זרע Входной (hex): `0000000000dec0ded`
- רזמר: 262144 בתים (256 KiB)
- Минимальный размер: 65536 בתים (64 KiB)
- Максимальный размер: 524288 בייטים (512 KiB)
- פולינום מתגלגל: `0x3DA3358B4DC173`
- ציוד זרע таблицы: `sorafs-v1-gear`
- מסיכת שבירה: `0x0000FFFF`

ריאליטי איטליה: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Любое SIMD-ускорение должно выдавать идентичные границы и digests.

## מכשירי Набор

`cargo run --locked -p sorafs_chunker --bin export_vectors` регенерирует
מתקנים и выпускает следующие файлы в `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — канонические границы чанков для
  потребителей Rust, TypeScript ו-Go. Каждый файл объявляет канонический ידית как
  `sorafs.sf1@1.0.0`, סאטם `sorafs.sf1@1.0.0`). Порядок фиксируется
  `ensure_charter_compliance` и НЕ ДОЛЖЕН изменяться.
- `manifest_blake3.json` — BLAKE3-верифицированный מניפסט, покрывающий каждый файл.
- `manifest_signatures.json` — подписи совета (Ed25519) поверх digest манифеста.
- `sf1_profile_v1_backpressure.json` וקורפוסים в `fuzz/` —
  זרמי סטרימינג של תקשורים, קטעי לחץ חוזר.

### Политика подписей

משחקי Регенерация **должна** включать валидную подпись совета. Генератор
отклоняет неподписанный вывод, если явно не передан `--allow-unsigned` (предназначен
только для локальных экспериментов). Конверты подписей הוספה בלבד и
дедуплицируются по подписанту.

Чтобы добавить подпись совета:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Верификация

CI helper `ci/check_sorafs_fixtures.sh` повторно запускает генератор с
`--locked`. Если fixtures расходятся или отсутствуют подписи, עבודה падает. Используйте
этот скрипт в זרימות עבודה ליליות и перед отправкой изменений גופי.

Шаги ручной проверки:

1. Запустите `cargo test -p sorafs_chunker`.
2. Выполните `ci/check_sorafs_fixtures.sh` локально.
3. Убедитесь, что `git status -- fixtures/sorafs_chunker` чист.

## Плейбук обновления

При предложении нового профиля chunker или обновлении SF1:

См. אמצעים: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) עבור
требований к метаданным, шаблонов предложений и чеклистов валидации.1. Подготовьте `ChunkProfileUpgradeProposalV1` (см. RFC SF-1) с новыми параметрами.
2. Регенерируйте fixtures через `export_vectors` и зафиксируйте новый digest манифеста.
3. Подпишите манифест требуемым кворумом совета. Все подписи должны быть
   добавлены в `manifest_signatures.json`.
4. התקן את רכיבי ה-SDK (Rust/Go/TS) וזמן ריצה אופטימלי.
5. Регенерируйте fuzz corpora при изменении параметров.
6. Обновите это руководство новым ידית профиля, זרעים и לעכל.
7. Отправьте изменения вместе с обновленными тестами и roadmap-обновлениями.

Измения, которые затрагивают границы чанков или מעכל ללא שיתוף פעולה,
недействительны и не должны быть смержены.