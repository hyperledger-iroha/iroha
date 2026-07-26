---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0f9d0442b880d90aa91ecab746c9a42313290f7d8d02eba7e7287e297e56bf8
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pq-ratchet-runbook
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
דף זה משקף את `docs/source/soranet/pq_ratchet_runbook.md`. שמרו על שתי הגרסאות מסונכרנות עד שהדוקס הישנים יפרשו.
:::

## מטרה

Runbook זה מנחה את רצף תרגיל האש עבור מדיניות האנונימיות post-quantum (PQ) המדורגת של SoraNet. המפעילים מתרגלים הן קידום (Stage A -> Stage B -> Stage C) והן הורדה מבוקרת בחזרה ל-Stage B/A כאשר אספקת PQ יורדת. התרגיל מאמת telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) ואוסף artefacts עבור יומן תרגול תקריות.

## דרישות מוקדמות

- בינרי `sorafs_orchestrator` עדכני עם capability-weighting (commit שווה או אחרי רפרנס התרגיל ב-`docs/source/soranet/reports/pq_ratchet_validation.md`).
- גישה ל-stack של Prometheus/Grafana שמגיש `dashboards/grafana/soranet_pq_ratchet.json`.
- guard directory snapshot תקין. משכו ואמתו עותק לפני התרגיל:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

אם ה-source directory מפרסם JSON בלבד, קודדו אותו מחדש ל-Norito בינארי עם `soranet-directory build` לפני הרצת helpers של הרוטציה.

- לכידת metadata ו-pre-stage של artefacts לרוטציית issuer דרך ה-CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- חלון שינוי מאושר על ידי צוותי on-call של networking ו-observability.

## צעדי קידום

1. **Stage audit**

   רשמו את ה-stage ההתחלתי:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   צפו ל-`anon-guard-pq` לפני קידום.

2. **קידום ל-Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - המתינו >=5 דקות לרענון manifests.
   - ב-Grafana (dashboard `SoraNet PQ Ratchet Drill`) ודאו שהפאנל "Policy Events" מציג `outcome=met` עבור `stage=anon-majority-pq`.
   - צלמו screenshot או JSON של הפאנל וצרפו ליומן התקריות.

3. **קידום ל-Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - ודאו שההיסטוגרמות `sorafs_orchestrator_pq_ratio_*` נוטות ל-1.0.
   - ודאו שמונה brownout נשאר שטוח; אחרת בצעו את צעדי ההורדה.

## תרגיל הורדה / brownout

1. **יצירת מחסור PQ סינתטי**

   כבו relays של PQ בסביבת playground באמצעות קיצוץ ה-guard directory לכניסות קלאסיות בלבד, ואז טענו מחדש את ה-cache של ה-orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **צפיה ב-telemetry של brownout**

   - Dashboard: הפאנל "Brownout Rate" קופץ מעל 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` אמור לדווח `anonymity_outcome="brownout"` עם `anonymity_reason="missing_majority_pq"`.

3. **הורדה ל-Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   אם אספקת PQ עדיין לא מספיקה, הורידו ל-`anon-guard-pq`. התרגיל מסתיים כאשר מוני brownout מתייצבים וניתן ליישם קידומים מחדש.

4. **שחזור guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry ו-artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** ודאו שהתראת brownout עבור `sorafs_orchestrator_policy_events_total` נשארת מתחת ל-SLO המוגדר (&lt;5% בכל חלון של 10 דקות).
- **Incident log:** צרפו את קטעי ה-telemetry והערות המפעיל ל-`docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Signed capture:** השתמשו ב-`cargo xtask soranet-rollout-capture` כדי להעתיק את ה-drill log וה-scoreboard אל `artifacts/soranet_pq_rollout/<timestamp>/`, לחשב digests של BLAKE3 ולהפיק `rollout_capture.json` חתום.

דוגמה:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

צרפו את ה-metadata והחתימה שהופקו לחבילת governance.

## Rollback

אם התרגיל חושף מחסור PQ אמיתי, הישארו ב-Stage A, הודיעו ל-Networking TL וצרפו את המדדים שנאספו יחד עם diffs של guard directory ל-incident tracker. השתמשו ב-export של guard directory שנלכד קודם כדי לשחזר שירות רגיל.

:::tip כיסוי רגרסיה
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` מספק את ה-validation הסינתטי שמגבה תרגיל זה.
:::
