---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK クイックスタート

Python SDK (`iroha-python`) は Rust クライアント ヘルパーをミラーリングしているため、
スクリプト、ノートブック、または Web バックエンドから Torii と対話します。このクイックスタート
インストール、トランザクションの送信、イベントのストリーミングについて説明します。より深く
適用範囲については、リポジトリの `python/iroha_python/README.md` を参照してください。

## 1. インストール

```bash
pip install iroha-python
```

オプションの追加物:

- `pip install aiohttp` の非同期バリアントを実行する予定の場合
  ストリーミングヘルパー。
- SDK の外部で Ed25519 キーの導出が必要な場合は、`pip install pynacl`。

## 2. クライアントと署名者を作成する

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` は、`timeout_ms` などの追加のキーワード引数を受け入れます。
`max_retries`、および `tls_config`。ヘルパー `resolve_torii_client_config`
Rust CLIと同等の機能が必要な場合は、JSON設定ペイロードを解析します。

## 3. トランザクションを送信する

SDK には命令ビルダーとトランザクション ヘルパーが同梱されているため、ビルドすることはほとんどありません。
Norito ペイロードを手動で作成します。

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` は、署名されたエンベロープと最後のエンベロープの両方を返します。
観察されたステータス (例: `Committed`、`Rejected`)。すでに署名をお持ちの場合
トランザクション エンベロープは `client.submit_transaction_envelope(envelope)` または
JSON 中心の `submit_transaction_json`。

## 4. クエリ状態

すべての REST エンドポイントには JSON ヘルパーがあり、多くは型指定されたデータクラスを公開します。のために
ドメインをリストする例:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

ページネーションを認識するヘルパー (`list_accounts_typed` など) は、次のオブジェクトを返します。
`items` と `next_cursor` の両方が含まれています。

## 5. ストリームイベント

Torii SSE エンドポイントはジェネレーター経由で公開されます。 SDKは自動的に再開します
`resume=True` で、`EventCursor` を指定した場合。

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

その他の便利なメソッドには、`stream_pipeline_transactions`、
`stream_events` (型付きフィルター ビルダーを使用)、および `stream_verifying_key_events`。

## 6. 次のステップ

- `python/iroha_python/src/iroha_python/examples/` の例を調べてください。
  ガバナンス、ISO ブリッジ ヘルパー、接続をカバーするエンドツーエンド フロー向け。
- 必要な場合は、`create_torii_client` / `resolve_torii_client_config` を使用します。
  `iroha_config` JSON ファイルまたは環境からクライアントをブートストラップします。
- Norito RPC または Connect 固有の API については、次のような特殊なモジュールを確認してください。
  `iroha_python.norito_rpc` および `iroha_python.connect`。

これらのビルディング ブロックを使用すると、書かずに Python から Torii を実行できます。
独自の HTTP グルーまたは Norito コーデック。 SDK が成熟するにつれて、追加の高レベルの
ビルダーが追加されます。 `python/iroha_python` の README を参照してください。
最新のステータスと移行メモのディレクトリ。