---
lang: ja
direction: ltr
source: docs/source/torii/api_versioning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e27b4aa44b025737d3e8dfed751fc5a0bc1dd12807f77a887f46ce830025c8
source_last_modified: "2026-01-04T10:50:53.697166+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/torii/api_versioning.md -->

# Torii API バージョニング

Torii は各リクエストで `x-iroha-api-version` ヘッダーを用いてセマンティックな API
バージョン (`major.minor`) をネゴシエートする。ヘッダーが無い場合、Torii は
最新のサポートバージョンを仮定し、完全なサポートマトリクスとともに返却する。

- 現在のサポート: `1.0`, `1.1` (デフォルト)。
- proof/staking/fee のサーフェスは少なくとも `1.1` が必要。古いバージョンには
  `426 UPGRADE_REQUIRED` と `code="torii_api_version_too_old"` が返る。
  `1893456000`, 2030-01-01 UTC) の場合は警告を記録しつつ応答を継続する。
- 応答ヘッダーには常に `x-iroha-api-version`,
  `x-iroha-api-supported`, `x-iroha-api-min-proof-version` を含め、クライアントが
  値をハードコードせずに適応できるようにする。
- エラーコードは安定:
  - `torii_api_version_invalid` — ヘッダーがセマンティックバージョンではない。
  - `torii_api_version_unsupported` — バージョンがサーバーのサポート集合にない。
  - `torii_api_version_too_old` — バージョンがサーフェスごとの最小値より古い。

`/v1/api/versions` エンドポイントは診断や SDK 既定値向けにサポートマトリクス
(`default`, `supported`, `min_proof_version`, 任意の `sunset_unix`) を返す。
クライアントはヘッダーを明示的に送るべきであり、CLI は現在 `1.1` をデフォルトに
して `client.toml` の `torii_api_version` 経由で上書きを通す。

`/api_version` エンドポイントはアクティブなブロックヘッダーバージョン文字列を
プレーンテキストで返す。genesis がまだコミットされていない場合、
`503 SERVICE_UNAVAILABLE` と本文 `genesis not applied` を返す。
