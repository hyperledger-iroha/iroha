---
id: pq-rollout-plan
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
このページは `docs/source/soranet/pq_rollout_plan.md` を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

SNNet-16G は SoraNet transport の post-quantum rollout を完了させる。`rollout_phase` knobs により、既存の Stage A guard 要件から Stage B の majority coverage、Stage C の strict PQ posture まで、各 surface の生 JSON/TOML を編集せずに deterministic な promotion を調整できる。

この playbook で扱う内容:

- Phase 定義と新しい configuration knobs (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) の codebase 反映 (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`)。
- SDK と CLI flags の mapping により各 client が rollout を追跡できるようにする。
- Relay/client の canary scheduling 期待値と、promotion を gate する governance dashboards (`dashboards/grafana/soranet_pq_ratchet.json`)。
- Rollback hooks と fire-drill runbook への参照 ([PQ ratchet runbook](./pq-ratchet-runbook.md))。

## Phase map

| `rollout_phase` | Effective anonymity stage | Default effect | Typical usage |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | フリートが温まる間、各 circuit に少なくとも 1 つの PQ guard を要求する。 | Baseline と初期 canary 週。 |
| `ramp`          | `anon-majority-pq` (Stage B) | 選択を PQ relays へ偏らせ、>= 2/3 の coverage を確保する。classical relays は fallback として残る。 | Region-by-region relay canaries; SDK preview toggles。 |
| `default`       | `anon-strict-pq` (Stage C) | PQ-only circuits を強制し、downgrade alarms を強化する。 | Telemetry と governance sign-off が完了した後の最終 promotion。 |

Surface が `anonymity_policy` を明示設定している場合、それが該当 component の phase を override する。明示的な stage を省略すると `rollout_phase` 値に defer されるため、operators は環境ごとに一度 phase を切り替え、clients がそれを継承できる。

## Configuration reference

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator loader は runtime で fallback stage を解決し (`crates/sorafs_orchestrator/src/lib.rs:2229`)、`sorafs_orchestrator_policy_events_total` と `sorafs_orchestrator_pq_ratio_*` に露出する。適用済みの snippets として `docs/examples/sorafs_rollout_stage_b.toml` と `docs/examples/sorafs_rollout_stage_c.toml` を参照。

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` は parse した phase を記録するようになった (`crates/iroha/src/client.rs:2315`) ため、helper commands (例: `iroha_cli app sorafs fetch`) は現在の phase を default anonymity policy とともに報告できる。

## Automation

2 つの `cargo xtask` helpers が schedule 生成と artefact capture を自動化する。

1. **Regional schedule を生成する**

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

   Durations は `s`, `m`, `h`, `d` の suffix を受け付ける。このコマンドは `artifacts/soranet_pq_rollout_plan.json` と Markdown summary (`artifacts/soranet_pq_rollout_plan.md`) を出力し、change request に添付できる。

2. **署名付き drill artefacts を capture**

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

   コマンドは指定ファイルを `artifacts/soranet_pq_rollout/<timestamp>_<label>/` にコピーし、各 artefact の BLAKE3 digests を計算し、payload の Ed25519 署名を含む `rollout_capture.json` を書き出す。fire-drill minutes と同じ private key を使用し、governance が迅速に capture を検証できるようにする。

## SDK & CLI flag matrix

| Surface | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` または phase 依存 | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` または `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

すべての SDK toggles は orchestrator と同じ stage parser (`crates/sorafs_orchestrator/src/lib.rs:365`) にマップされるため、複数言語のデプロイでも設定 phase と lock-step を保てる。

## Canary scheduling checklist

1. **Preflight (T minus 2 weeks)**

- Stage A の brownout rate が直近 2 週間で <1% かつ PQ coverage が地域ごとに >=70% であることを確認する (`sorafs_orchestrator_pq_candidate_ratio`)。
   - Canary window を承認する governance review slot を確保する。
   - Staging で `sorafs.gateway.rollout_phase = "ramp"` を更新（orchestrator JSON を編集して redeploy）し、promotion pipeline を dry-run する。

2. **Relay canary (T day)**

   - Orchestrator と参加 relay manifests に `rollout_phase = "ramp"` を設定し、1 地域ずつ promotion する。
   - PQ Ratchet dashboard（rollout panel 追加済み）の "Policy Events per Outcome" と "Brownout Rate" を guard cache TTL の 2 倍の間監視する。
   - 監査保管のために `sorafs_cli guard-directory fetch` snapshots を前後で取得する。

3. **Client/SDK canary (T plus 1 week)**

   - Client configs を `rollout_phase = "ramp"` に切り替えるか、指定 SDK cohorts に `stage-b` overrides を渡す。
   - Telemetry diffs (`sorafs_orchestrator_policy_events_total` を `client_id` と `region` で集計) を取得し、rollout incident log に添付する。

4. **Default promotion (T plus 3 weeks)**

   - Governance の承認後、orchestrator と client configs の両方を `rollout_phase = "default"` に切り替え、署名済み readiness checklist を release artefacts に回す。

## Governance & evidence checklist

| Phase change | Promotion gate | Evidence bundle | Dashboards & alerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Stage-A brownout rate <1% (直近 14 日)、`sorafs_orchestrator_pq_candidate_ratio` >= 0.7 (promote 対象地域)、Argon2 ticket verify p95 < 50 ms、かつ promotion の governance slot が予約済み。 | `cargo xtask soranet-rollout-plan` の JSON/Markdown ペア、`sorafs_cli guard-directory fetch` の before/after snapshots、署名済み `cargo xtask soranet-rollout-capture --label canary` bundle、[PQ ratchet runbook](./pq-ratchet-runbook.md) を参照した canary minutes。 | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio)、`docs/source/soranet/snnet16_telemetry_plan.md` の telemetry references。 |
| Ramp -> Default *(Stage C enforcement)* | 30 日の SN16 telemetry burn-in 達成、`sn16_handshake_downgrade_total` が baseline でフラット、client canary 中の `sorafs_orchestrator_brownouts_total` がゼロ、proxy toggle rehearsal が記録済み。 | `sorafs_cli proxy set-mode --mode gateway|direct` transcript、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml` 出力、`sorafs_cli guard-directory verify` log、署名済み `cargo xtask soranet-rollout-capture --label default` bundle。 | 同じ PQ Ratchet board と `docs/source/sorafs_orchestrator_rollout.md` および `dashboards/grafana/soranet_privacy_metrics.json` の SN16 downgrade panels。 |
| Emergency demotion / rollback readiness | downgrade counters の急増、guard-directory 検証失敗、または `/policy/proxy-toggle` バッファに持続的な downgrade events が記録された場合にトリガー。 | `docs/source/ops/soranet_transport_rollback.md` の checklist、`sorafs_cli guard-directory import` / `guard-cache prune` logs、`cargo xtask soranet-rollout-capture --label rollback`、incident tickets、notification templates。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json`、および両 alert packs (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`)。 |

