---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: SoraNet کے hybrid X25519+ML-KEM handshake کو canary سے default تک relays، clients اور SDKs میں promote کرنے کے لئے عملی رہنمائی۔
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

SNNet-16G SoraNet transport کے لئے post-quantum rollout مکمل کرتا ہے۔ `rollout_phase` knobs operators کو deterministic promotion coordinate کرنے دیتے ہیں جو موجودہ Stage A guard requirement سے Stage B majority coverage اور Stage C strict PQ posture تک جاتی ہے، بغیر ہر surface کے raw JSON/TOML کو edit کئے۔

یہ playbook درج ذیل چیزیں cover کرتا ہے:

- Phase definitions اور نئے configuration knobs (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) جو codebase میں wired ہیں (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- SDK اور CLI flags mapping تاکہ ہر client rollout کو track کر سکے۔
- Relay/client canary scheduling expectations اور governance dashboards جو promotion کو gate کرتے ہیں (`dashboards/grafana/soranet_pq_ratchet.json`).
- Rollback hooks اور fire-drill runbook کی references ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Phase map

| `rollout_phase` | Effective anonymity stage | Default effect | Typical usage |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | فلیٹ کے گرم ہونے تک ہر circuit کے لئے کم از کم ایک PQ guard لازم کریں۔ | Baseline اور ابتدائی canary ہفتے۔ |
| `ramp`          | `anon-majority-pq` (Stage B) | انتخاب کو PQ relays کی طرف bias کریں تاکہ >= دو تہائی coverage ہو؛ classical relays fallback رہیں۔ | Region-by-region relay canaries؛ SDK preview toggles۔ |
| `default`       | `anon-strict-pq` (Stage C) | PQ-only circuits enforce کریں اور downgrade alarms سخت کریں۔ | Telemetry اور governance sign-off مکمل ہونے کے بعد آخری promotion۔ |

اگر کوئی surface explicit `anonymity_policy` بھی set کرے تو وہ اس component کے لئے phase کو override کرتا ہے۔ Explicit stage نہ ہو تو اب `rollout_phase` value پر defer ہوتا ہے تاکہ operators ہر environment میں ایک بار phase flip کریں اور clients اسے inherit کریں۔

## Configuration reference

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator loader runtime میں fallback stage resolve کرتا ہے (`crates/sorafs_orchestrator/src/lib.rs:2229`) اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے surface کرتا ہے۔ `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` میں ready-to-apply snippets دیکھیں۔

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب parsed phase record کرتا ہے (`crates/iroha/src/client.rs:2315`) تاکہ helper commands (مثال کے طور پر `iroha_cli app sorafs fetch`) موجودہ phase کو default anonymity policy کے ساتھ report کر سکیں۔

## Automation

دو `cargo xtask` helpers schedule generation اور artefact capture automate کرتے ہیں۔

1. **Regional schedule generate کریں**

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

   Durations `s`, `m`, `h`, یا `d` suffix قبول کرتے ہیں۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` اور Markdown summary (`artifacts/soranet_pq_rollout_plan.md`) emit کرتی ہے جسے change request کے ساتھ بھیجا جا سکتا ہے۔

2. **Drill artefacts کو signatures کے ساتھ capture کریں**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   کمانڈ فراہم کردہ فائلیں `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں کاپی کرتی ہے، ہر artefact کے لئے BLAKE3 digests compute کرتی ہے، اور `rollout_capture.json` لکھتی ہے جس میں metadata اور payload پر Ed25519 signature شامل ہوتا ہے۔ اسی private key کو استعمال کریں جو fire-drill minutes sign کرتی ہے تاکہ governance جلدی validate کر سکے۔

## SDK & CLI flag matrix

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` یا phase پر انحصار | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تمام SDK toggles اسی stage parser سے map ہوتی ہیں جو orchestrator استعمال کرتا ہے (`crates/sorafs_orchestrator/src/lib.rs:365`)، لہذا multi-language deployments configured phase کے ساتھ lock-step رہتے ہیں۔

## Canary scheduling checklist

1. **Preflight (T minus 2 weeks)**

- Confirm کریں کہ Stage A brownout rate پچھلے دو ہفتوں میں <1% ہے اور PQ coverage فی region >=70% ہے (`sorafs_orchestrator_pq_candidate_ratio`).
   - Governance review slot schedule کریں جو canary window approve کرتا ہے۔
   - Staging میں `sorafs.gateway.rollout_phase = "ramp"` update کریں (orchestrator JSON edit کر کے redeploy) اور promotion pipeline کو dry-run کریں۔

2. **Relay canary (T day)**

   - ایک وقت میں ایک region promote کریں، `rollout_phase = "ramp"` کو orchestrator اور participating relay manifests پر set کرتے ہوئے۔
   - PQ Ratchet dashboard پر "Policy Events per Outcome" اور "Brownout Rate" کو guard cache TTL کے دوگنے عرصے تک monitor کریں (dashboard اب rollout panel دکھاتا ہے)۔
   - Audit storage کے لئے `sorafs_cli guard-directory fetch` snapshots پہلے اور بعد میں لیں۔

3. **Client/SDK canary (T plus 1 week)**

   - Client configs میں `rollout_phase = "ramp"` flip کریں یا منتخب SDK cohorts کے لئے `stage-b` overrides دیں۔
   - Telemetry diffs capture کریں (`sorafs_orchestrator_policy_events_total` کو `client_id` اور `region` کے حساب سے group کریں) اور انہیں rollout incident log کے ساتھ attach کریں۔

4. **Default promotion (T plus 3 weeks)**

   - Governance sign-off کے بعد orchestrator اور client configs دونوں کو `rollout_phase = "default"` پر switch کریں اور signed readiness checklist کو release artefacts میں rotate کریں۔

## Governance & evidence checklist

| Phase change | Promotion gate | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Stage-A brownout rate پچھلے 14 دن میں <1%، `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 فی promoted region، Argon2 ticket verify p95 < 50 ms، اور promotion کے لئے governance slot booked۔ | `cargo xtask soranet-rollout-plan` JSON/Markdown pair، `sorafs_cli guard-directory fetch` کے paired snapshots (before/after)، signed `cargo xtask soranet-rollout-capture --label canary` bundle، اور canary minutes جو [PQ ratchet runbook](./pq-ratchet-runbook.md) refer کرتے ہیں۔ | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate)، `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio)، telemetry references in `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | 30-day SN16 telemetry burn-in مکمل، `sn16_handshake_downgrade_total` baseline پر flat، client canary کے دوران `sorafs_orchestrator_brownouts_total` صفر، اور proxy toggle rehearsal log۔ | `sorafs_cli proxy set-mode --mode gateway|direct` transcript، `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` output، `sorafs_cli guard-directory verify` log، اور signed `cargo xtask soranet-rollout-capture --label default` bundle۔ | وہی PQ Ratchet board اور SN16 downgrade panels جو `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں documented ہیں۔ |
| Emergency demotion / rollback readiness | Trigger تب ہوتا ہے جب downgrade counters spike کریں، guard-directory verification fail ہو، یا `/policy/proxy-toggle` buffer مسلسل downgrade events ریکارڈ کرے۔ | `docs/source/ops/soranet_transport_rollback.md` کا checklist، `sorafs_cli guard-directory import` / `guard-cache prune` logs، `cargo xtask soranet-rollout-capture --label rollback`، incident tickets، اور notification templates۔ | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, اور دونوں alert packs (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- ہر artefact کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں store کریں، generated `rollout_capture.json` کے ساتھ، تاکہ governance packets میں scoreboard، promtool traces، اور digests شامل ہوں۔
- Uploaded evidence (minutes PDF، capture bundle، guard snapshots) کے SHA256 digests promotion minutes کے ساتھ attach کریں تاکہ Parliament approvals staging cluster تک رسائی کے بغیر replay ہو سکیں۔
- Promotion ticket میں telemetry plan کا حوالہ دیں تاکہ ثابت ہو کہ `docs/source/soranet/snnet16_telemetry_plan.md` downgrade vocabularies اور alert thresholds کے لئے canonical source ہے۔

## Dashboard & telemetry updates

`dashboards/grafana/soranet_pq_ratchet.json` اب "Rollout Plan" annotation panel کے ساتھ ship ہوتا ہے جو اس playbook سے link کرتا ہے اور current phase ظاہر کرتا ہے تاکہ governance reviews active stage کی تصدیق کر سکیں۔ Panel description کو config knobs کی مستقبل تبدیلیوں کے ساتھ sync رکھیں۔

Alerting کے لئے یقینی بنائیں کہ موجودہ rules `stage` label استعمال کریں تاکہ canary اور default phases الگ policy thresholds trigger کریں (`dashboards/alerts/soranet_handshake_rules.yml`).

## Rollback hooks

### Default -> Ramp (Stage C -> Stage B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` کے ذریعے orchestrator کو demote کریں (اور SDK configs میں وہی phase mirror کریں) تاکہ Stage B پورے fleet میں دوبارہ لاگو ہو۔
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے clients کو safe transport profile پر مجبور کریں، اور transcript capture کریں تاکہ `/policy/proxy-toggle` remediation workflow auditable رہے۔
3. `cargo xtask soranet-rollout-capture --label rollback-default` چلائیں تاکہ guard-directory diffs، promtool output، اور dashboard screenshots کو `artifacts/soranet_pq_rollout/` میں archive کیا جا سکے۔

### Ramp -> Canary (Stage B -> Stage A)

1. Promotion سے پہلے capture کیا گیا guard-directory snapshot `sorafs_cli guard-directory import --guard-directory guards.json` کے ذریعے import کریں اور `sorafs_cli guard-directory verify` دوبارہ چلائیں تاکہ demotion packet میں hashes شامل ہوں۔
2. Orchestrator اور client configs میں `rollout_phase = "canary"` set کریں (یا `anonymity_policy stage-a` override) اور پھر [PQ ratchet runbook](./pq-ratchet-runbook.md) سے PQ ratchet drill repeat کریں تاکہ downgrade pipeline prove ہو۔
3. Updated PQ Ratchet اور SN16 telemetry screenshots کے ساتھ alert outcomes کو incident log میں attach کریں، پھر governance کو notify کریں۔

### Guardrail reminders

- جب بھی demotion ہو `docs/source/ops/soranet_transport_rollback.md` refer کریں اور temporary mitigation کو rollout tracker میں `TODO:` کے طور پر log کریں تاکہ follow-up ہو سکے۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو rollback سے پہلے اور بعد میں `promtool test rules` coverage میں رکھیں تاکہ alert drift capture bundle کے ساتھ document ہو۔
