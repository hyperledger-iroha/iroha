---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4732abb0a119464473695df46524081d824ab1b22d9a2f816f3cce4888328076
source_last_modified: "2025-11-14T04:43:21.479399+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正規ソース
`docs/source/sorafs/chunker_profile_authoring.md` を反映しています。レガシーの Sphinx ドキュメントセットが退役するまで、両方のコピーを同期してください。
:::

# SoraFS チャンカー・プロファイル オーサリングガイド

このガイドは、SoraFS 向けの新しいチャンカー・プロファイルを提案して公開する方法を説明します。
アーキテクチャ RFC (SF-1) とレジストリ参照 (SF-2a) を補完し、
具体的なオーサリング要件、検証手順、提案テンプレートを提供します。
正規の例として
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
および付随する dry-run ログ
`docs/source/sorafs/reports/sf1_determinism.md` を参照してください。

## 概要

レジストリに入るすべてのプロファイルは次を満たす必要があります:

- アーキテクチャ間で同一の決定的 CDC パラメータと multihash 設定を広告する。
- 再現可能な fixtures (Rust/Go/TS の JSON + fuzz corpora + PoR witness) を提供し、
  下流 SDK が特別な tooling なしで検証できるようにする。
- ガバナンス対応のメタデータ (namespace, name, semver) と移行ガイダンスおよび
  互換ウィンドウを含める。
- 評議会レビュー前に決定的 diff スイートに合格する。

以下のチェックリストに従って、これらのルールを満たす提案を準備してください。

## レジストリ・チャーターの要約

提案を作成する前に、`sorafs_manifest::chunker_registry::ensure_charter_compliance()` が
適用するレジストリ・チャーターに準拠していることを確認してください:

- プロファイル ID は正の整数で、欠番なしに単調増加します。
- 正規ハンドル (`namespace.name@semver`) は alias リストに含まれ、**必ず** 最初の
- alias は他の正規ハンドルと衝突せず、重複してはいけません。
- alias は空であってはならず、前後の空白を取り除く必要があります。

便利な CLI ヘルパー:

