---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Validation du Marche de Capacite SoraFS
タグ: [SF-2c、受け入れ、チェックリスト]
概要: チェックリストは、プロバイダーのオンボーディングの受諾契約、訴訟の流動性および和解の条件、および一般的な行政政策に関する承認 SoraFS を満たしています。
---

# キャパシテ市場での検証チェックリスト SoraFS

**フェネトレ レビュー:** 2026-03-18 -> 2026-03-24  
**プログラムの責任者:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**担当者:** プロバイダーのオンボーディング、訴訟の判決および GA SF-2c に必要な和解プロセスのパイプライン。

La checklist ci-dessous doit etre review avant d'activer le Marche pour des Operators externes。 Chaque ligne renvoie と une 証拠を決定する (テスト、備品または文書) que les Auditeurs peuvent rejouer。

## 受け入れチェックリスト

### プロバイダーのオンボーディング

|チェック |検証 |証拠 |
|------|-----------|----------|
|レジストリは、canoniques de capacite の宣言を受け入れます。アプリ API 経由で `/v1/sorafs/capacity/declare` の統合テストを実行し、署名の検証、メタデータのキャプチャ、ハンドオフとレジストリの確認を行います。 | `crates/iroha_torii/src/routing.rs:7654` |
|ペイロードの一貫性がないスマート コントラクトの拒否 | GiB は、プロバイダーおよびチャンピオンの ID を保証するためのテストを行い、宣言署名者と前衛的な永続性を備えた特派員を派遣します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artefacts d'onboarding canonique | CLI の安全性 Norito/JSON/Base64 は、オフラインでの宣言の作成者とのラウンドトリップを決定および有効にします。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|入学および行政管理のワークフローを管理するガイド |宣言スキーマのドキュメント、ポリシーのデフォルト、評議会のレビューの作成などのドキュメントが含まれています。 | `../storage-capacity-marketplace.md` |

### 訴訟の解決

|チェック |検証 |証拠 |
|------|-----------|----------|
|ペイロードの正規のダイジェストの永続的な記録の登録 |法定単位をテストし、ペイロードの在庫をデコードし、台帳の保証を保証する保留中の法令を確認します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI に対応する正規スキーマの生成 | Base64/Norito をテストして、`CapacityDisputeV1` を注ぐ JSON を再開します。確実な証拠バンドルが確実に確定します。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|裁判/罰則の決定性を証明するための再生テスト |証拠のテレメトリは、スナップショットのスナップショットを識別し、台帳、信用、訴訟を特定し、ピアを決定します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
|エスカレードと失効の流動性に関するランブック文書 |操作ガイドでは、会議のワークフロー、事前の緊急事態、ロールバックの手順を説明します。 | `../dispute-revocation-runbook.md` |

### トレゾアの和解

|チェック |検証 |証拠 |
|------|-----------|----------|
| 30 時間の予測に対応する元帳の蓄積 |プロバイダーを 30 フェネトル デ 決済でテストし、支払いの参照と台帳の比較を行います。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|台帳の輸出と登録簿の調整 | `capacity_reconcile.py` 手数料元帳補助エクスポートの注意事項を比較します。XOR が実行され、メトリックス Prometheus と Alertmanager 経由での承認のゲートが実行されます。 | `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|罰金やテレメトリの蓄積を明らかにする請求のダッシュボード | Grafana のインポート Grafana の累積 GiB 時間、ストライキのコンピューティングおよび担保の実行は、オンコールでの訪問を可能にします。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|浸漬方法論と再生コマンドの報告書アーカイブ |密接な関係を築き、実行の命令と監査を監視するフックを作成します。 | `./sf2c-capacity-soak.md` |

## 実行時のメモ

承認前に一連の検証を行う:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

オンボーディング/リティージ アベニュー `sorafs_manifest_stub capacity {declaration,dispute}` およびアーカイバ ファイルのバイト JSON/Norito の結果を管理するためのチケット コートを作成する操作者です。

## 承認の成果物|アーティファクト |パス |ブレイク2b-256 |
|----------|------|---------------|
|プロバイダーのオンボーディングに対する承認のパケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|訴訟の解決に対する承認のパケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|トレゾルに対する和解承認のパケット | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

署名者と成果物のコピーを保存し、リリースおよび管理上の変更登録を保存します。

## 承認

- ストレージ チーム リーダー — @storage-tl (2026-03-24)  
- ガバナンス評議会書記 — @council-sec (2026-03-24)  
- 財務業務責任者 — @treasury-ops (2026-03-24)