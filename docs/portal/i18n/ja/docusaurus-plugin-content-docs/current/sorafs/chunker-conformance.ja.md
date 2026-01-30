---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eefe6bd546009d9e3506a51ef5ba5337391eb741da85128cff76f93906aac050
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: chunker-conformance
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
`docs/source/sorafs/chunker_conformance.md` を反映しています。レガシー docs が退役するまで両方の版を同期してください。
:::

このガイドは、すべての実装が SoraFS の決定的チャンカー・プロファイル（SF1）と
互換性を保つために従うべき要件を明文化します。また、再生成ワークフロー、
署名ポリシー、検証手順を記載し、SDK をまたいだ fixture 消費者が同期した状態を保てるようにします。

## 正規プロファイル

- 入力シード（hex）: `0000000000dec0ded`
- ターゲットサイズ: 262144 bytes (256 KiB)
- 最小サイズ: 65536 bytes (64 KiB)
- 最大サイズ: 524288 bytes (512 KiB)
- ローリング多項式: `0x3DA3358B4DC173`
- Gear テーブルシード: `sorafs-v1-gear`
- ブレークマスク: `0x0000FFFF`

参照実装: `sorafs_chunker::chunk_bytes_with_digests_profile`。
SIMD アクセラレーションは同一の境界と digest を生成しなければなりません。

## フィクスチャ・バンドル

`cargo run --locked -p sorafs_chunker --bin export_vectors` は fixtures を再生成し、
`fixtures/sorafs_chunker/` に次のファイルを出力します:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust/TypeScript/Go の消費者向けの正規チャンク境界。
  （例: `sorafs.sf1@1.0.0` の後に `sorafs.sf1@1.0.0`）を記載します。順序は
  `ensure_charter_compliance` によって強制され、変更してはなりません。
- `manifest_blake3.json` — すべての fixture ファイルを対象にした BLAKE3 検証済み manifest。
- `manifest_signatures.json` — manifest digest に対する評議会署名（Ed25519）。
- `sf1_profile_v1_backpressure.json` と `fuzz/` 内の生 corpora —
  チャンカーの back-pressure テストで使う決定的なストリーミングシナリオ。

### 署名ポリシー

fixtures の再生成には **必ず** 有効な評議会署名を含める必要があります。ジェネレーターは
`--allow-unsigned` を明示しない限り未署名出力を拒否します（ローカル実験のみ想定）。
署名エンベロープは append-only で、署名者ごとに重複排除されます。

評議会署名を追加するには:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 検証

CI helper の `ci/check_sorafs_fixtures.sh` は `--locked` でジェネレーターを再実行します。
fixtures の差分や署名欠如があればジョブは失敗します。夜間ワークフローや fixture 変更提出前に
このスクリプトを使ってください。

手動検証手順:

1. `cargo test -p sorafs_chunker` を実行します。
2. `ci/check_sorafs_fixtures.sh` をローカルで実行します。
3. `git status -- fixtures/sorafs_chunker` がクリーンであることを確認します。

## アップグレード・プレイブック

新しいチャンカープロファイルを提案する場合、または SF1 を更新する場合:

参照: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) を参照し、
メタデータ要件、提案テンプレート、検証チェックリストを確認してください。

1. 新しいパラメータを含む `ChunkProfileUpgradeProposalV1`（RFC SF-1 を参照）を作成します。
2. `export_vectors` で fixtures を再生成し、新しい manifest digest を記録します。
3. 必要な評議会クォーラムで manifest に署名します。署名はすべて
   `manifest_signatures.json` に追記します。
4. 影響を受ける SDK fixtures（Rust/Go/TS）を更新し、cross-runtime のパリティを確認します。
5. パラメータが変わる場合は fuzz corpora を再生成します。
6. このガイドを新しいプロファイルハンドル、シード、digest で更新します。
7. 更新済みテストと roadmap 更新と一緒に変更を提出します。

このプロセスに従わずにチャンク境界や digest を変更することは無効であり、
マージしてはいけません。
