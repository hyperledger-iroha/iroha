---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T12:29:51+00:00"
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

- `pip install aiohttp` (非同期バリアントを実行する場合)
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

アカウント インベントリ ヘルパーは、次の場合にのみオプションの `asset_id` フィルタを受け入れます。
特定の資産に関心を持つ:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. オフライン手当

オフライン許可エンドポイントを使用してウォレット証明書を発行し、登録します
それらは台帳上にあります。 `top_up_offline_allowance` 問題と登録ステップを連鎖させます
(単一の追加エンドポイントはありません):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "<katakana-i105-account-id>",
    "allowance": {"asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

更新するには、現在の証明書 ID を指定して `top_up_offline_allowance_renewal` を呼び出します。

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

フローを分割する必要がある場合は、`issue_offline_certificate` (または
`issue_offline_certificate_renewal`)、続いて `register_offline_allowance`
または `renew_offline_allowance`。

## 6. ストリームイベント

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

## 7. 次のステップ

- `python/iroha_python/src/iroha_python/examples/` の例を調べてください。
  ガバナンス、ISO ブリッジ ヘルパー、接続をカバーするエンドツーエンド フロー向け。
- 必要な場合は、`create_torii_client` / `resolve_torii_client_config` を使用します。
  `iroha_config` JSON ファイルまたは環境からクライアントをブートストラップします。
- Norito RPC または Connect 固有の API については、次のような特殊なモジュールを確認してください。
  `iroha_python.norito_rpc` および `iroha_python.connect`。

## 関連する Norito の例

- [Hajimari エントリポイント スケルトン](../norito/examples/hajimari-entrypoint) — コンパイル/実行をミラーリングします。
  このクイックスタートのワークフローを利用して、Python から同じスターター コントラクトをデプロイできるようにします。
- [ドメインとミント アセットを登録する](../norito/examples/register-and-mint) — ドメインと一致します +
  上記のアセット フローは、SDK ビルダーではなく台帳側の実装が必要な場合に役立ちます。
- [アカウント間の資産の転送](../norito/examples/transfer-asset) — `transfer_asset` を紹介します
  syscall を使用すると、コントラクト主導の転送を Python ヘルパー メソッドと比較できます。

これらのビルディング ブロックを使用すると、書かずに Python から Torii を実行できます。
独自の HTTP グルーまたは Norito コーデック。 SDK が成熟するにつれて、追加の高レベルの
ビルダーが追加されます。 `python/iroha_python` の README を参照してください。
最新のステータスと移行メモを保存するディレクトリ。