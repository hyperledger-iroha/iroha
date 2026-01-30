---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b40987df65bd6e3d5198170956b330d744839f837912e458ae266d0e7b89bd6d
source_last_modified: "2025-11-14T04:43:20.406733+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正本ソース
このページは `docs/source/nexus_elastic_lane.md` を反映しています。翻訳作業がポータルに反映されるまで、両方のコピーを同期してください。
:::

# エラスティックレーン・プロビジョニング・ツールキット (NX-7)

> **ロードマップ項目:** NX-7 - エラスティックレーンのプロビジョニングツール  
> **ステータス:** ツールは完了済み - マニフェスト、カタログ断片、Norito ペイロード、スモークテストを生成し、
> ロードテスト用バンドルヘルパーがスロット遅延のゲーティング + 証跡マニフェストを結合するため、
> バリデータ負荷試験を専用スクリプトなしで公開できます。

このガイドでは、新しい `scripts/nexus_lane_bootstrap.sh` ヘルパーを使って、レーンマニフェスト生成、lane/dataspace カタログの断片、ロールアウト証跡を自動化する手順を説明します。目的は、複数ファイルを手編集したり、カタログのジオメトリを再計算したりせずに、新しい Nexus レーン（public/private）を簡単に立ち上げられるようにすることです。

## 1. 前提条件

1. レーン alias、dataspace、バリデータ集合、故障許容度 (`f`)、settlement ポリシーに対するガバナンス承認。
2. 確定したバリデータ一覧（アカウント ID）と保護対象 namespace 一覧。
3. 生成された断片を追加できるノード設定リポジトリへのアクセス。
4. レーンマニフェストのレジストリパス（`nexus.registry.manifest_directory` と `cache_directory` を参照）。
5. レーン用のテレメトリ連絡先/PagerDuty ハンドル（オンライン化直後にアラートを接続できるようにする）。

## 2. レーン成果物を生成する

リポジトリのルートからヘルパーを実行します。

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

主なフラグ:

- `--lane-id` は `nexus.lane_catalog` の新しいエントリの index と一致させます。
- `--dataspace-alias` と `--dataspace-id/hash` は dataspace のカタログエントリを制御します（省略時は lane id を使用）。
- `--validator` は複数回指定するか、`--validators-file` から読み込めます。
- `--route-instruction` / `--route-account` は貼り付け可能なルーティングルールを生成します。
- `--metadata key=value`（または `--telemetry-contact/channel/runbook`）で runbook の連絡先を記録し、ダッシュボードに正しいオーナーを表示できます。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` は、レーンが拡張オペレーター制御を必要とする場合に runtime-upgrade フックをマニフェストへ追加します。
- `--encode-space-directory` は `cargo xtask space-directory encode` を自動実行します。`.to` を既定の場所以外に出す場合は `--space-directory-out` を併用してください。

スクリプトは `--output-dir`（既定はカレントディレクトリ）に 3 つの成果物を作成し、エンコードを有効にした場合は 4 つ目も生成します。

1. `<slug>.manifest.json` - バリデータ quorum、保護 namespace、runtime-upgrade フックの任意メタデータを含むレーンマニフェスト。
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]`、要求したルーティングルールを含む TOML 断片。dataspace エントリに `fault_tolerance` を設定して lane-relay 委員会（`3f+1`）をサイズ設定してください。
3. `<slug>.summary.json` - ジオメトリ（slug、セグメント、メタデータ）、必要なロールアウト手順、`cargo xtask space-directory encode` の正確なコマンド（`space_directory_encode.command`）を含む監査用サマリ。オンボーディングチケットに添付して証跡にします。
4. `<slug>.manifest.to` - `--encode-space-directory` を指定した場合に出力され、Torii の `iroha app space-directory manifest publish` フローでそのまま利用できます。

`--dry-run` でファイルを書かずに JSON/断片を確認し、`--force` で既存成果物を上書きします。

## 3. 変更を適用する

