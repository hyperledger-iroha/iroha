---
lang: ja
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS 容量シミュレーション ツールキット

このディレクトリは SF-2c 容量マーケットプレイスシミュレーション用の再現可能な artefacts を提供します。このツールキットは、プロダクション CLI ヘルパーと軽量な解析スクリプトを用いて、クォータ交渉、フェイルオーバー処理、slashing のリメディエーションを実行します。

## 前提条件

- ワークスペースメンバー向けに `cargo run` を実行できる Rust toolchain.
- Python 3.10+ (標準ライブラリのみ).

## クイックスタート

```bash
# 1. 代表的な CLI artefacts を生成
./run_cli.sh ./artifacts

# 2. 結果を集約して Prometheus メトリクスを出力
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` は `sorafs_manifest_stub capacity` を呼び出して以下を構築します:

- クォータ交渉 fixture セットの決定的 provider 宣言.
- 交渉シナリオに一致するレプリケーション順序.
- failover ウィンドウのテレメトリスナップショット.
- slashing 要求を含むディスピュート payload.

スクリプトは Norito bytes (`*.to`)、base64 payloads (`*.b64`)、Torii リクエストボディ、
および可読サマリ (`*_summary.json`) を選択した artefact ディレクトリに書き出します。

`analyze.py` は生成されたサマリを取り込み、集約レポート
(`capacity_simulation_report.json`) を生成し、Prometheus textfile
(`capacity_simulation.prom`) を出力します:

- `sorafs_simulation_quota_*` gauges: 交渉済み容量とプロバイダ別割当シェア.
- `sorafs_simulation_failover_*` gauges: downtime の差分と選択された代替プロバイダ.
- `sorafs_simulation_slash_requested`: ディスピュート payload から抽出されたリメディエーション率.

`dashboards/grafana/sorafs_capacity_simulation.json` の Grafana バンドルをインポートし、
生成された textfile を scrape する Prometheus datasource に接続してください (例: node-exporter の
textfile collector)。`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` の
runbook には、Prometheus 設定のヒントを含む全体フローが記載されています。

## Fixtures

- `scenarios/quota_negotiation/` — provider 宣言仕様とレプリケーション順序.
- `scenarios/failover/` — 主障害と failover リフトのテレメトリウィンドウ.
- `scenarios/slashing/` — 同じレプリケーション順序を参照するディスピュート仕様.

これらの fixtures は `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` で検証され、CLI スキーマと同期していることを保証します。
