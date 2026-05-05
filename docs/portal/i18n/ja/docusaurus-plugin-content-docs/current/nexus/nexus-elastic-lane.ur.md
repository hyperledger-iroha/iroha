---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: لچکدار レーン پروویژنگ (NX-7)
サイドバーラベル: レーン پروویژنگ
説明: Nexus レーン マニフェスト、カタログ エントリ、ロールアウト証拠、ブートストラップ、およびブートストラップ
---

:::note 正規ソース
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا، دونوں کاپیوں کو 整列した رکھیں۔
:::

# پروویژنگ ٹول کٹ (NX-7)

> **ロードマップ項目:** NX-7 - レーン پروویژنگ ツール  
> **ステータス:** ツール - マニフェスト、カタログ スニペット、Norito ペイロード、スモーク テスト、テスト
> 負荷テスト バンドル ヘルパー スロット レイテンシ ゲーティング + 証拠マニフェスト 検証バリデータ ロード実行
> スクリプト作成 شائع کیے جا سکیں۔

オペレーター `scripts/nexus_lane_bootstrap.sh` ヘルパー レーン マニフェストの生成 レーン/データスペース カタログ スニペット ロールアウトの証拠やあNexus レーン (パブリック、プライベート) متعدد فائلیں دستی طور پر 編集 کیے بغیر اور カタログ ジオメトリدوبارہ ہاتھ سے کیے بغیر آسانی سے بنائی جا سکیں۔

## 1. 前提条件

1. レーン エイリアス、データスペース、バリデータ セット、フォールト トレランス (`f`)、決済ポリシー、ガバナンスの承認
2. バリデータ (アカウント ID) 保護された名前空間
3. ノード構成リポジトリの生成されたスニペットの作成
4. レーン マニフェスト レジストリ パス (دیکھیں `nexus.registry.manifest_directory` اور `cache_directory`)。
5. レーン テレメトリー連絡先/PagerDuty ハンドル アラート レーン レーン オンライン 有線 有線

## 2. レーンのアーティファクト

リポジトリ ルート ヘルパー:

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

主要なフラグ:

- `--lane-id` کو `nexus.lane_catalog` エントリ インデックス 一致 マッチ ہونا چاہیے۔
- `--dataspace-alias` データスペース `--dataspace-id/hash` データスペース カタログ エントリ コントロール ہیں (デフォルト レーン ID を省略)。
- `--validator` کو 繰り返し کیا جا سکتا ہے یا `--validators-file` سے پڑھا جا سکتا ہے۔
- `--route-instruction` / `--route-account` すぐに貼り付けることができるルーティング ルールは、次のメッセージを出力します。
- `--metadata key=value` (`--telemetry-contact/channel/runbook`) ランブック連絡先のキャプチャ、ダッシュボード、所有者、および所有者
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ランタイム アップグレード フック マニフェスト マネージャ レーン 拡張オペレータ コントロール
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` セキュリティ セキュリティ `.to` セキュリティ デフォルト セキュリティ `.to` セキュリティ セキュリティこ

バージョン `--output-dir` バージョン アーティファクト バージョン (デフォルトのディレクトリ バージョン) エンコーディング バージョン日付:

1. `<slug>.manifest.json` - レーン マニフェスト、バリデーター クォーラム、保護された名前空間、ランタイム アップグレード フック メタデータ、データ
2. `<slug>.catalog.toml` - TOML スニペットの説明 `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]`、ルーティング ルールの説明データスペース エントリ میں `fault_tolerance` لازمی طور پر set کریں تاکہ レーンリレー委員会 (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` - 監査の概要とジオメトリ (スラッグ、セグメント、メタデータ) と必要なロールアウト手順 `cargo xtask space-directory encode` と正確なコマンド (`space_directory_encode.command` の説明) の説明❁❁❁❁オンボーディング チケットを添付する 証拠を添付する
4. `<slug>.manifest.to` - `--encode-space-directory` فعال ہونے پر بنتا ہے؛ Torii `iroha app space-directory manifest publish` フロー準備完了

`--dry-run` JSON/スニペットを表示 `--force` アーティファクトを上書きする

