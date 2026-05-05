---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: レーンエラスティックのプロビジョニング (NX-7)
Sidebar_label: レーン弾力性のプロビジョニング
説明: ブートストラップのワークフロー、レーン Nexus のマニフェスト作成、カタログのメイン、ロールアウトの準備。
---

:::note ソースカノニク
Cette ページの再表示 `docs/source/nexus_elastic_lane.md`。 Gardez les deux のコピーは、すべての目的を達成するために、目的地に到着します。
:::

# レーン弾力性のあるキットのプロビジョニング (NX-7)

> **ロードマップの要素:** NX-7 - レーンエラスティックのプロビジョニングツール  
> **ステータス:** ツールの完了 - マニフェストの種類、カタログのスニペット、ペイロード Norito、スモーク テスト、
> バンドルのヘルパーとロードテストのアセンブル、メンテナンス、スロットごとの遅延遅延 + 事前準備のマニフェスト、実行、料金検証
> スクリプトを使用せずに出版することができます。

このガイドは、le nouveau ヘルパー `scripts/nexus_lane_bootstrap.sh` を介してオペレーターに付属し、レーンのマニフェストの生成、カタログ レーン/データスペースのスニペット、およびロールアウトのスニペットを自動化します。新しいレーンの作成を容易にするオブジェクト Nexus (パブリックとプライベート) は、編集者なしでメイン プラスフィッシャーからカタログのメイン ジオメトリを再取得できます。

## 1. 前提条件

1. レーン別名、データスペース、検証のアンサンブル、パンヌの寛容性 (`f`) および和解の政治に対する統治の承認。
2. 検証の最終的なリスト (ID の計算) と名前空間の保護者のリストを作成します。
3. 一般的な情報の構成デポにアクセスします。
4. Chemins は、マニフェストのレジストリを登録します (`nexus.registry.manifest_directory` および `cache_directory`)。
5. 連絡先テレメトリ/PagerDuty は、レーン内でのアラートを維持するための接続を制御します。

## 2. レーンの芸術品の生成

Lancez le helper depuis la racine du depot :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

フラグ cle :

