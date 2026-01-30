---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/testnet-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: testnet-rollout
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet rollout (SNNet-10)
description: مرحلہ وار activation plan، onboarding kit، اور SoraNet testnet promotions کے لئے telemetry gates.
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/testnet_rollout_plan.md` میں SNNet-10 rollout plan کی عکاسی کرتا ہے۔ جب تک پرانا docs set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

SNNet-10 نیٹ ورک بھر میں SoraNet anonymity overlay کی مرحلہ وار activation کو coordinate کرتا ہے۔ اس plan کو استعمال کریں تاکہ roadmap bullet کو concrete deliverables، runbooks، اور telemetry gates میں بدلا جا سکے تاکہ ہر operator توقعات سمجھ لے اس سے پہلے کہ SoraNet default transport بنے۔

## Launch phases

| Phase | Timeline (target) | Scope | Required artefacts |
|-------|-------------------|-------|--------------------|
| **T0 - Closed Testnet** | Q4 2026 | >=3 ASNs پر 20-50 relays جو core contributors چلاتے ہیں۔ | Testnet onboarding kit، guard pinning smoke suite، baseline latency + PoW metrics، brownout drill log. |
| **T1 - Public Beta** | Q1 2027 | >=100 relays، guard rotation enabled، exit bonding enforced، SDK betas default طور پر SoraNet کے ساتھ `anon-guard-pq`. | Updated onboarding kit، operator verification checklist، directory publishing SOP، telemetry dashboard pack، incident rehearsal reports. |
| **T2 - Mainnet Default** | Q2 2027 (SNNet-6/7/9 completion پر gated) | Production network SoraNet پر default؛ obfs/MASQUE transports اور PQ ratchet enforcement enabled۔ | Governance approval minutes، direct-only rollback procedure، downgrade alarms، signed success metrics report. |

**کوئی skip path نہیں** - ہر phase کو پچھلے مرحلے کی telemetry اور governance artefacts ship کرنا لازمی ہے قبل از promotion۔

## Testnet onboarding kit

ہر relay operator کو درج ذیل فائلوں کے ساتھ ایک deterministic package ملتا ہے:

| Artefact | Description |
|----------|-------------|
| `01-readme.md` | Overview، contact points، اور timeline۔ |
| `02-checklist.md` | Pre-flight checklist (hardware، network reachability، guard policy verification). |
| `03-config-example.toml` | SNNet-9 compliance blocks کے ساتھ align کیا ہوا minimal SoraNet relay + orchestrator config، جس میں `guard_directory` block شامل ہے جو تازہ ترین guard snapshot hash کو pin کرتا ہے۔ |
| `04-telemetry.md` | SoraNet privacy metrics dashboards اور alert thresholds wire کرنے کی ہدایات۔ |
| `05-incident-playbook.md` | Brownout/downgrade response procedure اور escalation matrix۔ |
| `06-verification-report.md` | Template جسے operators smoke tests پاس ہونے کے بعد مکمل کر کے واپس دیتے ہیں۔ |

Rendered copy `docs/examples/soranet_testnet_operator_kit/` میں موجود ہے۔ ہر promotion میں kit refresh ہوتا ہے؛ version numbers phase کو follow کرتے ہیں (مثال کے طور پر `testnet-kit-vT0.1`).

Public-beta (T1) operators کے لئے `docs/source/soranet/snnet10_beta_onboarding.md` میں concise onboarding brief prerequisites، telemetry deliverables اور submission workflow خلاصہ کرتا ہے اور deterministic kit اور validator helpers کی طرف اشارہ کرتا ہے۔

`cargo xtask soranet-testnet-feed` JSON feed بناتا ہے جو promotion window، relay roster، metrics report، drill evidence اور attachment hashes کو aggregate کرتا ہے جنہیں stage-gate template reference کرتا ہے۔ پہلے `cargo xtask soranet-testnet-drill-bundle` سے drill logs اور attachments sign کریں تاکہ feed `drill_log.signed = true` record کر سکے۔

## Success metrics

Phases کے درمیان promotion درج ذیل telemetry پر gated ہے، جو کم از کم دو ہفتے جمع کی جاتی ہے:

- `soranet_privacy_circuit_events_total`: 95% circuits brownout یا downgrade کے بغیر مکمل ہوں؛ باقی 5% PQ supply سے محدود ہوں۔
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: روزانہ fetch sessions کا <1% scheduled drills کے علاوہ brownout trigger کرے۔
- `soranet_privacy_gar_reports_total`: متوقع GAR category mix سے variance +/-10% کے اندر؛ spikes کو approved policy updates سے explain کرنا ضروری ہے۔
- PoW ticket success rate: 3 s کے target window میں >=99%; `soranet_privacy_throttles_total{scope="congestion"}` کے ذریعے report ہوتا ہے۔
- Latency (95th percentile) per region: circuits مکمل ہونے کے بعد <200 ms، `soranet_privacy_rtt_millis{percentile="p95"}` کے ذریعے capture ہوتی ہے۔

Dashboard اور alert templates `dashboard_templates/` اور `alert_templates/` میں موجود ہیں؛ انہیں اپنے telemetry repository میں mirror کریں اور CI lint checks میں شامل کریں۔ Promotion کی درخواست سے پہلے governance-facing report بنانے کے لئے `cargo xtask soranet-testnet-metrics` استعمال کریں۔

Stage-gate submissions کو `docs/source/soranet/snnet10_stage_gate_template.md` فالو کرنا ہوگا، جو `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` میں موجود ready-to-copy Markdown form کی طرف لنک کرتا ہے۔

## Verification checklist

ہر phase میں داخل ہونے سے پہلے operators کو درج ذیل پر sign-off کرنا ہوگا:

- ✅ Relay advert موجودہ admission envelope کے ساتھ signed ہو۔
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) پاس ہو۔
- ✅ `guard_directory` تازہ ترین `GuardDirectorySnapshotV2` artefact کی طرف اشارہ کرے اور `expected_directory_hash_hex` committee digest سے match ہو (relay startup validated hash log کرتا ہے)۔
- ✅ PQ ratchet metrics (`sorafs_orchestrator_pq_ratio`) مطلوبہ stage کے target thresholds سے اوپر رہیں۔
- ✅ GAR compliance config تازہ ترین tag سے match کرے (SNNet-9 catalogue دیکھیں)۔
- ✅ Downgrade alarm simulation (collectors disable کریں، 5 منٹ میں alert expect کریں)۔
- ✅ PoW/DoS drill documented mitigation steps کے ساتھ execute ہو۔

ایک pre-filled template onboarding kit میں شامل ہے۔ Operators مکمل رپورٹ governance helpdesk کو submit کرتے ہیں، پھر production credentials ملتے ہیں۔

## Governance اور reporting

- **Change control:** promotions کے لئے Governance Council approval درکار ہے جو council minutes میں درج ہو اور status page کے ساتھ attach ہو۔
- **Status digest:** ہفتہ وار updates شائع کریں جن میں relay count، PQ ratio، brownout incidents اور outstanding action items شامل ہوں (cadence شروع ہونے کے بعد `docs/source/status/soranet_testnet_digest.md` میں محفوظ)۔
- **Rollbacks:** ایک signed rollback plan برقرار رکھیں جو 30 منٹ میں نیٹ ورک کو پچھلے phase پر واپس لے جائے، جس میں DNS/guard cache invalidation اور client communication templates شامل ہوں۔

## Supporting assets

- `cargo xtask soranet-testnet-kit [--out <dir>]` `xtask/templates/soranet_testnet/` سے onboarding kit کو target directory میں materialize کرتا ہے (default `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` SNNet-10 success metrics evaluate کرتا ہے اور governance reviews کے لئے structured pass/fail report emit کرتا ہے۔ Sample snapshot `docs/examples/soranet_testnet_metrics_sample.json` میں موجود ہے۔
- Grafana اور Alertmanager templates `dashboard_templates/soranet_testnet_overview.json` اور `alert_templates/soranet_testnet_rules.yml` میں موجود ہیں؛ انہیں telemetry repository میں copy کریں یا CI lint checks میں wire کریں۔
- SDK/portal messaging کے لئے downgrade communication template `docs/source/soranet/templates/downgrade_communication_template.md` میں ہے۔
- Weekly status digests کو canonical form کے طور پر `docs/source/status/soranet_testnet_weekly_digest.md` استعمال کرنا چاہئے۔

Pull requests کو اس صفحے کو artefacts یا telemetry میں کسی بھی تبدیلی کے ساتھ update کرنا چاہئے تاکہ rollout plan canonical رہے۔
