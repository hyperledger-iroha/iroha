---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bbbb64d20e531ebbada82fd004374d14b7e96a623d5b93f5f3017a4b377ba6b6
source_last_modified: "2026-01-30T15:41:27+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Python SDK クイックスタート

Python SDK（`iroha-python`）は Rust クライアントのヘルパーに合わせて、スクリプト／ノートブック／Web バックエンドから Torii を操作できます。このクイックスタートではインストール、トランザクション送信、イベントストリーミングを扱います。詳細はリポジトリ内の `python/iroha_python/README.md` を参照してください。

## 1. インストール

```bash
pip install iroha-python
```

オプションの追加:

- ストリーミングヘルパーの非同期版を使う場合は `pip install aiohttp`。
- SDK 外で Ed25519 キー導出が必要な場合は `pip install pynacl`。

## 2. クライアントと署名者を作成

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

`ToriiClient` は `timeout_ms`、`max_retries`、`tls_config` などを受け付けます。Rust CLI と同等にしたい場合は `resolve_torii_client_config` で JSON 設定を解析します。

## 3. トランザクションを送信

SDK には命令ビルダーとトランザクションヘルパーがあるため、Norito ペイロードを手で組む必要はほとんどありません:

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

`build_and_submit_transaction` は署名済み envelope と最後に観測したステータス（例: `Committed`, `Rejected`）を返します。署名済み envelope がある場合は `client.submit_transaction_envelope(envelope)` または JSON 中心の `submit_transaction_json` を使用してください。

## 4. 状態をクエリ

すべての REST エンドポイントには JSON ヘルパーがあり、多くは型付きデータクラスを提供します。例としてドメイン一覧:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

ページネーション対応のヘルパー（例: `list_accounts_typed`）は `items` と `next_cursor` を含むオブジェクトを返します。

アカウント在庫ヘルパーは、特定の資産だけを対象にする場合 `asset_id` フィルタを受け取ります:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline V2 readiness

Use `GET /v1/offline/v2/readiness` through `get_offline_v2_readiness()` for offline feature discovery.
Offline V2 note issuance, redemption, and audit payloads are submitted as transaction instructions;
legacy offline allowance, reserve, revocation, transfer-history, and cash HTTP routes are no longer published by Torii.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
readiness = client.get_offline_v2_readiness()
print("offline notes", readiness.offline_note_v2)
```
## 6. イベントストリーム

Torii の SSE エンドポイントはジェネレータとして公開されています。`resume=True` と `EventCursor` を指定すると SDK が自動的に再開します。

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

他の便利メソッドには `stream_pipeline_transactions`、`stream_events`（型付きフィルタビルダー付き）、`stream_verifying_key_events` があります。

## 7. 次のステップ

- `python/iroha_python/src/iroha_python/examples/` の例で、ガバナンス、ISO bridge、Connect の end‑to‑end フローを確認してください。
- `iroha_config` JSON や環境からクライアントを初期化したい場合は `create_torii_client` / `resolve_torii_client_config` を使用します。
- Norito RPC や Connect の API については `iroha_python.norito_rpc` や `iroha_python.connect` といった専用モジュールを参照してください。

## 関連 Norito 例

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — このクイックスタートの compile/run フローを反映し、Python から同じスターターコントラクトをデプロイできます。
- [Register domain and mint assets](../norito/examples/register-and-mint) — 上記のドメイン＋アセットフローに対応し、SDK ビルダーではなく台帳側実装が必要な場合に有用です。
- [Transfer asset between accounts](../norito/examples/transfer-asset) — コントラクト駆動の転送と Python ヘルパーを比較できるように `transfer_asset` syscall を示します。

これらの構成要素で Torii を Python から操作できます。独自の HTTP グルーや Norito コーデックは不要です。SDK の成熟に伴い高水準ビルダーが追加されます。最新状況と移行ノートは `python/iroha_python` の README を参照してください。