1. マニフェスト JSON を `nexus.registry.manifest_directory` にコピーします（レジストリがリモートバンドルをミラーする場合は cache directory にも）。設定リポジトリでマニフェストをバージョン管理している場合はコミットします。
2. カタログ断片を `config/config.toml`（または適切な `config.d/*.toml`）に追加します。`nexus.lane_count` が `lane_id + 1` 以上であることを確認し、新しいレーンに向けるべき `nexus.routing_policy.rules` を更新します。
3. （`--encode-space-directory` を省略した場合）エンコードして Space Directory にマニフェストを公開します。summary に記録されたコマンド（`space_directory_encode.command`）を使用してください。これにより Torii が要求する `.manifest.to` ペイロードが生成され、監査証跡も残ります。`iroha app space-directory manifest publish` で送信します。
4. `irohad --sora --config path/to/config.toml --trace-config` を実行し、トレース出力をロールアウトチケットに保存します。これで新しいジオメトリが生成された slug/Kura セグメントと一致することを示せます。
5. マニフェスト/カタログ変更が展開されたら、レーンに割り当てられたバリデータを再起動します。将来の監査用に summary JSON をチケットに保持してください。

## 4. レジストリ配布バンドルを作成する

生成したマニフェストとオーバーレイをパッケージ化し、各ホストで設定を編集せずにレーンガバナンスデータを配布できるようにします。バンドラーヘルパーはマニフェストを正規レイアウトへコピーし、`nexus.registry.cache_directory` 用のガバナンスカタログオーバーレイを生成し、オフライン転送用の tarball も出力できます。

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

出力:

1. `manifests/<slug>.manifest.json` - これらを `nexus.registry.manifest_directory` に配置します。
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` に配置します。各 `--module` エントリが差し替え可能なモジュール定義になり、`config.toml` を編集せずにキャッシュオーバーレイを更新することでガバナンスモジュールの入れ替え（NX-2）が可能になります。
3. `summary.json` - ハッシュ、オーバーレイメタデータ、オペレーター向け手順を含みます。
4. 任意の `registry_bundle.tar.*` - SCP、S3、アーティファクトトラッカーにそのまま送れます。

ディレクトリ全体（またはアーカイブ）を各バリデータへ同期し、air-gapped ホストで展開してから、マニフェスト + キャッシュオーバーレイをレジストリパスへコピーし、Torii を再起動します。

## 5. バリデータのスモークテスト

Torii 再起動後、新しいスモークヘルパーを使って、レーンが `manifest_ready=true` を報告すること、メトリクスが期待されるレーン数を公開していること、sealed ゲージがクリアであることを確認します。マニフェストが必要なレーンは空でない `manifest_path` を公開する必要があり、ヘルパーはパスがない場合に即時失敗します。これにより NX-7 の各デプロイに署名済みマニフェストの証跡が含まれます。

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

自己署名環境では `--insecure` を追加してください。レーンが存在しない、sealed 状態、またはメトリクス/テレメトリが期待値から逸脱している場合、スクリプトは非 0 で終了します。`--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog`、`--max-headroom-events` を使って lane ごとのブロック高/ファイナリティ/バックログ/ヘッドルームのテレメトリを運用範囲内に保ち、`--max-slot-p95` / `--max-slot-p99`（および `--min-slot-samples`）と組み合わせて NX-18 のスロット時間ターゲットをヘルパー内で達成してください。

air-gapped 検証（または CI）では、ライブのエンドポイントにアクセスする代わりに、キャプチャした Torii 応答を再生できます。

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` に保存されたフィクスチャは bootstrap ヘルパーが生成する成果物を反映しており、専用スクリプトなしで新しいマニフェストを lint できます。CI は `ci/check_nexus_lane_smoke.sh` と `ci/check_nexus_lane_registry_bundle.sh`（エイリアス: `make check-nexus-lanes`）で同じフローを実行し、NX-7 のスモークヘルパーが公開されたペイロード形式と互換であること、bundle の digest/overlay が再現可能であることを確認します。