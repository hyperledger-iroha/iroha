---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: авторство профиля-чанкера
title: دليل تأليف ملفات chunker في SoraFS
Sidebar_label: Добавить чанкера
описание: Встроенное устройство для чанкера и приспособления для SoraFS.
---

:::примечание
Создан на `docs/source/sorafs/chunker_profile_authoring.md`. Он был убит в фильме "Сфинкс" القديمة.
:::

# Установите флажок chunker в SoraFS.

Он был создан в 2008 году и был создан чанкером с SoraFS.
В составе RFC SF-1 (SF-1) и SF-2a.
Он сказал, что хочет, чтобы он сделал это.
للاطلاع على مثال معتمد, راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
Проведен пробный пробег.
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

В ответ на слова Уоллеса и Уилла:

- Поддержка мультихэш-контроля CDC с поддержкой мультихеширования.
- Создание обновлений и обновлений (JSON Rust/Go/TS + corpora fuzz + شهود PoR) для последующих SDK
  Узнайте больше о инструментах
- تضمين بيانات جاهزة للحوكمة (пространство имен, имя, семвер) и إرشادات الهجرة التوافق؛ Й
- Вы можете изменить разницу между значениями параметров.

Он был убит в 1980-х годах в Нью-Йорке.

## ملخص ميثاق السجل

В 1980-х годах он был убит в 1980-х годах в Лос-Анджелесе.
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Написано, что он был убит Биллом Рэйдером Джоном Уилсоном.
- Создан в المقبض المعتمد (`namespace.name@semver`) в في قائمة البدائل
  Он Хейн **الأول**. Установите флажок `sorafs.sf1@1.0.0`.
- Псевдоним Лайна Кейла, его имя - хэндл Мейстера и его сына.
- Он и Тэн, псевдоним Скарлет Уинстон в Нью-Йорке.

Интерфейс CLI:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Он выступил с речью о том, как он сказал: المعتمدة
Сделайте это.

## البيانات المطلوبة

| حقل | الوصف | Код (`sorafs.sf1@1.0.0`) |
|-------|-------|---------------------------|
| `namespace` | Он сказал, что это не так. | `sorafs` |
| `name` | تسمية مقروءة للبشر. | `sf1` |
| `semver` | Нажмите на ссылку, чтобы получить больше информации. | `1.0.0` |
| `profile_id` | Его автором стал Рэйди Уинстон. Он сказал, что хочет, чтобы он сделал это. | `1` |
| `profile_aliases` | Настоящий герой (Турнир قديمة, اختصارات) تفاوض. Он был отправлен в Австралию. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Это кусок, который можно использовать. | `65536` |
| `profile.target_size` | Это кусок, который можно было бы использовать. | `262144` |
| `profile.max_size` | Это кусок, который можно использовать. | `524288` |
| `profile.break_mask` | قناع تكيفي يستخدمه скользящий хэш (шестнадцатеричный). | `0x0000ffff` |
| `profile.polynomial` | Полином зубчатой ​​передачи (шестнадцатеричный). | `0x3da3358b4dc173` |
| `gear_seed` | Seed لاشتقاق جدول gear بحجم 64 КиБ. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Мультихеш-код обрабатывает фрагмент. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Дайджест последних событий المعتمدة. | `13fa...c482` |
| `fixtures_root` | Футбольный клуб "Нидерланды" зафиксировал матч с "Спорт-Спорт". | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Начальное значение PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (картинка) |Футбольный клуб "Спорт-Сити" в Нью-Йорке закрепил за собой السجل
Инструменты для интерфейса командной строки были созданы в ходе работы над программой. عند الشك، شغّل
Интерфейсы командной строки для хранилища фрагментов и манифеста `--json-out=-`
Будьте добры.

### Доступ к интерфейсу командной строки

- `sorafs_manifest_chunk_store --profile=<handle>` — создание фрагмента и дайджеста
  Манифест وفحوص PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — вызывает chunk-store в стандартном выводе
  للمقارنات الآلية.
- `sorafs_manifest_stub --chunker-profile=<handle>` — تأكيد أن манифестирует وخطط CAR
  تتضمن المقبض المعتمد والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة تغذية `chunk_fetch_specs` السابق للتحقق من
  смещения/дайджесты بعد التغيير.

Выполняется сбор данных (дайджесты, POR PoR, хеширование манифеста) в режиме реального времени.
Он ответил на вопрос о том, как это сделать.

## قائمة تحقق الحتمية والتحقق

1. **Светильники إعادة توليد**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Датчик тормоза** — установлен на `cargo test -p sorafs_chunker` и дифференциал жгута проводов
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) Купите новые светильники.
3. **Защита корпуса от пуха/противодавления** — `cargo fuzz list` и жгут проводов.
   (`fuzz/sorafs_chunker`).
4. **Уведомление о доказательстве возможности восстановления** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح وأكد تطابق الجذور
   Здесь представлены все необходимые приспособления.
5. **Пробный прогон через CI** — شغّل `ci/check_sorafs_fixtures.sh`. Келли и Нэнси
   Есть светильники и `manifest_signatures.json`.
6. **Обработка перекрестной среды выполнения** — можно использовать в Go/TS с JSON-файлом или файлом JSON.
   حدود chunk и дайджесты.

Обзоры дайджестов, опубликованные в журнале Tooling WG. تخمين.

### تأكيد Манифест / PoR

Календарь матчей в Сан-Франциско, Сан-Франциско, Манифест Бэнлала Лэйс-Бэнна, Сан-Франциско CAR и PoR:

```bash
# التحقق من بيانات chunk + PoR مع الملف الجديد
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# توليد manifest + CAR والتقاط chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# إعادة التشغيل باستخدام خطة fetch المحفوظة (تمنع offsets القديمة)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

استبدل ملف الإدخال بأي Corpus ممثل مستخدم في светильники الخاصة بك
(Объем потока: 1 ГиБ)

## قالب المقترح

Для этого необходимо установить Norito для `ChunkerProfileProposalV1`.
`docs/source/sorafs/proposals/`. Создание файла JSON для просмотра изображений
(на английском языке):


Добавление Markdown в (`determinism_report`) для просмотра и дайджеста фрагмента фрагмента.
Он сказал, что это не так.

## سير عمل الحوكمة

1. **Защита от PR + светильники.** ضمّن الأصول المولدة, مقترح Norito, وتحديثات
   `chunker_registry_data.rs`.
2. **Команда Tooling WG.** المقترح
   Он был отправлен в Лондон (Lا إعادة لاستخدام المعرفات, الحتمية متحققة).
3. **Обзор.** بعد الموافقة, يوقع أعضاء المجلس, дайджест
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`)
   Воспользуйтесь светильниками.
4. **Нажмите на кнопку.** Воспользуйтесь светильниками, установленными в магазине. Интерфейс CLI
   Он сказал, что это не так.
   Создайте миграционную книгу.

## نصائح التأليف- В фильме «Тренер» в фильме «Страна Стоуэлла» он разбивает куски на куски.
- Создал мультихеш-файл в формате манифеста и шлюза; Он сказал, что это не так.
- Семена и снаряжение можно найти в магазине «Лос-Анджелес-Сити» в Лос-Анджелесе.
- Отображение артефактов и пропускная способность (пропускная способность)
  `docs/source/sorafs/reports/` открыт.

Внедрение миграционного журнала
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة وقت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`.