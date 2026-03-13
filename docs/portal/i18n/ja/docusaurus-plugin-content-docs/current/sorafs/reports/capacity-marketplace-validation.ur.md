---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS 容量マーケットプレイスの検証
概要: 受け入れチェックリスト、プロバイダーのオンボーディング、紛争ワークフロー、財務省の調整、SoraFS 容量マーケットプレイス、GA ゲート、ゲートウェイ
タグ: [SF-2c、受け入れ、チェックリスト]
---

# SoraFS 容量マーケットプレイス検証チェックリスト

**レビュー期間:** 2026-03-18 -> 2026-03-24  
**プログラム所有者:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**範囲:** プロバイダーのオンボーディング パイプライン、紛争裁定フロー、財務省調整プロセス、SF-2c GA の処理。

チェックリスト オペレーター 市場の有効化 レビュー レビュー決定的な証拠 (テスト、設備、文書) の行 監査員のリプレイ チェック チェック

## 受け入れチェックリスト

### プロバイダーのオンボーディング

|チェック |検証 |証拠 |
|------|-----------|----------|
|レジストリの正規の容量宣言統合テスト アプリ API の実行 `/v2/sorafs/capacity/declare` 署名の処理 メタデータのキャプチャ ノード レジストリのハンドオフ 検証| `crates/iroha_torii/src/routing.rs:7654` |
|スマート コントラクトの不一致ペイロードが拒否されました |単体テスト プロバイダー ID コミットされた GiB フィールドの署名済み宣言 テスト 永続性| `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI の正規オンボーディング アーティファクトが生成する | CLI ハーネス決定論的 Norito/JSON/Base64 出力 ラウンドトリップ検証 演算子のオフライン宣言| `crates/sorafs_car/tests/capacity_cli.rs:17` |
|オペレーター ガイド 入場ワークフロー ガバナンス ガードレール 表紙文書宣言スキーマ、ポリシーのデフォルト、評議会レビュー手順の列挙| `../storage-capacity-marketplace.md` |

### 紛争解決

|チェック |検証 |証拠 |
|------|-----------|----------|
|紛争記録の正規ペイロード ダイジェストが永続化されます。単体テストの紛争登録 保存されたペイロードのデコード 保留中のステータスのアサート 台帳決定論 یقینی ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI 紛争ジェネレーターの正規スキーマの一致と一致 | CLI テスト `CapacityDisputeV1` および Base64/Norito 出力 JSON 概要は、決定論的ハッシュをカバーします 証拠バンドル 決定論的ハッシュ| `crates/sorafs_car/tests/capacity_cli.rs:455` |
|リプレイテストの紛争/ペナルティの決定論 |証明失敗テレメトリ、リプレイ、元帳、クレジット、紛争スナップショット、ピアを斬り裂く、確定的決定性| `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
|ランブックのエスカレーションと失効フロー、ドキュメントの作成 |運用ガイド 評議会のワークフロー、証拠要件、ロールバック手順、キャプチャの詳細| `../dispute-revocation-runbook.md` |

### 財務省調整

|チェック |検証 |証拠 |
|------|-----------|----------|
|元帳見越の 30 日間ソーク予測の一致結果 |ソークテスト プロバイダー 30 決済ウィンドウ 台帳エントリ 期待支払額参照 差分| `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|元帳輸出調整記録 ہوتا ہے | `capacity_reconcile.py` 手数料元帳の期待値 実行された XOR 転送エクスポート 比較 評価 Prometheus メトリクスの出力 アラート マネージャー 財務省の承認ゲートありがとうございます| `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|請求ダッシュボードのペナルティと発生テレメトリの画面の表示 | Grafana インポート GiB 時間の見越額、ストライキカウンター、保税担保プロットのインポート オンコールの可視性| `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|公開されたレポート ソーク手法とリプレイ コマンド アーカイブレポート ソーク スコープ、実行コマンド、可観測性フック、監査人、詳細情報| `./sf2c-capacity-soak.md` |

## 実行メモ

サインオフと検証スイートの説明:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

演算子 `sorafs_manifest_stub capacity {declaration,dispute}` オンボーディング/紛争リクエスト ペイロードを生成する 処理を生成する JSON/Norito バイト ガバナンス チケットを生成する٩ے ساتھ アーカイブ کرنا چاہیے۔

## サインオフアーティファクト

|アーティファクト |パス |ブレイク2b-256 |
|----------|------|---------------|
|プロバイダーのオンボーディング承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|紛争解決承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|財務省調整承認パケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

アーティファクトの署名入りコピー リリースバンドルの作成 ガバナンス変更記録の作成## 承認

- ストレージ チーム リーダー — @storage-tl (2026-03-24)  
- ガバナンス評議会書記 — @council-sec (2026-03-24)  
- 財務業務責任者 — @treasury-ops (2026-03-24)