---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-rollout-checklist
כותרת: Чеклист השקת реестра chunker SoraFS
sidebar_label: נתח השקה של Чеклист
תיאור: השקת Пошаговый план ל- обновлений реестра chunker.
---

:::note Канонический источник
Отражает `docs/source/sorafs/chunker_registry_rollout_checklist.md`. צור קשר עם ספינקס.
:::

# Чеклист השקת реестра SoraFS

Этот чеклист фиксирует шаги, необходимые для продвижения нового профиля chunker
או כניסה לספק חבילה от ревью до продакшена после ратификации
אמנת ממשל.

> **עליון:** применяется ко всем релизам, которые меняют
> `sorafs_manifest::chunker_registry`, מעטפות קבלה לספק или
> חבילות מתקנים канонические (`fixtures/sorafs_chunker/*`).

## 1. Предварительная валидация

1. Перегенерируйте fixtures и проверьте детерминизм:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Убедитесь, что hashes детерминизма в
   `docs/source/sorafs/reports/sf1_determinism.md` (אולי מידע נוסף
   профиля) совпадают с регенерированными артефактами.
3. Убедитесь, что `sorafs_manifest::chunker_registry` компилируется с
   `ensure_charter_compliance()` זמין עבורך:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Обновите dossier предложения:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Запись דקות совета в `docs/source/sorafs/council_minutes_*.md`
   - Отчет о детерминизме

## 2. אישור ממשל

1. Представьте отчет Tooling Working Group и digest предложения в
   פאנל תשתיות פרלמנט סורה.
2. Зафиксируйте детали одобрения в
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. אופטימיזציה של מעטפה, תקליטורים, מתקנים:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Проверьте, что envelope доступен через helper получения ממשל:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. השקת שלב

הדרכה Подробный см. • [ספר משחקי מניפסט בימוי](./staging-manifest-playbook).

1. Разверните Torii с включенным Discovery `torii.sorafs` ואכיפה включенным
   כניסה (`enforce_admission = true`).
2. הצג מעטפות קבלה של ספקים מאושרים בספריית רישום הבמה,
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. ראה, פרסומות של ספקים распространяются через Discovery API:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. הצג את כותרות הממשל של נקודות הקצה/התוכנית:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Убедитесь, что לוחות מחוונים של טלמטריה (`torii_sorafs_*`) וכללי התראה
   отображают новый профиль без ошибок.

## 4. השקת ייצור

1. בדוק את הבמה בפרודקשן Torii-узлах.
2. Объявите окно активации (дата/время, תקופת חסד, תוכנית החזרה לאחור) в каналы
   операторов ו-SDK.
3. Смёрджите релизный PR с:
   - Обновленными מתקנים и מעטפה
   - Документационными измениями (ссылки на charter, отчет о детерминизме)
   - אישור מפת דרכים/סטטוס
4. הצג מידע על מקורות מידע ומקור.

## 5. לאחר השקה аудит

1. Снимите финальные метрики (ספירות גילוי, שיעור הצלחה באחזור, שגיאה
   היסטוגרמות) через 24 часа после השקה.
.
3. הדרכה למעקב אחר (например, дополнительная הנחיות לכתיבה
   профилей) в `roadmap.md`.