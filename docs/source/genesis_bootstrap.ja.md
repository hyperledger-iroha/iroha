---
lang: ja
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 信頼できるピアからの Genesis ブートストラップ

ローカル `genesis.file` を持たない Iroha ピアは、信頼できるピアから署名付きジェネシス ブロックをフェッチできます
Norito でエンコードされたブートストラップ プロトコルを使用します。

- **プロトコル:** ピアは `GenesisRequest` (メタデータの場合は `Preflight`、ペイロードの場合は `Fetch`) を交換し、
  `request_id` をキーとする `GenesisResponse` フレーム。レスポンダーには、チェーン ID、署名者の公開鍵、
  ハッシュ、およびオプションのサイズヒント。ペイロードは `Fetch` でのみ返され、リクエスト ID が重複します
  `DuplicateRequest`を受信します。
- **Guards:** レスポンダーはホワイトリスト (`genesis.bootstrap_allowlist` または信頼できるピア) を強制します。
  set)、チェーン ID/公開キー/ハッシュ マッチング、レート制限 (`genesis.bootstrap_response_throttle`)、および
  サイズキャップ(`genesis.bootstrap_max_bytes`)。ホワイトリスト外のリクエストは `NotAllowed` を受け取ります。
  間違った鍵で署名されたペイロードは `MismatchedPubkey` を受け取ります。
- **リクエスター フロー:** ストレージが空で、`genesis.file` が設定されていない場合 (および
  `genesis.bootstrap_enabled=true`)、ノードはオプションの
  `genesis.expected_hash` は、ペイロードをフェッチし、`validate_genesis_block` 経由で署名を検証します。
  ブロックを適用する前に、Kura と一緒に `genesis.bootstrap.nrt` を永続化します。ブートストラップの再試行
  `genesis.bootstrap_request_timeout`、`genesis.bootstrap_retry_interval`、および
  `genesis.bootstrap_max_attempts`。
- **障害モード:** ホワイトリストのミス、チェーン/公開キー/ハッシュの不一致、サイズによりリクエストが拒否されます。
  上限違反、レート制限、ローカル ジェネシスの欠落、またはリクエスト ID の重複。競合するハッシュ
  ピア間ではフェッチが中止されます。レスポンダーなし/タイムアウトの場合は、ローカル構成にフォールバックします。
- **オペレータの手順:** 少なくとも 1 つの信頼できるピアが有効なジェネシスで到達可能であることを確認し、設定します
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` および再試行ノブ、および
  必要に応じて、`expected_hash` を固定して、不一致のペイロードの受け入れを回避します。永続化されたペイロードは、
  `genesis.file` を `genesis.bootstrap.nrt` に指定することで、後続のブートで再利用されます。