- `--lane-id` は、`nexus.lane_catalog` の新しいメインディッシュのインデックスに対応します。
- `--dataspace-alias` および `--dataspace-id/hash` は、データスペースのカタログのエントリを制御します (デフォルトのまま、レーンの制限はありません)。
- `--validator` は、繰り返し、私は、`--validators-file` を繰り返します。
- `--route-instruction` / `--route-account` ルート管理に関する規則は、収集者に優先されます。
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) は、所有者に関わるダッシュボードの連絡先とランブックをキャプチャします。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` アジョテント ル フック ランタイム アップグレード マニフェスト エリア レーンの制御操作が必要です。
- `--encode-space-directory` 自動呼び出し `cargo xtask space-directory encode`。 Combinez-le avec `--space-directory-out` quand vous voulez que le fichier `.to` encode aille ailleurs que le chemin par defaut.

`--output-dir` によるトロワ アーティファクトのスクリプト作成 (デフォルトのレパートリー クーラントによる)、およびアクティブなコードの四種類のオプション:

1. `<slug>.manifest.json` - レーン コンテンツの検証クォーラム、ネームスペースの保護者およびメタドンのオプション フック ランタイム アップグレードのマニフェスト。
2. `<slug>.catalog.toml` - スニペット TOML 平均 `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` およびルート要求者の規則。 `fault_tolerance` は、レーンリレー (`3f+1`) の寸法を注ぐエントリ データスペースの定義を保証します。
3. `<slug>.summary.json` - ジオメトリ (スラグ、セグメント、メタドン) およびロールアウト要求の監査記録と正確なコマンド `cargo xtask space-directory encode` (`space_directory_encode.command`) の監査再開。 Joignez ce JSON au チケット オンボーディング コム プレウブ。
4. `<slug>.manifest.to` - EMIS Quand `--encode-space-directory` est アクティブ。プレトプールルフラックス`iroha app space-directory manifest publish`デTorii。

`--dry-run` は、JSON/スニペットをサンエクリレ デ フィチエでプレビジュアライザーを注ぎ、`--force` は既存のアーティファクトを注ぎます。

## 3. アップリケの変化1. `nexus.registry.manifest_directory` を構成してマニフェスト JSON をコピーします (キャッシュ ディレクトリやレジストリのミロライト、リモート バンドルなど)。 Committez le fichier sil les は、設定のバージョンと投票リポジトリをマニフェストします。
2. `config/config.toml` のカタログの抜粋 (`config.d/*.toml` は適切です)。 `nexus.lane_count` 街歩き `lane_id + 1` を保証し、1 時間の旅行規則 `nexus.routing_policy.rules` でポインタとヌーベル レーンを比較します。
3. エンコード (`--encode-space-directory` を実行) し、概要をキャプチャするコマンド (`space_directory_encode.command`) を介して、スペース ディレクトリのマニフェストを公開します。 Cela 製品ペイロード `.manifest.to` は Torii に参加し、事前監査を登録します。 `iroha app space-directory manifest publish` 経由のソウメッツ。
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` と、出撃追跡およびチケットのロールアウトのアーカイブ。 Cela prouve que la nouvelle geometrie は、slug Genere の補助セグメントに対応します。
5. Redemarrez les validateurs は、マニフェスト/カタログの展開に合わせて変更を割り当てます。将来の監査のチケットを含む JSON の概要を保存します。

## 4. レジストリのバンドルと配布を構成する

Empaquetez は、マニフェスト ジェネレータとオーバーレイを管理し、配布者が管理者を管理し、編集者なしで設定を管理します。レイアウト標準のバンドル コピー マニフェストのヘルパー、`nexus.registry.cache_directory` のカタログ ド ガバナンスのオーバーレイ オプションの製品、オフラインでの転送ファイルの転送など:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

出撃：

1. `manifests/<slug>.manifest.json` - `nexus.registry.manifest_directory` 構成をコピーします。
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` によるデポセズ。メイン `--module` は、分岐可能なモジュールの定義、モジュール管理 (NX-2) の永続的なスワップアウト、およびキャッシュ プルトットの編集者 `config.toml` の管理者です。
3. `summary.json` - ハッシュ、オーバーレイのメタドン、および命令操作が含まれます。
4. オプション `registry_bundle.tar.*` - SCP、S3 のトラッカー ダーティファクトを準備します。

完全なレパートリー (アーカイブ) とチェックの検証、エアギャップのあるホテルの追加、およびマニフェスト + オーバーレイのキャッシュとルールのレジストリのコピー Torii を同期します。

## 5. 検証のためのスモークテスト

Torii の再婚後、ランセ ル ヌーボー ヘルパーが煙を注ぐ検証者、車線の関係 `manifest_ready=true`、車線の監視の方法、および密封された映像を確認します。 `manifest_path` 非ビデオの公開者に必要なマニフェストの説明。 le helper echoue des qu'il manque le chemin afin que Chaque deploiement NX-7 inclue la preuve du manques :

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

Ajoutez `--insecure` lorsque vous testez des environnements 自己署名。スクリプトは、ゼロ以外のコードを並べ替えて、レーン マンクを作成し、封印され、評価/参加者の評価/統計情報の派生を確認します。ノブ `--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog` および `--max-headroom-events` を使用して、レーン (ブロック/ファイナル/バックログ/ヘッドルーム) ごとにテレメトリのメンテナンスを実行し、運用管理やカップルの管理を実行します。 `--max-slot-p95` / `--max-slot-p99` (および `--min-slot-samples`) は、ヘルパーを終了することなく、スロット NX-18 の目的を課すものです。

エアギャップ検証 (OU CI) を確認して応答 Torii をキャプチャし、エンドポイント ライブで尋問を行います:

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

`fixtures/nexus/lanes/` の備品は、ブートストラップのヘルパーと同様に製品の成果物を登録し、スクリプト作成を必要とせずに新しいマニフェストを作成します。 La CI は、`ci/check_nexus_lane_smoke.sh` および `ci/check_nexus_lane_registry_bundle.sh` (エイリアス: `make check-nexus-lanes`) を介してミーム フラックスを実行し、煙の NX-7 を保持し、ペイロードの公開フォーマットに準拠し、バンドルの保持された複製物のダイジェスト/オーバーレイを保証します。