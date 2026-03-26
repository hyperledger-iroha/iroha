---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd5e56947cf480b976997e91c13208562bb78b3d5f9aa2c9ab67a4f1494943bb
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: הקצאה אלסטית של lane (NX-7)
sidebar_label: הקצאה אלסטית של lane
description: תהליך bootstrap ליצירת manifests של Nexus lane, ערכי קטלוג וראיות rollout.
---

:::note מקור קנוני
העמוד הזה משקף את `docs/source/nexus_elastic_lane.md`. שמרו על שתי הגרסאות מיושרות עד שהתרגום יגיע לפורטל.
:::

# ערכת כלים להקצאה אלסטית של lane (NX-7)

> **פריט roadmap:** NX-7 - tooling להקצאה אלסטית של lane  
> **סטטוס:** tooling הושלם - מייצר manifests, קטעי קטלוג, Norito payloads, smoke tests,
> ועוזר ה-bundle ל-load-test מחבר כעת gating של השהיית slot + manifests של ראיות כדי שניתן יהיה לפרסם ריצות עומס של validators
> בלי scripting מותאם.

המדריך הזה מוביל מפעילים דרך העוזר החדש `scripts/nexus_lane_bootstrap.sh` שמאוטומט יצירת manifests ל-lane, קטעי קטלוג lane/dataspace וראיות rollout. המטרה היא להקל על הקמה של Nexus lanes חדשים (public או private) בלי לערוך ידנית מספר קבצים ובלי לגזור מחדש את גאומטריית הקטלוג ידנית.

## 1. דרישות מקדימות

1. אישור governance עבור alias של lane, dataspace, סט validators, fault tolerance (`f`) ומדיניות settlement.
2. רשימת validators סופית (account IDs) ורשימת namespaces מוגנים.
3. גישה לריפו הקונפיגורציה של הנוד כדי לצרף את הקטעים שנוצרו.
4. נתיבים לרג'יסטרי manifests של lane (ראו `nexus.registry.manifest_directory` ו-`cache_directory`).
5. אנשי קשר לטלמטריה/handles של PagerDuty עבור ה-lane כדי שההתראות יחוברו מיד כשה-lane באונליין.

## 2. יצירת artefacts של lane

הריצו את העוזר מה-root של הריפו:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

דגלים עיקריים:

