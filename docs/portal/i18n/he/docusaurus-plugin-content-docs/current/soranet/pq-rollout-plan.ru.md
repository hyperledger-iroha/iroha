---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-rollout-plan
כותרת: Плейбук постквантового השקת SNNet-16G
sidebar_label: План השקת PQ
תיאור: Операционный гид по продвижению гибридного X25519+ML-KEM לחיצת יד SoraNet от canary לברירת מחדל בממסרים, לקוחות ו-SDKs.
---

:::note Канонический источник
:::

SNNet-16G זמין לאחר השקה קוונטית לטראנספורטה של SoraNet. ידיות `rollout_phase` позволяют операторам координировать детерминированный קידום מכירות от текущего שלב א דרישת שמירה על כיסוי שלב ב' רוב פוסט C и שלב Q редактирования сырого JSON/TOML עבור каждой поверхности.

ספר המשחקים של Этот покрывает:

- Определения phase и новые כפתורי תצורה (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) בבסיס קוד (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- דגלים של SDK ו-CLI, чтобы каждый client мог отслеживать הושקה.
- Ожидания תזמון קנרי ללוחות מחוונים של ממסר/לקוח וממשל, קידום שערים (`dashboards/grafana/soranet_pq_ratchet.json`).
- ווים לאחור и ссылки на ספר הפעלה של תרגילי כיבוי ([פנקס ריצה של PQ](./pq-ratchet-runbook.md)).

## Карта фаз

| `rollout_phase` | שלב האנונימיות של Эффективная | Эффект по умолчанию | Типичное использование |
|----------------|------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (שלב א') | Требовать как минимум один PQ guard במעגל, пока флот прогревается. | Baseline и ранние недели קנרית. |
| `ramp` | `anon-majority-pq` (שלב ב') | Смещать выбор в сторону ממסרי PQ לכיסוי >= двух третей; классические ממסרים остаются fallback. | Региональные כנריות ממסר; מחליפים את התצוגה המקדימה של SDK. |
| `default` | `anon-strict-pq` (שלב ג') | התקן את מעגלי ה-PQ בלבד והפעל אזעקות לשדרוג לאחור. | קידום מכירות Финальный после завершения טלמטריה וממשל חתימה. |

Если поверхность также задает явный `anonymity_policy`, он переопределяет שלב עבור этого компонента. Отсутствие явного שלב теперь defer-ится к значению `rollout_phase`, чтобы операторы могли переключить phase онадин разудис לקוח унаследовать его.

## הפניה конфигурации

### תזמורת (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

מתזמר מטעין решает fallback stage во время выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и отображает его через `sorafs_orchestrator_policy_events_total` ו ```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```. См. `docs/examples/sorafs_rollout_stage_b.toml` ו-`docs/examples/sorafs_rollout_stage_c.toml` לקטעי קוד.

### לקוח חלודה / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь сохраняет разобранную phase (`crates/iroha/src/client.rs:2315`), чтобы helper команды (например `iroha_cli app sorafs fetch`) мог текущую phase вместе с ברירת המחדל של מדיניות אנונימיות.

## אוטומציה

Два `cargo xtask` helper автоматизируют генерацию расписания и לכידת חפצי אמנות.

1. **לוח זמנים של Сгенерировать региональный**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   משכי זמן принимают суффиксы `s`, `m`, `h` או `d`. Команда выпускает `artifacts/soranet_pq_rollout_plan.json` וסיכום Markdown (`artifacts/soranet_pq_rollout_plan.md`), который можно приложить к בקשת שינוי.

2. **לכידת חפצי תרגיל с подписями**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```Команда копирует указанные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет BLAKE3 digests для каждого artefact и пишет ```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```0000 подписью поверх מטען. Используйте тот же מפתח פרטי, которым подписаны דקות תרגיל אש, чтобы ממשל могла быстро валидировать לכידת.

## Матрица флагов SDK & CLI

| משטח | קנרי (שלב א') | רמפה (שלב ב') | ברירת מחדל (שלב C) |
|--------|----------------|----------------|------------------------|
| `sorafs_cli` אחזור | `--anonymity-policy stage-a` или опираться на שלב | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| תצורת התזמורת JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| תצורת לקוח חלודה (`iroha.toml`) | `rollout_phase = "canary"` (ברירת מחדל) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` פקודות חתומות | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, אופציונלי `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, אופציונלי `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, אופציונלי `.ANON_STRICT_PQ` |
| עוזרי מתזמר JavaScript | `rolloutPhase: "canary"` או `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

ה-SDK הופך את המעבר בין מנתח הבמה, מתזמר (`crates/sorafs_orchestrator/src/lib.rs:365`), אפשרות פריסות מרובות שפות בשלב נעילה בשלבים.

## רשימת תיוג קנרית

1. **טיסה מוקדמת (T פחות שבועיים)**

- Убедитесь, что Stage A שיעור חום-אאוט <1% עבור כיסוי נטולי ו-PQ >=70% בממוצע (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте משבצת ביקורת ממשל, который утверждает окно canary.
   - התקן את `sorafs.gateway.rollout_phase = "ramp"` ב-Staging (תזמר JSON ו-Reployment) ו-Vыполните צינור קידום יבש.

2. **קנרית ממסר (יום ט')**

   - Продвигайте по одному региону, задавая `rollout_phase = "ramp"` על מתזמר וממסרים מניפסטים.
   - בדוק את "אירועי מדיניות לפי תוצאה" ו"שיעור חום" בלוח המחוונים של PQ Ratchet (עם פאנל השקה) ב-течение удвоенного מטמון TTL guard.
   - צלם תמונות Snapshot `sorafs_cli guard-directory fetch` לאחסון ביקורת.

3. **קנרית לקוח/SDK (T ועוד שבוע)**

   - הצג את `rollout_phase = "ramp"` בהגדרות לקוח או הפעל עקיפת `stage-b` עבור קבוצות SDK.
   - בדיקת הבדלי טלמטריה (`sorafs_orchestrator_policy_events_total`, сгруппированные по `client_id` ו- `region`) ואירוע התנתקות רגיל.

4. **מבצע ברירת מחדל (T ועוד 3 שבועות)**

   - בדוק את אישור הממשל של מתזמר ותצורות לקוח ב-`rollout_phase = "default"` ו-Rotуйте подписанный רשימת תיוג מוכנות ב-release artefacts.

## רשימת ביקורת של ממשל וראיות| שינוי שלב | שער קידום | צרור ראיות | לוחות מחוונים והתראות |
|-------------|----------------|----------------|---------------------|
| קנרי -> רמפה *(תצוגה מקדימה של שלב ב')* | שיעור שלב א' של חום <1% מתאריך 14 בינואר, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 לקידום מכירות, אימות כרטיס Argon2 p95 < 50 אלפיות השנייה או קידום מכירות של ארגון ממשל. | Пара JSON/Markdown от `cargo xtask soranet-rollout-plan`, парные צילומי מצב `sorafs_cli guard-directory fetch` (до/после), подписанный חבילה `cargo xtask soranet-rollout-capture --label canary`, и canary minutes сой runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (אירועי מדיניות + שיעור תקלות), `dashboards/grafana/soranet_privacy_metrics.json` (יחס שדרוג לאחור SN16), הפניות לטלמטריה в `docs/source/soranet/snnet16_telemetry_plan.md`. |
| רמפה -> ברירת מחדל *(אכיפה בשלב C)* | צריבת טלמטריה SN16 של 30 דונמים, `sn16_handshake_downgrade_total` נקודת התחלה, `sorafs_orchestrator_brownouts_total` לא מנוצלת ב-Client Canary, וחזרה על החלפת פרוקסי. | Транскрипт `sorafs_cli proxy set-mode --mode gateway|direct`, вывод `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, лог `sorafs_cli guard-directory verify`, חבילת подписанный `cargo xtask soranet-rollout-capture --label default`. | Тот же לוח PQ Ratchet плюс SN16 לוחות שדרוג לאחור, אופייני ל-`docs/source/sorafs_orchestrator_rollout.md` ו-`dashboards/grafana/soranet_privacy_metrics.json`. |
| הורדה בחירום / מוכנות לחזרה לאחור | Триггерится при всплесках מונים להורדה, צור אימות של מדריך שומר או אירועי שדרוג לאחור ב- буфере `/policy/proxy-toggle`. | רשימת רשימות של `docs/source/ops/soranet_transport_rollback.md`, логи `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, כרטיסים לאירועים ותבניות הודעות. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` או חבילות התראה (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Храните каждый artefact в `artifacts/soranet_pq_rollout/<timestamp>_<label>/` вместе с сгенерированным `rollout_capture.json`, чтобы מנות ממשל включали לוח תוצאות, מעקבים דיגיטליים.
- Приложите SHA256 digests загруженных доказательств (דקות PDF, צרור לכידת, תמונות מצב של שמירה) к דקות קידום, чтобы אישורי הפרלמנט можно было воспроизвод אשכול.
- Сссылайтесь на תוכנית טלמטריה בכרטיס קידום מכירות, чтобы подтвердить, что `docs/source/soranet/snnet16_telemetry_plan.md` остается каноническим источником для הורדת אוצר מילים.

## עדכוני לוח מחוונים וטלמטריה

`dashboards/grafana/soranet_pq_ratchet.json` теперь содержит פאנל ביאורים "תוכנית השקה", который ссылается на этот פלייבוק и отображает текущую שלב, סקירות ניהול משטר שלב האקטיוונו. Держите описание פאנל синхронным с будущими изменениями ידיות.

Для alerting убедитесь, что существующие כללים используют תווית `stage`, чтобы canary и שלבי ברירת מחדל триггерили отдельны01X (I180 מדיניות סף 80 מדיניות סף 80 מדיניות ערך 100).

## ווים לאחור

### ברירת מחדל -> רמפה (שלב C -> שלב ב')

1. הורדת מתזמר через `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (ו отзеркалить ту же phase в configs SDK), чтобы Stage B восстановился по всему флоту.
2. Принудить clients в безопасный transport profile через `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, сохранив תמלול לזרימת עבודה לביקורת `/policy/proxy-toggle`.
3. הצג את `cargo xtask soranet-rollout-capture --label rollback-default` עבור הבדלי מדריך שומר, פלט Promtool וצילומי מסך של לוח המחוונים под `artifacts/soranet_pq_rollout/`.

### רמפה -> קנרית (שלב ב' -> שלב א')1. הצג תמונת מצב של מדריך השומר, קידום מכירות של פרויקט, ראה `sorafs_cli guard-directory import --guard-directory guards.json` ו- повторно запустить `sorafs_cli guard-directory verify`, чтобвкhe hats.
2. התקן את `rollout_phase = "canary"` (או לעקוף את `anonymity_policy stage-a`) בהגדרות התזמור והלקוח, צור מקדחה של PQ ב-[PQ ratchet runchbook](00ч0 060), доказать צינור לשדרוג לאחור.
3. הצג צילומי מסך של PQ Ratchet ו-SN16 טלמטריה עם תוצאות התראות ויומן תקריות המאפשר ממשל.

### תזכורות למעקה בטיחות

- Сссылайтесь на `docs/source/ops/soranet_transport_rollback.md` при каждом ירידה בדרגה и фиксируйте временные mitigation как `TODO:` в rollout tracker для послед.
- Держите `dashboards/alerts/soranet_handshake_rules.yml` ו-`dashboards/alerts/soranet_privacy_rules.yml` под покрытием `promtool test rules` до и после rollback, чтобы alert drift брыл завте צרור.