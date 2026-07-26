---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c5262042092d8e5b18857236cd8d84fed244a301f55fc8017b7be6800f43064
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pq-ratchet-runbook
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

## مقصد

یہ runbook SoraNet کی staged post-quantum (PQ) anonymity policy کے لئے fire-drill sequence گائیڈ کرتا ہے۔ Operators promotion (Stage A -> Stage B -> Stage C) اور PQ supply کم ہونے پر controlled demotion واپس Stage B/A دونوں rehearse کرتے ہیں۔ Drill telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) validate کرتا ہے اور incident rehearsal log کے لئے artefacts جمع کرتا ہے۔

## Prerequisites

- Capability-weighting کے ساتھ تازہ ترین `sorafs_orchestrator` binary (commit drill reference کے برابر یا بعد میں جو `docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھایا گیا ہے)۔
- Prometheus/Grafana stack تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` serve کرتا ہے۔
- Nominal guard directory snapshot۔ drill سے پہلے copy fetch اور verify کریں:

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

## Promotion steps

1. **Stage audit**

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

## Demotion / brownout drill

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

## Telemetry & artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** یقینی بنائیں کہ `sorafs_orchestrator_policy_events_total` brownout alert configured SLO کے نیچے رہے (&lt;5% کسی بھی 10 minute window میں)۔
- **Incident log:** telemetry snippets اور operator notes کو `docs/examples/soranet_pq_ratchet_fire_drill.log` میں append کریں۔
- **Signed capture:** `cargo xtask soranet-rollout-capture` استعمال کریں تاکہ drill log اور scoreboard کو `artifacts/soranet_pq_rollout/<timestamp>/` میں copy کیا جا سکے، BLAKE3 digests compute ہوں، اور signed `rollout_capture.json` بنے۔

Example:

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

## Rollback

اگر drill حقیقی PQ shortage ظاہر کرے تو Stage A پر رہیں، Networking TL کو مطلع کریں، اور collected metrics کے ساتھ guard directory diffs کو incident tracker میں attach کریں۔ پہلے capture کیا گیا guard directory export استعمال کر کے normal service restore کریں۔

:::tip Regression Coverage
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` اس drill کو support کرنے والی synthetic validation فراہم کرتا ہے۔
:::
