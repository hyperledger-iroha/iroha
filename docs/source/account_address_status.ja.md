---
lang: ja
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

## アカウントアドレス・コンプライアンス・ステータス (ADDR-2)

ステータス: 承認 2026-03-30  
オーナー: データモデルチーム / QA ギルド  
ロードマップ参照: ADDR-2 — Dual-Format Compliance Suite

### 1. 概要

- フィクスチャ: `fixtures/account/address_vectors.json` (IH58 (preferred) + compressed (`sora`, second-best) + multisig の正常/異常ケース)。
- スコープ: implicit-default、Local-12、Global registry、multisig controllers を含む決定論的 V1 payloads と、
  完全なエラー分類。
- 配布: Rust data-model、Torii、JS/TS、Swift、Android SDK で共有され、いずれかが逸脱すると CI は失敗。
- 真のソース: 生成器は `crates/iroha_data_model/src/account/address/compliance_vectors.rs` にあり、
  `cargo xtask address-vectors` で公開される。

### 2. 再生成と検証

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

フラグ:

- `--out <path>` — アドホックな bundle を生成する際の任意上書き (既定:
  `fixtures/account/address_vectors.json`)。
- `--stdout` — JSON を stdout に出力し、ディスクに書き込まない。
- `--verify` — 現在のファイルと再生成結果を比較 (差分があれば即失敗; `--stdout` と併用不可)。

### 3. アーティファクト・マトリクス

| 対象 | 施行 | 備考 |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON を解析し、canonical payloads を再構築し、IH58（推奨）/compressed（`sora`、次善）/canonical 変換 + 構造化エラーを検証する。 |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | サーバー側 codec を検証し、Torii が不正な IH58（推奨）/compressed（`sora`、次善） payloads を決定論的に拒否するようにする。 |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | V1 fixtures (IH58 推奨/compressed（`sora`）次善/fullwidth) をミラーし、全ての負ケースで Norito 風のエラーコードを検証する。 |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | IH58（推奨）/compressed（`sora`、次善）のデコード、multisig payloads、Apple プラットフォームでのエラー露出を確認する。 |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Kotlin/Java バインディングが canonical fixture と一致し続けることを確認する。 |

### 4. 監視と残タスク

- ステータス報告: 本ドキュメントは `status.md` と roadmap からリンクされ、週次レビューで
  フィクスチャの健全性を確認できる。
- 開発者ポータル要約: ドキュメントポータルの **Reference -> Account address compliance**
  (`docs/portal/docs/reference/account-address-status.md`) を参照。
- Prometheus とダッシュボード: SDK のコピーを検証するたびに `--metrics-out` (必要なら
  `--metrics-label`) 付きで helper を実行し、Prometheus textfile collector が
  `account_address_fixture_check_status{target=...}` を取り込めるようにする。Grafana ダッシュボード
  **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) は
  各 surface の pass/fail 件数と、監査証跡用の canonical SHA-256 digest を表示する。いずれかの target が
  `0` を返した場合はアラートする。
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` は成功した account literal 解析ごとに
  出力され、`torii_address_invalid_total`/`torii_address_local8_total` をミラーする。本番で
  `domain_kind="local12"` のトラフィックがあればアラートし、カウンタを SRE の `address_ingest`
  ダッシュボードへミラーして Local-12 退役ゲートの監査証跡を残す。
- フィクスチャ helper: `scripts/account_fixture_helper.py` は canonical JSON をダウンロード/検証し、
  SDK リリース自動化が手動コピーなしで bundle を取得/確認できるようにする。必要に応じて
  Prometheus メトリクスも出力できる。例:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  helper は `account_address_fixture_check_status{target="android"} 1` を書き込み、target が一致すると
  `account_address_fixture_remote_info` / `account_address_fixture_local_info` の gauges に SHA-256 digest を
  出力する。ファイルがない場合は `account_address_fixture_local_missing` を報告する。
  Automation wrapper: `ci/account_fixture_metrics.sh` を cron/CI から呼び出して、統合 textfile
  (既定 `artifacts/account_fixture/address_fixture.prom`) を出力する。`--target label=path` を繰り返し渡し、
  (任意で `::https://mirror/...` を target ごとに付与して source を上書き) Prometheus が全ての SDK/CLI copy を
  1 つのファイルで scrape できるようにする。GitHub workflow `address-vectors-verify.yml` は既にこの helper を
  canonical fixture に対して実行し、SRE 取り込み用の `account-address-fixture-metrics` artifact をアップロードする。
