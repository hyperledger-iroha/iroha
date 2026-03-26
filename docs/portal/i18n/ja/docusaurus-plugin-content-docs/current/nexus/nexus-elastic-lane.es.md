---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: プロビジョニングト デ レーン エラシコ (NX-7)
サイドバーラベル: プロビジョニング・アミエント・デ・レーン・エラシコ
説明: ブートストラップの詳細は、レーン Nexus のマニフェスト、カタログおよびロールアウトの証拠の記録です。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/nexus_elastic_lane.md`。 Manten は、ローカルリーグのポータルで、さまざまな情報を共有しています。
:::

# レーンエラスティコのプロビジョニングキット (NX-7)

> **ロードマップの要素:** NX-7 - レーンエラスティックのプロビジョニング用ツール  
> **Estado:** ツールの完成 - 一般マニフェスト、カタログのスニペット、ペイロード Norito、プルエバス、
> ロード テストのバンドル ヘルパーとスロットのレイテンシア ゲート + 検証用のマニフェストの組み合わせ
> メディアのスクリプトを公開する罪。

新しいヘルパー `scripts/nexus_lane_bootstrap.sh` は、レーンのマニフェスト生成の自動化、レーン/データスペースのカタログのスニペット、ロールアウトの証拠を提供します。オブジェクトは、新しいレーン Nexus (公開または公開) を容易にし、複数のアーカイブを編集し、幾何学的なカタログを再提供します。

## 1. 前提条件

1. レーン、データスペース、妥当性検査、寛容性 (`f`) および和解における政治に関する統治に関する問題。
2. 最終的な有効性のリスト (ID の確認) と名前空間の保護のリスト。
3. ノードの構成ファイルのリポジトリにアクセスし、スニペットを保存します。
4. マニフェストの登録に関する規則 (バージョン `nexus.registry.manifest_directory` および `cache_directory`)。
5. テレメトリアへの連絡先/PagerDuty のパラレル モード、オンライン エステの接続方法の処理。

## 2. レーンの加工品の属

リポジトリのヘルパーの取り出し:

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

クラーベの旗:

- `--lane-id` は、`nexus.lane_catalog` の新しいインデックスを一致させます。
- `--dataspace-alias` および `--dataspace-id/hash` は、データスペースのカタログを制御します (米国の欠陥により、安全な情報が提供されません)。
- `--validator` は、`--validators-file` を繰り返します。
- `--route-instruction` / `--route-account` は、ペガーのリストを表示します。
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) ダッシュボードのダッシュボードに関するランブックの連絡先をキャプチャします。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ランタイム アップグレードのアグリガン エル フックは、マニフェスト クアンド エル レーンを必要とし、オペラドールの拡張を制御します。
- `--encode-space-directory` は `cargo xtask space-directory encode` を自動的に呼び出します。 `--space-directory-out` は、デフォルトのアーカイブ `.to` を結合します。

El スクリプトは、`--output-dir` (実際のディレクトリの欠陥) を生成し、オプションのエンコーディングを実行できます。

1. `<slug>.manifest.json` - 有効性の維持およびクォーラムのマニフェスト、ランタイム アップグレードのフックの名前空間プロテギドおよびメタデータのオプション。
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` に関するスニペット TOML は、適切な規制を要求します。 `fault_tolerance` は、レーン リレー (`3f+1`) のパラディメンション データスペースのエントリを設定します。
3. `<slug>.summary.json` - `cargo xtask space-directory encode` (bajo `space_directory_encode.command`) のジオメトリ (スラッグ、セグメント、メタデータ) のロールアウト要求と正確なコマンドの説明を再開します。オンボーディングに関する証拠として JSON チケットを追加します。
4. `<slug>.manifest.to` - エミディド・クアンド `--encode-space-directory` エスタ・アクティバド; `iroha app space-directory manifest publish` と Torii のリスト。

米国 `--dry-run` には JSON/スニペットの事前視覚化が保存されており、`--force` には保存されたアーティファクトが存在します。