- `--lane-id` חייב להתאים ל-index של הערך החדש ב-`nexus.lane_catalog`.
- `--dataspace-alias` ו-`--dataspace-id/hash` שולטים בערך הקטלוג של dataspace (ברירת מחדל: id של lane כאשר מושמט).
- `--validator` ניתן לחזור עליו או לקרוא מ-`--validators-file`.
- `--route-instruction` / `--route-account` מפיקים כללי ניתוב מוכנים להדבקה.
- `--metadata key=value` (או `--telemetry-contact/channel/runbook`) מתעדים אנשי קשר של runbook כדי שה-dashboards יציגו את ה-owners הנכונים.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` מוסיפים hook של runtime-upgrade ל-manifest כאשר ה-lane דורש בקרות מפעיל מורחבות.
- `--encode-space-directory` מריץ אוטומטית `cargo xtask space-directory encode`. השתמשו עם `--space-directory-out` כדי להוציא את קובץ ה-`.to` המקודד למיקום שאינו ברירת המחדל.

הסקריפט מפיק שלושה artefacts בתוך `--output-dir` (ברירת מחדל: התיקיה הנוכחית), ועוד רביעי אופציונלי כאשר encoding מופעל:

1. `<slug>.manifest.json` - manifest של lane הכולל quorum של validators, namespaces מוגנים ומטא-דאטה אופציונלי עבור hook runtime-upgrade.
2. `<slug>.catalog.toml` - קטע TOML עם `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` וכללי ניתוב שביקשתם. ודאו ש-`fault_tolerance` מוגדר בערך dataspace כדי לגודל ועדת lane-relay (`3f+1`).
3. `<slug>.summary.json` - סיכום ביקורת שמתאר את הגאומטריה (slug, segments, מטא-דאטה), שלבי rollout נדרשים והפקודה המדויקת `cargo xtask space-directory encode` (תחת `space_directory_encode.command`). צרפו את ה-JSON לטיקט onboarding כראיה.
4. `<slug>.manifest.to` - נוצר כאשר `--encode-space-directory` פעיל; מוכן לזרימת `iroha app space-directory manifest publish` ב-Torii.

השתמשו ב-`--dry-run` כדי להציג JSON/קטעים בלי כתיבת קבצים, ו-`--force` כדי לדרוס artefacts קיימים.

## 3. החלת השינויים

1. העתיקו את manifest JSON ל-`nexus.registry.manifest_directory` המוגדר (וגם ל-cache directory אם ה-registry משכפל bundles מרוחקים). בצעו commit לקובץ אם manifests מווסננים בריפו הקונפיגורציה.
2. הוסיפו את קטע הקטלוג ל-`config/config.toml` (או ל-`config.d/*.toml` המתאים). ודאו ש-`nexus.lane_count` לפחות `lane_id + 1`, ועדכנו כל `nexus.routing_policy.rules` שצריכים להצביע ל-lane החדש.
3. קודדו (אם דילגתם על `--encode-space-directory`) ופרסמו את ה-manifest ב-Space Directory באמצעות הפקודה שנלכדה ב-summary (`space_directory_encode.command`). זה מייצר payload `.manifest.to` ש-Torii מצפה לו ורושם ראיות עבור auditors; שלחו דרך `iroha app space-directory manifest publish`.
4. הריצו `irohad --sora --config path/to/config.toml --trace-config` וארכבו את פלט ה-trace בטיקט rollout. זה מוכיח שהגאומטריה החדשה תואמת ל-slug/segments של Kura שנוצרו.
5. אתחלו מחדש את ה-validators שהוקצו ל-lane לאחר פריסת שינויים במניפסט/קטלוג. שמרו את summary JSON בטיקט לביקורות עתידיות.

## 4. בניית bundle להפצת registry

ארזו את ה-manifest וה-overlay שנוצרו כדי שמפעילים יוכלו להפיץ נתוני governance של lanes בלי לערוך configs בכל host. עוזר ה-bundler מעתיק manifests לפריסת canonical, מייצר overlay אופציונלי של governance catalog עבור `nexus.registry.cache_directory`, ויכול להוציא tarball להעברה offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

פלטים:

1. `manifests/<slug>.manifest.json` - העתיקו ל-`nexus.registry.manifest_directory` המוגדר.
2. `cache/governance_catalog.json` - הניחו ב-`nexus.registry.cache_directory`. כל כניסה של `--module` הופכת להגדרת מודול הניתנת להחלפה, ומאפשרת swap-outs של מודול governance (NX-2) על ידי עדכון overlay של ה-cache במקום לערוך `config.toml`.
3. `summary.json` - כולל hashes, מטא-דאטה של overlay והנחיות למפעילים.
4. אופציונלי `registry_bundle.tar.*` - מוכן ל-SCP, S3 או trackers של artefacts.

סנכרנו את התיקיה כולה (או הארכיון) לכל validator, חלצו על hosts מבודדים, והעתיקו manifests + overlay cache לנתיבי ה-registry לפני אתחול Torii.

## 5. smoke tests ל-validators

לאחר אתחול Torii, הריצו את עוזר ה-smoke החדש כדי לוודא שה-lane מדווח `manifest_ready=true`, שהמדדים מציגים את מספר ה-lanes הצפוי, ושה-gauge של sealed נקי. lanes שדורשים manifests חייבים לחשוף `manifest_path` לא ריק; העוזר נכשל מיד כאשר הנתיב חסר כדי שכל פריסה NX-7 תכלול הוכחת manifest חתום:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

הוסיפו `--insecure` בעת בדיקת סביבות self-signed. הסקריפט מסתיים בקוד שאינו אפס אם ה-lane חסר, sealed, או אם metrics/telemetry סוטים מהערכים המצופים. השתמשו ב-`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` ו-`--max-headroom-events` כדי לשמור telemetery לפי lane (גובה בלוק/סופיות/backlog/headroom) בתוך המעטפת התפעולית, ושלבו עם `--max-slot-p95` / `--max-slot-p99` (וגם `--min-slot-samples`) כדי לאכוף את יעדי משך slot של NX-18 מבלי לצאת מה-helper.

לבדיקות air-gapped (או CI) אפשר לשחזר תגובת Torii שנלכדה במקום לפנות ל-endpoint חי:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

ה-fixtures המוקלטים תחת `fixtures/nexus/lanes/` משקפים את ה-artefacts שנוצרים ב-bootstrap helper כדי שניתן יהיה לבצע lint על manifests חדשים בלי scripting מותאם. ה-CI מריץ את אותו זרם דרך `ci/check_nexus_lane_smoke.sh` ו-`ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) כדי לוודא שעוזר ה-smoke של NX-7 נשאר תואם לפורמט ה-payload שפורסם וכדי להבטיח ש-digests/overlays של bundle נשארים שחזוריים.
