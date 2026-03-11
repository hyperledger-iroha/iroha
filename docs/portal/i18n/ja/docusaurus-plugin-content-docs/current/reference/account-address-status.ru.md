---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a35c753eb296375dc6136050d0151480a457bb637f18a526d2608cef51254f65
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: account-address-status
title: アカウントアドレス準拠
description: ADDR-2 フィクスチャのワークフロー概要と SDK チームの同期方法のまとめ。
---

カノニカルな ADDR-2 バンドル（`fixtures/account/address_vectors.json`）には、I105、i105-default (`sora`; half/full width)、multisignature、negative の fixtures が含まれます。すべての SDK + Torii のサーフェスは同じ JSON に依存しており、本番に到達する前に codec のドリフトを検知できます。このページは内部のステータス・ブリーフ（リポジトリルートの `docs/source/account_address_status.md`）を反映しており、ポータル読者が mono-repo を掘らずにワークフローを参照できます。

## バンドルの再生成または検証

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — JSON を stdout に出力してアドホックに確認します。
- `--out <path>` — 別のパスに書き出します（例: ローカル差分確認時）。
- `--verify` — 作業コピーを新規生成内容と比較します（`--stdout` と併用不可）。

CI ワークフロー **Address Vector Drift** は、fixture、ジェネレータ、docs が変わるたびに `cargo xtask address-vectors --verify` を実行し、レビュー担当者へ即時に通知します。

## fixture を消費するのは？

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

各ハーネスはカノニカルバイト + I105 + 圧縮エンコードを往復し、ネガティブケースの Norito スタイルのエラーコードが fixture と一致することを検証します。

## 自動化が必要ですか？

リリースツールは `scripts/account_fixture_helper.py` を使って fixture の更新を自動化できます。コピー/ペーストなしでカノニカルバンドルの取得または検証を行います:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

このヘルパーは `--source` の上書きや環境変数 `IROHA_ACCOUNT_FIXTURE_URL` を受け付け、SDK の CI ジョブが任意のミラーを指せるようにします。`--metrics-out` が指定されると、`account_address_fixture_check_status{target="…"}` とカノニカル SHA-256 digest（`account_address_fixture_remote_info`）を書き込み、Prometheus textfile collector と Grafana ダッシュボード `account_address_fixture_status` が各サーフェスの同期を証明できます。ターゲットが `0` を返した場合はアラートしてください。マルチサーフェス自動化には `ci/account_fixture_metrics.sh`（`--target label=path[::source]` を繰り返し受け付け）を使い、オンコールチームが node-exporter の textfile collector 用に単一の `.prom` ファイルを公開できるようにします。

## 完全版のブリーフが必要ですか？

ADDR-2 の完全な準拠ステータス（owners、監視計画、未解決のアクション項目）は、リポジトリ内の `docs/source/account_address_status.md` と Address Structure RFC（`docs/account_structure.md`）にあります。このページは簡易な運用リマインダーとして使い、詳細なガイダンスは repo docs を参照してください。