## 3. アプリカ・ロス・カンビオス1. マニフェスト JSON と `nexus.registry.manifest_directory` の構成をコピーします (ディレクトリ キャッシュとレジストリ リフレクションとリモート バンドル)。 Commitea el archive si los マニフェストのバージョンと全体的なリポジトリの構成を示します。
2. `config/config.toml` のカタログ スニペットの付録 (`config.d/*.toml` 対応)。 Asegura que `nexus.lane_count` sea al menos `lane_id + 1` yactualiza cualquier `nexus.routing_policy.rules` que deba apuntar al nuevo LANE。
3. Codifica (省略 `--encode-space-directory`) およびスペース ディレクトリの公開マニフェストとコマンド キャプチャの概要 (`space_directory_encode.command`)。 Esto は、ペイロード `.manifest.to` que Torii を生成し、監査上の証拠として登録します。エンビアコン`iroha app space-directory manifest publish`。
4. Ejecuta `irohad --sora --config path/to/config.toml --trace-config` は、ロールアウトのトレースとチケットのアーカイブを保存します。エスト・プルエバ・ケ・ラ・ヌエバの幾何学的形状は、ナメクジ/セグメント・デ・クラ・ジェネラドスと一致します。
5. マニフェスト/カタログの表示は、すべてのレーンに表示されます。 Conserva el summary JSON en el ticket para futuras audiotorias。

## 4. 登録バンドルの配布を構成する

エンパケタ エル マニフェスト ジェネラドとエル オーバーレイ パラ ケ ロス オペラドーレス プエダン ディストリビュール データ ゴベルナンザ デ レーン 編集設定とホストをホストします。バンドル コピアのヘルパーは、レイアウト カノニコのマニフェストをバンドルし、`nexus.registry.cache_directory` パラメタのオーバーレイ オプション カタログを生成し、オフラインでの転送パラレルの送信を実行します。

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

サリダス:

1. `manifests/<slug>.manifest.json` - `nexus.registry.manifest_directory` 設定をコピーします。
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` を参照してください。 `--module` は、編集可能なモジュールの定義、編集可能な `config.toml` のキャッシュのオーバーレイ (NX-2) の実際のスワップアウトの使用を可能にします。
3. `summary.json` - ハッシュ、オーバーレイのメタデータ、およびオペランドの命令が含まれます。
4. オプション `registry_bundle.tar.*` - SCP、S3 のトラッカー デ アーティファクトのリスト。

Torii の記録ファイル (アーカイブ) とディレクトリの同期、エアギャップおよびコピア ロス マニフェストのホストの追加 + オーバーレイ キャッシュ。

## 5. バリダドレスのプルエバス

Despues de reiniciar Torii、Ejecuta el nuevo helper de steam para verificar que el LANE report `manifest_ready=true`、que las metricas expongan el conteo esperado de LANES y que elgue de sealed este limpio。 Las lanes que requieren は、deben exponer un `manifest_path` no vacio を明示します。エル・ヘルパー・アホラ・デ・インメディアト・クアンド・ファルタ・ラ・ルータ・パラ・ケ・カダ・デスリーグNX-7には、マニフェスト・ファームードの証拠が含まれています:

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

Agrega `--insecure` cuando pruebes entornos con certificados は自己署名されています。エル スクリプト ターミナル コン コディゴ ノー セロ レーン ファルタ、エスタ シールされたオ ラス メトリカス/テレメトリア セ デス ヴァローレス エスペラード。米国ロスノブ `--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog` および `--max-headroom-events` テレメトリアポーレーン (ブロック/ファイナル/バックログ/ヘッドルーム) の管理および組み合わせの制限`--max-slot-p95` / `--max-slot-p99` (mas `--min-slot-samples`) スロット NX-18 のオブジェクトを無効にし、ヘルパーを支援します。

パラ検証エアギャップ (o CI) は、エンドポイントの生体内でのエンドポイント Torii のキャプチャを再現します。

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

ロス フィクスチャ グラバドス バホ `fixtures/nexus/lanes/` reflejan ロス アーティファクト プロダクト ポル エル ヘルパー デ ブートストラップ パラ ケ ロス ヌエボス マニフェスト セ プエダン 糸くず耳罪スクリプト作成メディア。 `ci/check_nexus_lane_smoke.sh` y `ci/check_nexus_lane_registry_bundle.sh` (エイリアス: `make check-nexus-lanes`) での CI 出力は、スモーク NX-7 のデモストラル クエリ ヘルパーとペイロードの公開形式の公開と、ダイジェスト/オーバーレイのバンドル セキュアを介して実行されます。まんてんがん再現。