- すべての artefact を `artifacts/soranet_pq_rollout/<timestamp>_<label>/` に格納し、生成された `rollout_capture.json` を含めて governance packet に scoreboard、promtool traces、digests を含める。
- アップロード済み evidence（minutes PDF、capture bundle、guard snapshots）の SHA256 digests を promotion minutes に添付し、Parliament approvals が staging cluster なしで再現できるようにする。
- Promotion ticket で telemetry plan を参照し、`docs/source/soranet/snnet16_telemetry_plan.md` が downgrade vocabularies と alert thresholds の canonical source であることを示す。

## Dashboard & telemetry updates

`dashboards/grafana/soranet_pq_ratchet.json` には "Rollout Plan" annotation panel が追加され、この playbook へのリンクと current phase を表示する。Governance reviews が現在の stage を確認できるようにするため、panel description は config knobs の将来変更と同期して保つこと。

Alerting では、既存の rules が `stage` label を使うようにし、canary と default phases が別々の policy thresholds をトリガーするようにする (`dashboards/alerts/soranet_handshake_rules.yml`)。

## Rollback hooks

### Default -> Ramp (Stage C -> Stage B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` で orchestrator を demote し、SDK configs も同じ phase に揃えて Stage B をフリート全体に戻す。
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` で clients を safe transport profile に強制し、`/policy/proxy-toggle` remediation workflow の監査性を維持するため transcript を取得する。
3. `cargo xtask soranet-rollout-capture --label rollback-default` を実行し、guard-directory diffs、promtool output、dashboard screenshots を `artifacts/soranet_pq_rollout/` にアーカイブする。

### Ramp -> Canary (Stage B -> Stage A)

1. Promotion 前に取得した guard-directory snapshot を `sorafs_cli guard-directory import --guard-directory guards.json` で取り込み、`sorafs_cli guard-directory verify` を再実行して demotion packet に hashes を含める。
2. Orchestrator と client configs に `rollout_phase = "canary"`（または `anonymity_policy stage-a` override）を設定し、[PQ ratchet runbook](./pq-ratchet-runbook.md) の PQ ratchet drill を再実行して downgrade pipeline を証明する。
3. 更新された PQ Ratchet と SN16 telemetry screenshots と alert outcomes を incident log に添付してから governance に通知する。

### Guardrail reminders

- Demotion 発生時は必ず `docs/source/ops/soranet_transport_rollback.md` を参照し、暫定 mitigation は rollout tracker に `TODO:` として記録する。
- `dashboards/alerts/soranet_handshake_rules.yml` と `dashboards/alerts/soranet_privacy_rules.yml` を rollback の前後で `promtool test rules` の対象にし、alert drift を capture bundle と併せて文書化する。
