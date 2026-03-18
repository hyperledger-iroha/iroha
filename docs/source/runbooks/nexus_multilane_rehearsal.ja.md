---
lang: ja
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

# Nexus マルチレーン・ローンチリハーサル運用ランブック

このランブックは Phase B4 の Nexus リハーサルをガイドします。ガバナンスが
承認した `iroha_config` バンドルとマルチレーン genesis マニフェストが、
テレメトリ・ルーティング・ロールバックドリルにおいて決定論的に動くことを
検証します。

## スコープ

- 3 レーン（`core`, `governance`, `zk`）を Torii の混合入力（トランザクション、
  コントラクトデプロイ、ガバナンス操作）で実行し、署名済みワークロード seed
  `NEXUS-REH-2026Q1` を使用。
- B4 受け入れに必要なテレメトリ/トレースアーティファクトを収集
  （Prometheus scrape、OTLP export、構造化ログ、Norito admission traces、RBC メトリクス）。
- dry‑run の直後に `B4-RB-2026Q1` ロールバックドリルを実施し、単一レーン
  プロファイルがクリーンに再適用されることを確認。

## 事前条件

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` が GOV-2026-03-19
   承認（署名済みマニフェスト + reviewer イニシャル）を反映している。
2. `defaults/nexus/config.toml`（sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`,
   `nexus.enabled = true` を内包）と `defaults/nexus/genesis.json` が承認済み
   ハッシュと一致し、`kagami genesis bootstrap --profile nexus` が同一 digest を
   返す。
3. レーンカタログが承認された 3 レーン構成に一致し、
   `irohad --sora --config defaults/nexus/config.toml` が Nexus ルーターバナーを
   出力する。
4. マルチレーン CI がグリーン: `ci/check_nexus_multilane_pipeline.sh`
   （`.github/workflows/integration_tests_multilane.yml` 経由で
   `integration_tests/tests/nexus/multilane_pipeline.rs` を実行）と
   `ci/check_nexus_multilane.sh`（router カバレッジ）が両方成功し、Nexus
   プロファイルが multi‑lane ready（`nexus.enabled = true`、Sora カタログハッシュ
   不変、`blocks/lane_{id:03}_{slug}` にレーンストレージ、merge logs 準備済み）。
   defaults バンドル変更時はアーティファクト digest を tracker に記録する。
5. Nexus メトリクス用のダッシュボード/アラートがリハーサル用 Grafana フォルダに
   インポート済みで、アラートルートがリハーサル用 PagerDuty に向いている。
6. Torii SDK のレーン設定がルーティングポリシー表に準拠し、ローカルで
   リハーサルワークロードを再生できる。

## タイムライン概要

| フェーズ | 目標期間 | 担当 | 完了条件 |
|-------|---------------|----------|---------------|
| 準備 | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed 公開、ダッシュボード準備、リハーサルノード構築済み。 |
| Staging freeze | Apr 8 2026 18:00 UTC | @release-eng | config/genesis のハッシュ再検証、freeze 通知送信。 |
| 実行 | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | チェックリスト完了・ブロッキングなし・テレメトリパック保存。 |
| ロールバックドリル | 実行直後 | @sre-core | `B4-RB-2026Q1` 完了・ロールバックテレメトリ取得。 |
| 振り返り | Apr 15 2026 期限 | @program-mgmt, @telemetry-ops, @governance | Retro/学びのドキュメント + ブロッカートラッカー公開。 |

## 実行チェックリスト（Apr 9 2026 15:00 UTC）

1. **コンフィグアテステーション** — 各ノードで `iroha_cli config show --actual`。
   tracker エントリのハッシュと一致することを確認。
2. **レーン warm‑up** — seed ワークロードを 2 スロット再生し、
   `nexus_lane_state_total` が 3 レーンすべてで活動を示すことを確認。
3. **テレメトリ収集** — Prometheus `/metrics` スナップショット、OTLP パケットサンプル、
   Torii 構造化ログ（レーン/データスペースごと）、RBC メトリクスを記録。
4. **ガバナンスフック** — ガバナンストランザクションのサブセットを実行し、
   レーンルーティングとテレメトリタグを確認。
5. **インシデントドリル** — レーン飽和を計画通りに模擬し、アラート発火と応答ログを確認。
6. **ロールバックドリル `B4-RB-2026Q1`** — 単一レーンプロファイルを適用し、
   ロールバックチェックリストを再実行、テレメトリを回収後に Nexus バンドルを再適用。
7. **アーティファクトアップロード** — テレメトリパック、Torii トレース、ドリルログを
   Nexus エビデンスバケットへ保存し、`docs/source/nexus_transition_notes.md` にリンク。
8. **マニフェスト/検証** — `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` を実行して
   `telemetry_manifest.json` + `.sha256` を生成し、リハーサルの tracker エントリに添付。
   ヘルパーはスロット境界を整数として正規化し、必要なヒントが欠けると即時失敗するため、
   ガバナンスアーティファクトの決定論性が保たれる。

## 出力

- 署名済みリハーサルチェックリスト + インシデントドリルログ。
- テレメトリパック（`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`）。
- 検証スクリプト生成のテレメトリマニフェスト + digest。
- ブロッカー/対策/担当をまとめた振り返りドキュメント。

## 実行サマリー — Apr 9 2026

- リハーサルは 15:00 UTC–16:12 UTC に `NEXUS-REH-2026Q1` で実施。
  3 レーンは ~2.4k TEU/slot を維持し、`nexus_lane_state_total` で
  バランスした envelopes を確認。
- テレメトリパックは `artifacts/nexus/rehearsals/2026q1/` にアーカイブ
  （`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log,
  rollback 証跡を含む）。チェックサムは
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md` に記録。
- ロールバックドリル `B4-RB-2026Q1` は 16:18 UTC に完了。単一レーンプロファイルは
  6m42s で再適用され、レーン停止は無し。その後、テレメトリ確認後に Nexus を再有効化。
- レーン飽和インシデント（slot 842 の headroom clamp 強制）で想定通りのアラートが発火。
  mitigation playbook により 11m でページ終了、PagerDuty タイムラインを記録。
- ブロッカーは無し。フォローアップ（TEU headroom ログ自動化、テレメトリパック検証スクリプト）は
  Apr 15 の retro で追跡。

## エスカレーション

- ブロッキングインシデントやテレメトリ退行はリハーサルを停止し、4 営業時間以内に
  ガバナンスへエスカレーション。
- 承認済み config/genesis バンドルからの逸脱があれば再承認後にリハーサルをやり直す。

## テレメトリパック検証（完了）

各リハーサル後に `scripts/telemetry/validate_nexus_telemetry_pack.py` を実行し、
Prometheus export、OTLP NDJSON、Torii 構造化ログ、rollback log の各アーティファクトが
揃っていることを証明し、SHA‑256 digest を記録します。ヘルパーは
`telemetry_manifest.json` と対応する `.sha256` を書き出すため、ガバナンスは
retro パケット内でエビデンスハッシュを引用できます。

Apr 9 2026 のリハーサルでは、検証済みマニフェストが
`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` にあり、digest は
`telemetry_manifest.json.sha256` です。公開時に両方を tracker エントリに添付してください。

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

CI では `--require-slot-range` / `--require-workload-seed` を使って、
注釈漏れのアップロードをブロックしてください。追加アーティファクト（例: DA レシート）が必要な
場合は `--expected <name>` を使います。