## 3. いいえ

1. マニフェスト JSON が構成された `nexus.registry.manifest_directory` オブジェクト (キャッシュ ディレクトリ、レジストリ リモート バンドル、ミラーリング)。マニフェスト構成リポジトリのバージョン管理とコミットの実行
2. カタログ スニペット کو `config/config.toml` (یا مناسب `config.d/*.toml`) میں append کریں۔ `nexus.lane_count` ٩م از کم `lane_id + 1` ہونا چاہیے اور نئے lan کے لئے `nexus.routing_policy.rules` اپ ڈیٹ کریں۔
3. エンコード (اگر `--encode-space-directory` چھوڑا تھا) スペースディレクトリのマニフェスト公開 کریں۔概要 میں موجود コマンド (`space_directory_encode.command`) استعمال کریں۔ `.manifest.to` ペイロード 監査人 証拠 証拠`iroha app space-directory manifest publish` ذریعے 送信する کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` トレース出力、ロールアウト チケット、アーカイブیہ ثابت کرتا ہے کہ نئی ジオメトリ生成されたスラッグ/Kura セグメント کے مطابق ہے۔
5. マニフェスト/カタログのデプロイ レーンの割り当て バリデータの割り当て 再起動将来の監査の概要 JSON チケットの詳細

## 4. レジストリ配布バンドル生成されたマニフェスト オーバーレイ パッケージ オペレーター ホスト 構成編集 レーン ガバナンス データの配布バンドラー ヘルパー マニフェスト、正規レイアウト、`nexus.registry.cache_directory`、ガバナンス カタログ オーバーレイ、オフライン転送、tarball など日付:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

出力:

1. `manifests/<slug>.manifest.json` - 構成済み `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` عرض المزيد `--module` エントリ プラガブル モジュール定義 プラグ可能モジュール定義 ガバナンス モジュール スワップアウト (NX-2) キャッシュ オーバーレイکرنا پڑتا ہے، `config.toml` میں ترمیم نہیں۔
3. `summary.json` - ハッシュ、オーバーレイ メタデータ、オペレーター指示、メッセージ
4. オプションの `registry_bundle.tar.*` - SCP、S3、アーティファクト トラッカーの準備完了

ディレクトリ (アーカイブ) バリデータ 同期 エアギャップ ホスト 抽出 Torii 再起動 マニフェスト + キャッシュ オーバーレイ レジストリ パスऔर देखें

## 5. バリデータースモークテスト

Torii 再起動 スモーク ヘルパー レーン `manifest_ready=true` メトリクス 予想レーン数 シールド ゲージऔर देखेंレーンのマニフェスト 空でない `manifest_path` 空でない `manifest_path`ヘルパー パス 成功 失敗 NX-7 導入 マニフェスト証拠に署名 成功:

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

自己署名環境 میں ٹیسٹ کرتے وقت `--insecure` شامل کریں۔レーン、シールド、メトリクス/テレメトリの期待値、ドリフト、スクリプト、ゼロ以外の終了、およびスクリプト`--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog`、`--max-headroom-events` レーンレベルのブロック高さ/ファイナリティ/バックログ/ヘッドルームテレメトリーの操作エンベロープ`--max-slot-p95` / `--max-slot-p99` (`--min-slot-samples`) NX-18 スロット期間ターゲット ヘルパーの適用ありがとう

エアギャップ検証 (CI) ライブ エンドポイントのキャプチャ Torii 応答リプレイの確認:

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

`fixtures/nexus/lanes/` 記録されたフィクスチャ、ブートストラップ ヘルパー、アーティファクト、スクリプト、マニフェスト、スクリプト糸くず、糸くず、糸くず、糸くず、糸くず、糸くず、糸くず、糸くずCI フロー `ci/check_nexus_lane_smoke.sh` `ci/check_nexus_lane_registry_bundle.sh` (別名: `make check-nexus-lanes`) NX-7 スモークヘルパー ペイロード形式 مطابق رہتا ہے اور バンドル ダイジェスト/オーバーレイの再現性 ہیں۔