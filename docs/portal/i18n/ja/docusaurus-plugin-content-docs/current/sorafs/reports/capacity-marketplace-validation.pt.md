---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Validacao do mercado de capacidade SoraFS
タグ: [SF-2c、受け入れ、チェックリスト]
概要: プロバイダーのオンボーディングを確認するチェックリスト、紛争と和解を行うためのチェックリスト、ディスポニビリダードの管理、管理 SoraFS。
---

# 容量の有効性を確認するチェックリスト SoraFS

**ジャネラ デ レビュー:** 2026-03-18 -> 2026-03-24  
**プログラムへの対応:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**Escopo:** プロバイダーのオンボーディングのパイプライン、GA SF-2c の要求事項の調整のための調整プロセスの処理。

チェックリストは、ハビリタールの安全性を確認するためのチェックリストです。確実な証拠の確定 (テスト、フィクスチャー、または文書化) は、監査結果を再現します。

## 安全チェックリスト

### プロバイダーのオンボーディング

|チェカジェム |バリダカオ |証拠 |
|------|-----------|----------|
| O レジストリ aceita declaracoes canonicas de capacidade |アプリ API 経由で `/v1/sorafs/capacity/declare` を実行し、操作の検証、レジストリ ノードからのメタデータの取得、ハンドオフのキャプチャを実行します。 | `crates/iroha_torii/src/routing.rs:7654` |
|スマート コントラクトのペイロードが発散する |プロバイダーの ID を保証するために、GiB との互換性を確認し、継続的な対応を宣言してください。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI はオンボーディングの正規のアーティファトを発行します。 CLI のハーネスは、Norito/JSON/Base64 の確定性を利用して、オフラインでの操作の準備と宣言の往復を検証します。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|行政管理のワークフローやガードレールを管理するための操作 |宣言のスキーマの列挙、デフォルトの政策および議会の見直しに関する文書。 | `../storage-capacity-marketplace.md` |

### 紛争解決

|チェカジェム |バリダカオ |証拠 |
|------|-----------|----------|
|レジストロス デ ディスピュタ パーシスタンス コム ダイジェスト カノニコ ドゥ ペイロード | O teste unitarioregistra uma disputa、decodifica o payload armamazenado、afirma status pending para garantir determinismo do 元帳。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI でスキーマ canonico に対応するための議論を行います。 Base64/Norito と呼ばれる CLI をテストして、`CapacityDisputeV1` に関する JSON を確認し、テナム ハッシュの確定的な証拠バンドルを保証します。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|論争/刑罰の決定論を再現するためのテスト |遠隔測定による証明失敗再現は、スナップショットの台帳の同一性、信用度、および紛争との関係を決定するものです。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo deescalonamento e revogacao |オペラ座のキャプチャ、ワークフローの協議、証拠の要件、ロールバックの手順。 | `../dispute-revocation-runbook.md` |

### 和解してください

|チェカジェム |バリダカオ |証拠 |
|------|-----------|----------|
| 30 日分のプロジェクトに対応する見越した元帳 | 30 月 30 日までに、支払いに関する参照を行う元帳の内容を確認し、プロバイダーを調べてください。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|台帳と登録を輸出するための調整 | `capacity_reconcile.py` は、料金台帳の期待値を比較し、XOR 実行をエクスポートし、メトリクスを出力します。Prometheus は、Alertmanager を介して、ゲート ダ アプロバカオを実行します。 | `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|請求に関するペナルティやテレメトリの発生に関するダッシュボード | O インポートは、発生した GiB 時間の Grafana プロットをインポートし、ストライキのコンタドールと担保付きのパラビジビリダーデをオンコールでインポートします。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|リプレイの方法論やコマンドに関する公開情報を公開します。関係を詳細に調べ、監査を監視しながら実行コマンドを実行します。 | `./sf2c-capacity-soak.md` |

## 実行上の注意事項

サインオフを行う前に検証スイートを再実行します。

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

オンボーディング/ディスピューティング コム `sorafs_manifest_stub capacity {declaration,dispute}` からのオペレーティング システムの再生成ペイロードの開発、および JSON/Norito の結果、政府のチケット管理が行われます。

## アプロバカオの芸術品

|アルテファト |カミーニョ |ブレイク2b-256 |
|----------|------|---------------|
|プロバイダーのオンボーディングに関するパコート | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|紛争解決策の提案 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|調整を行うための準備 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |ガバナンカのレジストリとしてのリリースやビンキュールのコピーとして、アシナダのデセス アートファトを保護します。

## アプロバコス

- ストレージ チーム リーダー - @storage-tl (2026-03-24)  
- ガバナンス評議会書記 - @council-sec (2026-03-24)  
- 財務業務責任者 - @treasury-ops (2026-03-24)