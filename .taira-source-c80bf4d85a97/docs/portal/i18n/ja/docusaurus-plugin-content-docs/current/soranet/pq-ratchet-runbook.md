---
id: pq-ratchet-runbook
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Canonical Source
このページは `docs/source/soranet/pq_ratchet_runbook.md` を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

## 目的

この runbook は、SoraNet の段階的 post-quantum (PQ) 匿名性ポリシーに対する fire-drill シーケンスを案内する。operators は promotion (Stage A -> Stage B -> Stage C) と、PQ 供給が落ちた際の Stage B/A への controlled demotion の両方をリハーサルする。ドリルでは telemetry hooks (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) を検証し、incident rehearsal log 向けの artefacts を収集する。

## 前提条件

- capability-weighting を含む最新の `sorafs_orchestrator` binary（`docs/source/soranet/reports/pq_ratchet_validation.md` に記載された drill reference 以降の commit）。
- `dashboards/grafana/soranet_pq_ratchet.json` を提供する Prometheus/Grafana stack へのアクセス。
- 正常な guard directory snapshot。ドリル前に取得し検証する:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

source directory が JSON のみを公開している場合は、rotation helpers を実行する前に `soranet-directory build` で Norito binary に再エンコードする。

- CLI で metadata を取得し、issuer rotation artefacts を pre-stage する:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- networking/observability on-call チームが承認した change window。

## Promotion steps

1. **Stage audit**

   開始ステージを記録する:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   promotion 前に `anon-guard-pq` を期待する。

2. **Stage B (Majority PQ) へ promotion**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - manifests の更新を待つため >=5 分待機する。
   - Grafana（`SoraNet PQ Ratchet Drill` dashboard）で "Policy Events" パネルが `stage=anon-majority-pq` の `outcome=met` を示すことを確認する。
   - スクリーンショットまたはパネル JSON を取得し、incident log に添付する。

3. **Stage C (Strict PQ) へ promotion**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` histograms が 1.0 に近づくことを確認する。
   - brownout counter が平坦であることを確認し、そうでなければ demotion steps を実施する。

## Demotion / brownout drill

1. **合成 PQ shortage の誘発**

   playground 環境で guard directory を classical entries のみに削り、PQ relays を無効化し、orchestrator cache を再読み込みする:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **brownout telemetry の観測**

   - Dashboard: "Brownout Rate" パネルが 0 を超えてスパイクする。
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` が `anonymity_outcome="brownout"` と `anonymity_reason="missing_majority_pq"` を報告する。

3. **Stage B / Stage A へ demotion**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ 供給が不十分な場合は `anon-guard-pq` まで降格する。brownout counters が落ち着き、promotion を再適用できたらドリル完了。

4. **guard directory の復元**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetry & artefacts

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus alerts:** `sorafs_orchestrator_policy_events_total` の brownout alert が設定 SLO を下回ることを確認する（&lt;5% / 10 分ウィンドウ）。
- **Incident log:** 取得した telemetry snippets と operator notes を `docs/examples/soranet_pq_ratchet_fire_drill.log` に追記する。
- **Signed capture:** `cargo xtask soranet-rollout-capture` を使い、drill log と scoreboard を `artifacts/soranet_pq_rollout/<timestamp>/` にコピーし、BLAKE3 digests を計算して署名済み `rollout_capture.json` を生成する。

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

生成した metadata と signature を governance packet に添付する。

## Rollback

ドリルで実際の PQ shortage が判明した場合、Stage A に留まり、Networking TL に通知し、収集した metrics と guard directory diffs を incident tracker に添付する。先に取得した guard directory export を使って通常サービスを復元する。

:::tip Regression Coverage
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` がこの drill を裏付ける synthetic validation を提供する。
:::