```bash
# 登録済み descriptor の JSON 一覧 (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# 既定候補プロファイルのメタデータを出力 (正規ハンドル + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

これらのコマンドは提案をレジストリ・チャーターに整合させ、ガバナンス議論に必要な
正規メタデータを提供します。

## 必須メタデータ

| フィールド | 説明 | 例 (`sorafs.sf1@1.0.0`) |
|-----------|------|------------------------|
| `namespace` | 関連するプロファイルの論理的なグループ。 | `sorafs` |
| `name` | 人が読めるラベル。 | `sf1` |
| `semver` | パラメータセットのセマンティックバージョン文字列。 | `1.0.0` |
| `profile_id` | プロファイルが登録されたときに一度だけ割り当てる単調増加の数値 ID。次の id を予約するが既存の番号は再利用しない。 | `1` |
| `profile.min_size` | チャンク長の最小値 (bytes)。 | `65536` |
| `profile.target_size` | チャンク長のターゲット値 (bytes)。 | `262144` |
| `profile.max_size` | チャンク長の最大値 (bytes)。 | `524288` |
| `profile.break_mask` | ローリングハッシュで使う適応マスク (hex)。 | `0x0000ffff` |
| `profile.polynomial` | gear 多項式の定数 (hex)。 | `0x3da3358b4dc173` |
| `gear_seed` | 64 KiB gear テーブルを導出する seed。 | `sorafs-v1-gear` |
| `chunk_multihash.code` | チャンク digest 用の multihash コード。 | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | 正規 fixtures バンドルの digest。 | `13fa...c482` |
| `fixtures_root` | 再生成された fixtures を含む相対ディレクトリ。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | 決定的 PoR サンプリングの seed (`splitmix64`)。 | `0xfeedbeefcafebabe` (例) |

メタデータは提案書と生成された fixtures の両方に記載し、レジストリ、CLI tooling、
ガバナンス自動化が値を手作業の突合せなしで検証できるようにしてください。
迷った場合は `--json-out=-` で chunk-store と manifest CLI を実行し、算出メタデータを
レビュー用のノートへストリームしてください。

### CLI とレジストリの接点

- `sorafs_manifest_chunk_store --profile=<handle>` — 提案パラメータでチャンクメタデータ、
  manifest digest、PoR チェックを再実行する。
- `sorafs_manifest_chunk_store --json-out=-` — chunk-store レポートを stdout にストリームし、
  自動比較に使う。
- `sorafs_manifest_stub --chunker-profile=<handle>` — manifests と CAR プランが正規ハンドルと
  aliases を埋め込んでいることを確認する。
- `sorafs_manifest_stub --plan=-` — 以前の `chunk_fetch_specs` を再入力し、変更後の
  offsets/digests を検証する。

コマンド出力（digests、PoR ルート、manifest ハッシュ）を提案に記録し、レビュー担当が
文字どおり再現できるようにします。

## 決定性と検証チェックリスト

1. **fixtures を再生成**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **パリティスイートを実行** — `cargo test -p sorafs_chunker` と cross-language diff harness
   (`crates/sorafs_chunker/tests/vectors.rs`) が、新しい fixtures を反映した状態で green であること。
3. **fuzz/back-pressure corpora を再実行** — `cargo fuzz list` と streaming harness
   (`fuzz/sorafs_chunker`) を再生成された assets に対して実行する。
4. **Proof-of-Retrievability witness を検証** — 提案プロファイルで
   `sorafs_manifest_chunk_store --por-sample=<n>` を実行し、ルートが fixture manifest と一致することを確認する。
5. **CI dry run** — `ci/check_sorafs_fixtures.sh` をローカルで実行し、新しい fixtures と既存の
   `manifest_signatures.json` で成功することを確認する。
6. **cross-runtime 確認** — Go/TS バインディングが再生成された JSON を消費し、同一の
   チャンク境界と digest を出力することを確認する。

Tooling WG が推測なしで再実行できるよう、コマンドと結果の digest を提案書に記録してください。

### Manifest / PoR 確認

fixtures を再生成した後、manifest パイプライン全体を実行して、CAR メタデータと PoR 証明が
整合していることを確認します:

```bash
# 新しいプロファイルでチャンクメタデータ + PoR を検証
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR を生成し、chunk fetch specs を収集
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# 保存した fetch plan で再実行（古い offsets を防ぐ）
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

入力ファイルは fixtures に使われる代表的なコーパス（例: 1 GiB の決定的ストリーム）に置き換え、
得られた digest を提案に添付してください。

## 提案テンプレート

提案は `ChunkerProfileProposalV1` の Norito レコードとして
`docs/source/sorafs/proposals/` に格納します。以下の JSON テンプレートは期待される形を示します
（必要に応じて値を置き換えてください）:


対応する Markdown レポート (`determinism_report`) を用意し、コマンド出力、チャンク digest、
検証中に発生した差分を記録してください。

## ガバナンスのワークフロー

1. **提案 + fixtures を含む PR を提出。** 生成した assets、Norito の提案、
   `chunker_registry_data.rs` の更新を含めます。
2. **Tooling WG レビュー。** レビュー担当が検証チェックリストを再実行し、提案がレジストリ規則
   （id 再利用なし、決定性の達成）に沿っていることを確認します。
3. **評議会エンベロープ。** 承認後、評議会メンバーが提案 digest
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) に署名し、fixtures と一緒に保管される
   プロファイル・エンベロープへ署名を追加します。
4. **レジストリ公開。** マージによりレジストリ、docs、fixtures が更新されます。既定の CLI は
   ガバナンスが移行準備完了を宣言するまで以前のプロファイルに留まります。
   とし、migration ledger を通じてオペレーターへ通知します。

## オーサリングのヒント

- 端の挙動を最小化するため、2 のべき乗で偶数の境界を優先します。
- multihash コードを変更する場合は manifest と gateway の消費者と調整し、互換性ノートを追加します。
- gear テーブル seed は人が読める形にしつつ、グローバルに一意にして監査を容易にします。
- ベンチマーク成果物（例: throughput 比較）は `docs/source/sorafs/reports/` に保管します。

ロールアウト中の運用上の期待事項は migration ledger
(`docs/source/sorafs/migration_ledger.md`) を参照してください。runtime の準拠ルールは
`docs/source/sorafs/chunker_conformance.md` を参照してください。
