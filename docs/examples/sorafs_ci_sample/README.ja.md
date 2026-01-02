---
lang: ja
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CI サンプル Fixtures

このディレクトリは `fixtures/sorafs_manifest/ci_sample/` のサンプル payload から生成された決定的な artefacts をパッケージしています。このバンドルは、CI ワークフローが実行する SoraFS のエンドツーエンドのパッケージングおよび署名パイプラインを示します。

## アーティファクト一覧

| ファイル | 説明 |
|------|-------------|
| `payload.txt` | fixture スクリプトで使用するソース payload (プレーンテキストのサンプル). |
| `payload.car` | `sorafs_cli car pack` が生成する CAR アーカイブ. |
| `car_summary.json` | `car pack` が生成するサマリで、chunk digest とメタデータを記録. |
| `chunk_plan.json` | chunk 範囲と provider 期待値を記述する fetch-plan JSON. |
| `manifest.to` | `sorafs_cli manifest build` により生成された Norito manifest. |
| `manifest.json` | デバッグ用の人間可読 manifest. |
| `proof.json` | `sorafs_cli proof verify` が出力する PoR サマリ. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` が生成する keyless 署名 bundle. |
| `manifest.sig` | manifest に対応する分離 Ed25519 署名. |
| `manifest.sign.summary.json` | 署名時に生成される CLI サマリ (hashes と bundle メタデータ). |
| `manifest.verify.summary.json` | `manifest verify-signature` の CLI サマリ. |

リリースノートやドキュメントで参照される digests はこれらのファイル由来です。`ci/check_sorafs_cli_release.sh` は同一 artefacts を再生成し、コミット済みの内容と diff します。

## Fixtures 再生成

リポジトリルートで以下のコマンドを実行して fixture セットを再生成してください。`sorafs-cli-fixture` ワークフローの手順を反映しています:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

いずれかの手順で異なる hash が生成された場合、fixtures を更新する前に調査してください。CI ワークフローは決定的な出力に依存して回帰を検出します。

## 今後のカバレッジ

追加の chunker プロファイルや proof 形式が roadmap から昇格したら、それらの正規 fixtures をこのディレクトリに追加します (例:
`sorafs.sf2@1.0.0` (参照: `fixtures/sorafs_manifest/ci_sample_sf2/`) または PDP streaming proofs)。新しいプロファイルは
payload、CAR、plan、manifest、proofs、署名 artefacts の同じ構造に従い、downstream 自動化が特別なスクリプト無しで
リリースを diff できるようにします。
