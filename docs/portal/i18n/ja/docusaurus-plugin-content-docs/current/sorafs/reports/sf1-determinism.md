<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS SF1 Determinism Dry-Run

このレポートは正規 chunker プロファイル `sorafs.sf1@1.0.0` のベースライン
ドライランを記録する。Tooling WG は fixtures の refresh や新しい consumer
パイプラインを検証する際に、以下の checklist を再実行すること。各コマンドの
結果を表に記録し、監査可能なトレイルを維持する。

## Checklist

| ステップ | コマンド | 期待結果 | メモ |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | すべてのテストが通過し、`vectors` parity test が成功する。 | 正規 fixtures がコンパイルされ、Rust 実装と一致することを確認。 |
| 2 | `ci/check_sorafs_fixtures.sh` | スクリプトが 0 で終了し、下記の manifest digests を報告する。 | Fixtures がクリーンに再生成され、署名が付与されたままであることを確認。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` のエントリが registry descriptor (`profile_id=1`) と一致。 | Registry metadata が同期されていることを保証。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | `--allow-unsigned` なしで再生成に成功し、manifest と signature ファイルは不変。 | Chunk 境界と manifests の determinism を示す証明。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript fixtures と Rust JSON の diff がないと報告。 | オプションの helper。runtimes 間の parity を確認 (script は Tooling WG が維持)。 |

## 期待 digests

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Sign-Off Log

| 日付 | Engineer | Checklist 結果 | メモ |
|------|----------|------------------|-------|
| 2026-02-12 | Tooling (LLM) | ✅ Passed | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` で fixtures を再生成し、canonical handle + alias lists と新しい manifest digest `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` を生成。`cargo test -p sorafs_chunker` と `ci/check_sorafs_fixtures.sh` のクリーン実行で検証 (fixtures をチェック用にステージ)。ステップ 5 は Node parity helper の到着待ち。 |
| 2026-02-20 | Storage Tooling CI | ✅ Passed | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) を `ci/check_sorafs_fixtures.sh` で取得。スクリプトが fixtures を再生成し、manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` を確認し、Rust harness を再実行 (Go/Node ステップは利用可能時に実行) して diff なし。 |

Tooling WG は checklist 実行後に日付付き行を追加すること。いずれかの
ステップが失敗した場合は、ここにリンクされた issue を作成し、
remediation 詳細を記載してから fixtures やプロファイルを承認する。
