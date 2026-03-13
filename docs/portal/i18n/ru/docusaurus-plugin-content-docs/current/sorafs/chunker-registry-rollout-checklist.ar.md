---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-registry-rollout-checklist
Название: قائمة تحقق لإطلاق سجل chunker لسوراFS
Sidebar_label: Добавление фрагмента
описание: خطة إطلاق خطوة بخطوة لتحديثات سجل chunker.
---

:::примечание
تعكس `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Он был убит в фильме "Сфинкс" القديمة.
:::

# قائمة تحقق لإطلاق سجل SoraFS

Вы можете использовать его для создания куска чанкера, а также для того, чтобы сделать это.
В 2007 году он был отправлен в 2007 году.

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` أو أظرف قبول المزوّدين и светильники
> Ошибка (`fixtures/sorafs_chunker/*`).

## 1. تحقق ما قبل الإطلاق

1. Результаты матчей, запланированных на матч:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Поздравление с Днем Рождения
   `docs/source/sorafs/reports/sf1_determinism.md` (أو تقرير الملف المعني)
   Он был отправлен в Данию.
3. Установите флажок `sorafs_manifest::chunker_registry`.
   `ensure_charter_compliance()` Описание:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Ответ на вопрос:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Дополнительный файл `docs/source/sorafs/council_minutes_*.md`.
   - تقرير الحتمية

## 2. اعتماد الحوكمة

1. Создана Рабочая группа по инструментам и дайджест الاقتراح إلى Панель по инфраструктуре парламента Сора.
2. Скалли Уилсон в США
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Матчи матча "Спорт-Сити" с "Баварией":
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Сообщение о том, как сделать это в режиме реального времени:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Хорошая постановка

ارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل.

1. Torii для открытия файла `torii.sorafs` для обеспечения соблюдения требований.
   (`enforce_admission = true`).
2).
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. Открытие, сделанное в 1990-х годах в Колумбийском университете:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Создание манифеста/плана для проверки:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Установите флажок для датчика (`torii_sorafs_*`) и установите флажок
   Дэн Азар.

## 4. إطلاق الإنتاج

1. Создайте промежуточную версию Torii.
2. أعلن نافذة التفعيل (التاريخ/الوقت، فترة السماح, خطة التراجع) Используйте SDK.
3. Информационный пиар в социальных сетях:
   - Светильники
   - تغييرات الوثائق (مراجع الميثاق, تقرير الحتمية)
   - Дорожная карта/статус تحديث
4. Проверьте происхождение товара.

## 5. تدقيق ما بعد الإطلاق

1. التقط المقاييس النهائية (ранее открытие, معدل نجاح fetch, هيستوغرامات
   الأخطاء) 24 числа в день.
2. Установите `status.md` для получения дополнительной информации.
3. Зарегистрируйтесь в приложении (в разделе "Управление файлами") для `roadmap.md`.