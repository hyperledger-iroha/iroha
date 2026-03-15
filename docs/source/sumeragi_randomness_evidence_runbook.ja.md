---
lang: ja
direction: ltr
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

<!-- 日本語訳: docs/source/sumeragi_randomness_evidence_runbook.md -->

# Sumeragi Randomness & Evidence Runbook

このガイドは、VRF ランダムネスと slashing evidence に関するオペレーター手順の更新を
求めた Milestone A6 の要件を満たします。新しいバリデータビルドを投入する場合や、
ガバナンス向けの readiness アーティファクトを収集する場合は、{doc}`sumeragi` と
{doc}`sumeragi_chaos_performance_runbook` と併せて使用してください。


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## スコープと前提条件

- 対象クラスタ向けに `iroha_cli` が設定済み（`docs/source/cli.md` を参照）。
- 入力準備のために Torii `/status` payload を取得する `curl`/`jq`。
- `sumeragi_vrf_*` メトリクス用の Prometheus アクセス（または snapshot exports）。
- 現在の epoch と roster を把握し、CLI 出力を staking snapshot または governance manifest に合わせられること。

## 1. モード選択と epoch コンテキストの確認

1. `iroha --output-format text ops sumeragi params` を実行して、バイナリが
   `sumeragi.consensus_mode="npos"` を読み込んでいることを示し、`k_aggregators`、
   `redundant_send_r`、epoch 長、VRF commit/reveal offsets を記録します。
2. runtime view を確認します:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi collectors
   iroha --output-format text ops sumeragi rbc status
   ```

   `status` 行は leader/view タプル、RBC backlog、DA retries、epoch offsets、
   pacemaker deferrals を出力します。`collectors` は collector index を peer ID に
   マップし、検査中の height でどのバリデータがランダムネスの職務を担っているかを示します。
3. 監査する epoch 番号を取得します:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   値（10 進または `0x` 付き）を下記 VRF コマンドで使えるよう保存します。

## 2. VRF epoch とペナルティのスナップショット

各バリデータから永続化された VRF 記録を取得するため、専用 CLI サブコマンドを使います:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

サマリは epoch の確定有無、commit/reveal の提出数、roster 長、導出 seed を表示します。
JSON には参加者リスト、signer ごとのペナルティ状態、pacemaker が使う `seed_hex` が
含まれます。参加者数を staking roster と突き合わせ、chaos テストで発生したアラートが
ペナルティ配列に反映されていることを確認してください（late reveals は `late_reveals`、
forfeited validators は `no_participation` に出るべきです）。

## 3. VRF テレメトリとアラートの監視

Prometheus は roadmap で要求されたカウンタを公開します:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

週次レポート用 PromQL 例:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

readiness drills 中は以下を確認します:

- `sumeragi_vrf_commits_emitted_total` と `..._reveals_emitted_total` が
  commit/reveal window 内の各ブロックで増加する。
- late-reveal シナリオが `sumeragi_vrf_reveals_late_total` を増やし、
  `vrf_penalties` JSON の該当エントリをクリアする。
- `sumeragi_vrf_no_participation_total` は chaos テストで意図的に commit を
  withheld した場合のみ増加する。

Grafana overview（`docs/source/grafana_sumeragi_overview.json`）には各カウンタの
パネルがあります。実行ごとにスクリーンショットを撮り、
{doc}`sumeragi_chaos_performance_runbook` で参照されるアーティファクト束に添付してください。

## 4. Evidence の取り込みとストリーミング

Slashing evidence は全バリデータで収集し、Torii に中継される必要があります。
{doc}`torii/sumeragi_evidence_app_api` に記載の HTTP endpoints と同等であることを
示すため、CLI helpers を使います:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

報告される `total` が Grafana の `sumeragi_evidence_records_total` ウィジェットと一致する
ことを確認し、`sumeragi.npos.reconfig.evidence_horizon_blocks` より古いレコードが
拒否されることを確認してください（CLI が drop reason を表示します）。alerting を試す際は、
以下の known-good payload を送信します:

```bash
iroha --output-format text ops sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex
```

`/v2/events/sse` をフィルタ付きで監視し、SDKs が同じデータを見ていることを示します。
{doc}`torii/sumeragi_evidence_app_api` の Python one-liner を使ってフィルタを構築し、
生の `data:` フレームを取得してください。SSE payload は CLI 出力に出た evidence kind と
signer をエコーするはずです。

## 5. Evidence のパッケージングとレポート

各 rehearsal または release candidate では:

1. CLI の JSON（`vrf_epoch_*.json`, `vrf_penalties_*.json`, `evidence_snapshot.json`）を
   run の artifacts ディレクトリに保存（chaos/performance スクリプトと同じ root）。
2. 上記カウンタの Prometheus クエリ結果または snapshot exports を記録。
3. SSE キャプチャとアラートの acknowledgement を artifacts README に添付。
4. `status.md` と
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` に artifact パスと
   監査した epoch 番号を記載。

このチェックリストに従うことで、NPoS rollout 中の VRF ランダムネス証跡と slashing evidence
が監査可能になり、ガバナンスレビュアがメトリクスと CLI スナップショットへ辿れる
決定的な経路を確保できます。

## 6. トラブルシューティング信号

- **Mode selection mismatch** — `iroha --output-format text ops sumeragi params` が
  `consensus_mode="permissioned"` を示す、または `k_aggregators` が manifest と
  異なる場合、取得済み artifacts を破棄し、`iroha_config` を修正して
  バリデータを再起動し、{doc}`sumeragi` の検証フローを再実行してください。
- **Missing commits or reveals** — `sumeragi_vrf_commits_emitted_total` または
  `sumeragi_vrf_reveals_emitted_total` がフラットな場合、Torii が VRF frames を
  転送していないことを示します。バリデータログで `handle_vrf_*` エラーを確認し、
  その後、上記の POST helpers を使って payload を手動送信してください。
- **Unexpected penalties** — `sumeragi_vrf_no_participation_total` が急増する場合、
  `vrf_penalties_<epoch>.json` で signer ID を確認し、staking roster と比較してください。
  chaos drills と一致しないペナルティは、バリデータの clock skew か Torii の replay
  protection を示唆します。該当 peer を修正してから再テストしてください。
- **Evidence ingestion stalls** — `sumeragi_evidence_records_total` が停滞し、chaos
  テストが fault を出している場合は、複数のバリデータで
  `iroha ops sumeragi evidence count` を実行し、`/v2/sumeragi/evidence/count` が CLI
  出力と一致することを確認します。差分があれば SSE/webhook consumers が stale の
  可能性があるため、known-good fixture を再送し、それでもカウンタが増えない場合は
  Torii メンテナにエスカレーションしてください。
