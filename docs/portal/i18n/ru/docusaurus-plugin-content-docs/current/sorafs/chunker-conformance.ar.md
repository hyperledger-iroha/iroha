---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: соответствие чанкеру
title: دليل مطابقة chunker في SoraFS
Sidebar_label: чанкер
описание: Создан для использования с чанкером для SF1, оснащенным приспособлениями и SDK.
---

:::примечание
Был установлен `docs/source/sorafs/chunker_conformance.md`. Он был убит в 1980-х годах в Нью-Йорке.
:::

Он сказал, что в фильме "Улыбка" и "Тренер" в Лос-Анджелесе. Используется чанкером для SoraFS (SF1).
Он Сон Сэнсэй в Нью-Йорке и в Нью-Йорке, а также в Нью-Йорке. Светильники عبر SDKs متزامنين.

## الملف المعتمد

- Код: `sorafs.sf1@1.0.0` (независимо от `sorafs.sf1@1.0.0`)
- بذرة الإدخال (шестнадцатеричный): `0000000000dec0ded`
- Размер файла: 262144 байт (256 КиБ)
- Размер файла: 65536 байт (64 КиБ)
- Размер файла: 524288 байт (512 КиБ)
- Имя пользователя: `0x3DA3358B4DC173`.
- Дополнительная передача: `sorafs-v1-gear`.
- Код запроса: `0x0000FFFF`

Сообщение: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Он был создан для SIMD и других дайджестов.

## حزمة светильники

`cargo run --locked -p sorafs_chunker --bin export_vectors` يعيد توليد
Светильники в каталоге `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — позволяет использовать чанк для работы с Rust, TypeScript и Go.
  Он был создан в 2008 году в `profile_aliases`, в 1999 году. (مثل
  `sorafs.sf1@1.0.0` или `sorafs.sf1@1.0.0`). В الترتيب بواسطة
  `ensure_charter_compliance` в Уфе.
- `manifest_blake3.json` — манифест تم التحقق منه عبر BLAKE3, посвященный приборам.
- `manifest_signatures.json` — добавление (Ed25519) для дайджеста манифеста.
- `sf1_profile_v1_backpressure.json` в корпусе `fuzz/` —
  Сэнсэйд Трэйд-Ди-Си использует противодавление в чанкере.

### سياسة التوقيع

Футболист и Сан-Диего Сан-Франциско зафиксировали матчи с Торонто Сейном на турнире. يرفض المولد
الإخراج غير الموقّع لم يتم تمرير `--allow-unsigned` صراحة (مخصص)
للتجارب المحلية فقط). Вы можете использовать функцию «только для добавления» в режиме «только для добавления».

Ответ на вопрос:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## تحقق

Дополнительный код CI `ci/check_sorafs_fixtures.sh`.
`--locked`. Он провел матчи с Сан-Франциско и Сан-Франциско. استخدم
Отслеживайте рабочие процессы и следите за светильниками.

Ответ на вопрос:

1. Код `cargo test -p sorafs_chunker`.
2. Код `ci/check_sorafs_fixtures.sh`.
3. Установите флажок `git status -- fixtures/sorafs_chunker`.

## دليل الترقية

В ответ на Чанкера Дэвиса и SF1:

Сообщение: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Он хочет, чтобы он был готов к работе.

1. Установлен `ChunkProfileUpgradeProposalV1` (RFC SF-1).
2. Установите светильники `export_vectors` и просмотрите дайджест манифеста.
3. Продемонстрируйте исчезновение лишней воды. Он был установлен в بـ `manifest_signatures.json`.
4. Добавлены исправления с использованием SDK (Rust/Go/TS) и встроенного программного обеспечения.
5. أعد توليد corpora fuzz إذا تغيرت المعلمات.
6. حدّث هدليل بالمقبض الجديد للملف والبذور وdigest.
7. Дорожная карта разработки и развития проекта.

Вы можете получить кусок фрагмента и дайджесты, которые вы хотите получить.
Он сказал, что у него есть возможность.