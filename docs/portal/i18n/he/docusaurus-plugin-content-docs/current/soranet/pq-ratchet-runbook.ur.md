---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pq-ratchet-runbook
כותרת: SoraNet PQ Ratchet Drill
sidebar_label: PQ Ratchet Runbook
description: مرحلہ وار PQ anonymity policy کو promote یا demote کرنے کے لئے on-call rehearsal steps اور deterministic telemetry validation.
---

:::הערה מקור קנוני
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

## הודעה

یہ runbook SoraNet کی staged post-quantum (PQ) anonymity policy کے لئے fire-drill sequence گائیڈ کرتا ہے۔ Operators promotion (Stage A -> Stage B -> Stage C) اور PQ supply کم ہونے پر controlled demotion واپس Stage B/A دونوں rehearse کرتے ہیں۔ ווי טלמטריית מקדחה (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) מאמתים את יומן החזרות של התקריות.

## דרישות מוקדמות

- שקלול יכולת. دکھایا گیا ہے)۔
- Prometheus/Grafana stack تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` serve کرتا ہے۔
- תמונת מצב של ספריית שומרים נומינלית. drill سے پہلے copy fetch اور verify کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر source directory صرف JSON publish کرتا ہے تو rotation helpers چلانے سے پہلے `soranet-directory build` سے اسے Norito binary میں re-encode کریں۔

- CLI سے metadata capture کریں اور issuer rotation artefacts pre-stage کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Networking اور observability on-call teams کی منظور شدہ change window۔

## שלבי קידום

1. **ביקורת שלב**

   ابتدا کا stage ریکارڈ کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Promotion سے پہلے `anon-guard-pq` expect کریں۔

2. **Stage B (Majority PQ) پر promote کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Manifests refresh ہونے کے لئے >=5 منٹ انتظار کریں۔
   - Grafana میں (dashboard `SoraNet PQ Ratchet Drill`) "Policy Events" panel پر `stage=anon-majority-pq` کے لئے `outcome=met` کی تصدیق کریں۔
   - Screenshot یا panel JSON capture کریں اور incident log میں attach کریں۔

3. **Stage C (Strict PQ) پر promote کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` histograms کو 1.0 کی طرف جاتا ہوا verify کریں۔
   - Brownout counter کا flat رہنا confirm کریں؛ ورنہ demotion steps فالو کریں۔

## תרגיל ירידה בדרגה / חום

1. **Synthetic PQ shortage پیدا کریں**

   Playground environment میں guard directory کو صرف classical entries تک trim کر کے PQ relays disable کریں، پھر orchestrator cache reload کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Brownout telemetry observe کریں**

   - Dashboard: "Brownout Rate" panel 0 سے اوپر spike کرے۔
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_outcome="brownout"` اور `anonymity_reason="missing_majority_pq"` رپورٹ کرنا چاہئے۔

3. **Stage B / Stage A پر demote کریں**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر PQ supply اب بھی ناکافی ہو تو `anon-guard-pq` پر demote کریں۔ Drill اس وقت مکمل ہوتا ہے جب brownout counters settle ہوں اور promotions دوبارہ لاگو ہو سکیں۔

4. **Guard directory بحال کریں**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## טלמטריה וחפצי אמנות- **לוח מחוונים:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** یقینی بنائیں کہ `sorafs_orchestrator_policy_events_total` brownout alert configured SLO کے نیچے رہے (<5% کسی بھی 10 minute window میں)۔
- **Incident log:** telemetry snippets اور operator notes کو `docs/examples/soranet_pq_ratchet_fire_drill.log` میں append کریں۔
- **Signed capture:** `cargo xtask soranet-rollout-capture` استعمال کریں تاکہ drill log اور scoreboard کو `artifacts/soranet_pq_rollout/<timestamp>/` میں copy کیا جا سکے، BLAKE3 digests compute ہوں، اور signed `rollout_capture.json` بنے۔

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

Generated metadata اور signature کو governance packet کے ساتھ attach کریں۔

## חזרה לאחור

מקדחה תקח מחסור ב-PQ מעקב שלב א', רשתות TL מעקב אחר מדדים שנאספו תקריות מדריך דפים میں attach کریں۔ پہلے capture کیا گیا guard directory export استعمال کر کے normal service restore کریں۔

:::tip כיסוי רגרסיה
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` اس drill کو support کرنے والی synthetic validation فراہم کرتا ہے۔
:::