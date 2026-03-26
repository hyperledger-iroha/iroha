---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: プロビジョナメント・デ・レーン・エラシコ (NX-7)
サイドバーラベル: レーンエラスティコのプロビジョニング
説明: Fluxo のブートストラップパラグラフマニフェスト、レーン Nexus、カタログおよびロールアウトの証拠の記録。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/nexus_elastic_lane.md`。マンテンハ・アンバスは、コピア・アリニャダスとして、ポータルのローカル情報を一掃しました。
:::

# レーンエラスティコのプロビジョニングキット (NX-7)

> **項目のロードマップ:** NX-7 - レーンエラスティックのプロビジョニングツール  
> **ステータス:** ツールは完了しました - ジェラマニフェスト、カタログのスニペット、ペイロード Norito、スモークテスト、
> ロード テストのバンドル ヘルパー、スロットのレイテンシア ゲーティング、および検証用の証拠としてのマニフェストのアゴラ コストラ
> possam ser publicadas sem scripting sob medida。

エステ ギア レバー オペラドール ペロ ノボ ヘルパー `scripts/nexus_lane_bootstrap.sh` は、レーンのマニフェスト、ロールアウトのカタログのスニペット、データスペースの自動化を実行します。 O オブジェクトは、新しいレーン Nexus (公開、公開) のマニュアル編集マニュアルを作成し、ジオメトリのカタログマニュアルの再派生を容易にします。

## 1. 前提条件

1. 別名レーン、データスペース、妥当性検査、ファルハスに対する寛容性 (`f`) および和解の政治に関する政府の政策。
2. 最終的な有効性のリスト (ID の保存) と名前空間のプロテジドのリスト。
3. ノードのノードのアネクサー OS スニペット gerados を構成するリポジトリにアクセスします。
4. レーンのマニフェスト パラオ登録情報 (veja `nexus.registry.manifest_directory` e `cache_directory`)。
5. テレメトリ/ハンドルは、PagerDuty パラオレーン、パラオレーンアラート、オンライン接続を実行します。

## 2. ギア・アルティファト・デ・レーン

リポジトリを実行するヘルパーを実行します。

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

フラグを刻む:

- `--lane-id` 開発者対応者青インデックス da nova entrada em `nexus.lane_catalog`。
- `--dataspace-alias` および `--dataspace-id/hash` は、データスペースのカタログを制御します (米国のデータ レーンを削除します)。
- `--validator` は、`--validators-file` を繰り返します。
- `--route-instruction` / `--route-account` エミテム・レグラ・デ・ロテアメント・プロンタス・パラ・コラー。
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) OS ダッシュボードにある Runbook の内容をキャプチャし、OS 所有者を最も多く記録します。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ランタイム アップグレードのフック、マニフェスト、レーンの要求は、操作性を制御します。
- `--encode-space-directory` ちゃま `cargo xtask space-directory encode` 自動修正。 com `--space-directory-out` を結合して、arquivo `.to` codificado va para outro lugar alem をデフォルトで実行します。

O script produz tres artefatos dentro do `--output-dir` (por Padrao or diretorio atual)、mais um quarto opcional quando o encoding esta habilitado:

1. `<slug>.manifest.json` - レーンコンテインドまたはクォーラムデバリダレスのマニフェスト、名前空間プロテクトおよびメタダドオプションは、ランタイムアップグレードのフックを実行します。
2. `<slug>.catalog.toml` - TOML com `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` のスニペットは、安全な要求を保証します。 `fault_tolerance` は、レーン リレー (`3f+1`) に関する次元のデータスペースのエントリを定義しています。
3. `<slug>.summary.json` - ジオメトリア (スラッグ、セグメント、メタダド) の聴覚に関する問題解決は、`cargo xtask space-directory encode` (em `space_directory_encode.command`) のロールアウト要求とコマンドの実行に役立ちます。 Anexe esse JSON AO チケット デ オンボーディング コモ証拠。
4. `<slug>.manifest.to` - エミディド・クアンド `--encode-space-directory` エスタ・ハビリタド;すぐに `iroha app space-directory manifest publish` を実行し、Torii を実行します。

JSON/スニペットの視覚化に `--dry-run` を使用して、重要な情報を把握し、`--force` に非常に優れた技術が存在します。

## 3. ムダンカスとしてのアップリケ1. `nexus.registry.manifest_directory` 設定のマニフェスト JSON をコピーします (キャッシュ ディレクトリまたはレジストリ エスペルハ バンドル リモートをコピーします)。 Comite o arquivo se マニフェストの sao バージョンには、設定リポジトリがありません。
2. `config/config.toml` のカタログの抜粋 (`config.d/*.toml` は適切ではありません)。 `nexus.lane_count` を確認してください。`lane_id + 1` を確認してください。`nexus.routing_policy.rules` は、新しいレーンにアクセスする必要があります。
3. エンコード (se voce pulou `--encode-space-directory`) 公開、マニフェストなし、スペース ディレクトリ使用、コマンド キャプチャ、要約なし (`space_directory_encode.command`)。ペイロード `.manifest.to` キュー Torii は、監査時の登録証拠として使用されます。 `iroha app space-directory manifest publish`経由の羨望。
4. `irohad --sora --config path/to/config.toml --trace-config` を実行して、ロールアウトのトレース チケットを取得しません。ナメクジ/クラ ゲラドスのセグメントに対応する新しい幾何学模様をすべて発見します。
5. マニフェスト/カタログの目的として、すべての権限を保持します。 Mantenha o 概要 JSON チケットなし、auditoras futuras です。

## 4. レジストリの配布バンドルを管理する

エンパコート、マニフェスト ジェラド、オーバーレイ パラケ オペラドール、ポッサム ディストリビューター、政府管理局、レーン、編集者設定、およびホストを管理します。 O ヘルパー デ バンドル コピア マニフェスト パラオ レイアウト カノニコ、製品オーバーレイ カタログ `nexus.registry.cache_directory` をオフラインで送信するための tarball を送信します。

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

サイダス:

1. `manifests/<slug>.manifest.json` - `nexus.registry.manifest_directory` 設定のパラメータをコピーします。
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` と一致します。 `--module` は、モジュール プラグアベルの定義、`config.toml` の編集用キャッシュのオーバーレイ (NX-2) モジュールのスワップアウトを許可します。
3. `summary.json` - ハッシュを含み、メタデータはオペランドのオーバーレイを指示します。
4. オプション `registry_bundle.tar.*` - SCP、S3 およびトラッカー デ アートのプロント。

Torii のレジストリのディレクトリ (アーカイブ) を同期し、エアギャップ電子コピー OS マニフェストとオーバーレイ キャッシュのホストを追加します。

## 5. バリデーションの煙テスト

Torii を再発行し、レーン レポート `manifest_ready=true` の煙の検証を実行し、感染経路の感染状況を説明し、密封されたエスタ リンポのゲージを測定します。 Lanes que exigem は、devem expor um `manifest_path` nao vazio をマニフェストします。 o ヘルパー アゴラ ファルハ 即時実行 または カミーニョ ファルタ パラケ カダ デプロイ NX-7 には、マニフェスト アサシナドの証拠が含まれています:

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

Adicione `--insecure` ao テスター アンビエンテスの自己署名。おおスクリプトサイコムコディゴナオゼロセオレーンエスティバーオーセンテ、封印された私たちはメトリカス/テレメトリアディベルギレムドスバロールエスペラドス。 OS ノブ `--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog`、`--max-headroom-events` を使用して、テレメトリ ポート レーン (ブロック/ファイナル/バックログ/ヘッドルーム) を制御し、操作を制限します。 `--max-slot-p95` / `--max-slot-p99` (`--min-slot-samples`) は、スロット NX-18 のスロット ヘルパーとして重要です。

エアギャップ (OU CI) の検証を行った後、応答情報を再生成します Torii はエンドポイントのライブアクセスをキャプチャします:

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

OS フィクスチャのグラバドス `fixtures/nexus/lanes/` refletem OS のアーティファト プロダクト ペロ ヘルパー デ ブートストラップ パラ ケ ノボス マニフェストのポッサム サー リンタドス スクリプト作成、泣きながらメディダ。 `ci/check_nexus_lane_smoke.sh` および `ci/check_nexus_lane_registry_bundle.sh` (エイリアス: `make check-nexus-lanes`) を介して CI を実行し、スモーク NX-7 の継続的な互換性を維持するために、ペイロードの形式を保証し、ダイジェスト/オーバーレイをバンドルして再現します。