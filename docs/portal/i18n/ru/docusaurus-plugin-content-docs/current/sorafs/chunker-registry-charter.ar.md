---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр-чартер
title: ميثاق سجل chunker لـ SoraFS
Sidebar_label: Добавить чанкера
описание: ميثاق الحوكمة لتقديم ملفات chunker واعتمادها.
---

:::примечание
Создан на `docs/source/sorafs/chunker_registry_charter.md`. Он был убит в фильме "Сфинкс" القديمة.
:::

# Для создания чанкера на SoraFS

> ** مُصادَق عليه:** 29.10.2025, 29:00 Панель инфраструктуры парламента Сора (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). В 2007 году он сказал:
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق Блин.

Он был создан в результате использования чанкера SoraFS.
В случае [دليل تأليف ملفات chunker](./chunker-profile-authoring.md) необходимо установить الجديدة ومراجعتها
Он сказал, что он Ладан.

## النطاق

Нажмите на ссылку `sorafs_manifest::chunker_registry` для `sorafs_manifest::chunker_registry`.
Дополнительные инструменты (манифестный CLI, CLI для рекламы поставщика и SDK). Псевдоним Ника Дэниела и дескриптор التي
Код файла `chunker_registry::ensure_charter_compliance()`:

- Написано, что он сказал, что его ждет Бреннан.
- Зарегистрируйтесь в приложении `namespace.name@semver` для `profile_aliases`.
  تليه البدائل القديمة.
- Сюжет фильма "Убийца" в фильме "Улыбка"" Да, это так.

## أدوار

- **Автор(ы)** – يُعدّون المقترح, ويعيدون توليد, расписание матчей, NYC أدلة الحتمية.
- **Инструментальная рабочая группа (TWG)** قواعد السجل.
- **Совет управления (GC)** – руководитель TWG, Нью-Йорк, Нью-Йорк, США النشر/الإيقاف.
- **Команда хранения** – руководитель службы поддержки Нью-Йорка.

## سير العمل عبر دورة الحياة

1. **Вечеринка**
   - Для создания файла JSON в формате `ChunkerProfileProposalV1`.
     `docs/source/sorafs/proposals/`.
   - Доступ к CLI:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Проведите PR-конференцию и зафиксируйте матчи на стадионе «Голден Стэйт».

2. **Инструментальная оснастка (TWG)**
   - أعد تشغيل قائمة التحقق (светильники, fuzz, манифест/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من نجاح
     `ensure_charter_compliance()` в приложении.
   - Запускается в интерфейсе CLI (`--list-profiles`, `--promote-profile`, потоковая передача
     `--json-out=-`).
   - Он был убит в 2007 году в 2017 году.

3. **Вечеринка (GC)**
   - Об этом сообщил TWG в Вашингтоне.
   - وقّع дайджест المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     Воспользуйтесь светильниками.
   - Сказал Нэнси Уилсон в Маунтин-Вилле.

4. **Убийство**
   - Информационный PR в сообщении:
     - `sorafs_manifest::chunker_registry_data`.
     - Добавлено (`chunker_registry.md` в случае необходимости/отсутствия).
     - светильники وتقارير الحتمية.
   - Запустите SDK для создания и редактирования файлов.

5. **Вечеринка / الإزالة التدريجية**
   - Он был отправлен в Нью-Йоркский университет в Нью-Йорке в Нью-Йорке. (Мисс Сэнсэй)

6. **Вечеринка**
   - تتطلب الإزالة أو الإصلاحات العاجلة تصويتاً بالأغلبية من المجلس.
   - В TWG он выступил в роли режиссёра в фильме «Старый мир».

## Инструменты для обработки- `sorafs_manifest_chunk_store` и `sorafs_manifest_stub`.
  - `--list-profiles` لفحص السجل.
  - `--promote-profile=<handle>` может быть использован в качестве защитного устройства для установки.
  - `--json-out=-` запускается со стандартным стандартным интерфейсом, и его можно использовать для проверки.
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`, `provider_advert_stub`). Он был назначен президентом CI и Кейна.
  Сделайте это в ближайшее время.

## حفظ السجلات

- Зарегистрировано в приложении `docs/source/sorafs/reports/`.
- Написано в журнале "Chunker" в качестве примера.
  `docs/source/sorafs/migration_ledger.md`.
- `roadmap.md` и `status.md` были отправлены в США.

## المراجع

- دليل التأليف: [دليل تأليف ملفات chunker](./chunker-profile-authoring.md)
- Имя файла: `docs/source/sorafs/chunker_conformance.md`.
- Добавлено: [просмотр фрагмента](./chunker-registry.md)