---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 容量の検証 SoraFS
タグ: [SF-2c、受け入れ、チェックリスト]
概要: プロバイダーのオンボーディングを受け入れ、紛争や調停を行うためのチェックリスト。
---

# 容量の検証リスト SoraFS

**ベンタナの改訂:** 2026-03-18 -> 2026-03-24  
**プログラムの責任者:** ストレージ チーム (`@storage-wg`)、ガバナンス評議会 (`@council`)、財務ギルド (`@treasury`)  
**Alcance:** プロバイダーのオンボーディングのパイプライン、GA SF-2c におけるテゾレリア要求の調停手続きおよび紛争処理のフルホス。

オペラドールの外部の市場でのハビリタールのチェックリストを見直します。決定的な証拠 (テスト、記録、記録) を参照して、再現性を確認することができます。

## 受け入れチェックリスト

### プロバイダーのオンボーディング

|チェケオ |検証 |証拠 |
|------|-----------|----------|
| El レジストリは、capacidad の正規宣言を受け入れます。アプリ API 経由で `/v1/sorafs/capacity/declare` を統合テストし、企業のマネージャを検証し、メタデータをキャプチャし、レジストリからハンドオフします。 | `crates/iroha_torii/src/routing.rs:7654` |
| El スマート コントラクトの rechaza ペイロードの塩基配列 |プロバイダーの ID をテストし、GIB のコンプロメティドが一致して宣言会社が継続的に保持されます。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI はオンボーディングの正規のアーティファクトを生成します。 CLI のハーネスは、Norito/JSON/Base64 の確定的な検証ラウンドトリップをオフラインでのオペランドの準備宣言として記述します。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|オペラドールの入場とゴベルナンザのガードレールのキャプチャ |宣言の列挙と宣言の文書化、政策のデフォルトと議会の改訂の延期。 | `../storage-capacity-marketplace.md` |

### 紛争解決

|チェケオ |検証 |証拠 |
|------|-----------|----------|
|ペイロードのキャノニコ ダイジェストに関する紛争の記録が持続します。紛争に関する登録のテスト、ペイロードの解読、および台帳の確定を保証するための確定申告を行います。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI の紛争の生成は、カノニコの問題と一致します。 Base64/Norito と履歴書 JSON パラ `CapacityDisputeV1` の CLI テストを実行し、証拠バンドルを形式的に決定します。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|論争/罰則の決定論を再実行するためのテスト |証明失敗再現のテレメトリは、台帳と同一のスナップショットを生成し、ピア間の決定を監視します。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
|取り消しおよびエスカラミエントに関する運用手順書の文書 |評議会での操作のキャプチャ、証拠の要求、ロールバックの手順。 | `../dispute-revocation-runbook.md` |

### テソレリア会議

|チェケオ |検証 |証拠 |
|------|-----------|----------|
| 30 日間の実績と元帳の累積は一致します。プロバイダーは 30 ベンタナ デ 決済、コンパランド エントラダス デル 帳簿と支払いエスペラダの照合テストを行ってください。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|輸出登録簿の調停 | `capacity_reconcile.py` は、XOR 出力の料金台帳の期待値を比較し、Alertmanager を介してメトリクス Prometheus を発行します。 | `scripts/telemetry/capacity_reconcile.py:1`、`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`、`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|請求指数の罰則とテレメトリの蓄積を示すダッシュボード | Grafana グラフィックの蓄積は GiB 時間で行われ、オンコールで担保に保たれたストライキとコンタドールが表示されます。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|リプレイ手法やコマンドの公開アーカイブを報告します。詳細なレポートや、視聴中の出力コマンド、フックの観察などを行います。 | `./sf2c-capacity-soak.md` |

## 排出に関する注意事項

承認前の検証スイートの確認:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

オンボーディング/ディスピュータ コン `sorafs_manifest_stub capacity {declaration,dispute}` およびアーカイブ バイト JSON/Norito の結果は、政府のチケットの取得に役立ちます。

## 悪用技術|アーティファクト |ルタ |ブレイク2b-256 |
|----------|------|---------------|
|プロバイダーのオンボーディングに関するパッケージ | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|紛争解決の報告書 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|テソレリアの調停に関する報告書 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

リリースやレジストリの登録など、さまざまなアーティファクトを含む企業のコピーを管理します。

## アロバシオネス

- ストレージのライダー — @storage-tl (2026-03-24)  
- 統治評議会事務局 — @council-sec (2026-03-24)  
- Lider de Operaciones de Tesoreria — @treasury-ops (2026-03-24)