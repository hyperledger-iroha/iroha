---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/python.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T12:29:51.735640+00:00"
translation_last_reviewed: 2026-01-30
---

# Быстрый старт SDK Python

Python SDK (`iroha-python`) отражает помощники клиента Rust, чтобы вы могли работать с Torii из скриптов, ноутбуков или веб‑бэкендов. Этот quickstart охватывает установку, отправку транзакций и стриминг событий. Подробнее см. `python/iroha_python/README.md` в репозитории.

## 1. Установка

```bash
pip install iroha-python
```

Дополнительные опции:

- `pip install aiohttp`, если планируете использовать асинхронные варианты helpers для стриминга.
- `pip install pynacl`, когда нужна деривация ключей Ed25519 вне SDK.

## 2. Создайте клиента и signers

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

`ToriiClient` принимает дополнительные аргументы вроде `timeout_ms`, `max_retries` и `tls_config`. Хелпер `resolve_torii_client_config` парсит JSON‑конфиг, если нужна паритетность с Rust CLI.

## 3. Отправьте транзакцию

SDK включает builders инструкций и helpers транзакций, поэтому редко нужно собирать Norito payload вручную:

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

`build_and_submit_transaction` возвращает подписанный envelope и последний наблюдаемый статус (например, `Committed`, `Rejected`). Если у вас уже есть подписанный envelope, используйте `client.submit_transaction_envelope(envelope)` или JSON‑ориентированный `submit_transaction_json`.

## 4. Запрос состояния

Все REST‑эндпоинты имеют JSON‑helpers, многие возвращают типизированные dataclasses. Например, список доменов:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Helpers с пагинацией (например, `list_accounts_typed`) возвращают объект с `items` и `next_cursor`.

Helpers инвентаризации аккаунтов принимают необязательный фильтр `asset_id`, если вам нужен конкретный актив:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline allowances

Используйте offline allowance endpoints, чтобы выпускать кошельковые сертификаты и регистрировать их on‑ledger. `top_up_offline_allowance` объединяет выпуск + регистрацию (отдельного top‑up эндпоинта нет):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "<i105-account-id>",
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

Для продления вызовите `top_up_offline_allowance_renewal` с текущим id сертификата:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Если нужно разделить поток, вызовите `issue_offline_certificate` (или `issue_offline_certificate_renewal`), затем `register_offline_allowance` или `renew_offline_allowance`.

## 6. Потоки событий

SSE эндпоинты Torii доступны через генераторы. SDK автоматически возобновляет поток при `resume=True` и переданном `EventCursor`.

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

Другие удобные методы: `stream_pipeline_transactions`, `stream_events` (с типизированными фильтрами) и `stream_verifying_key_events`.

## 7. Дальнейшие шаги

- Изучите примеры в `python/iroha_python/src/iroha_python/examples/` для end‑to‑end потоков по governance, ISO bridge и Connect.
- Используйте `create_torii_client` / `resolve_torii_client_config`, если хотите инициализировать клиента из JSON `iroha_config` или окружения.
- Для Norito RPC или Connect API смотрите специализированные модули `iroha_python.norito_rpc` и `iroha_python.connect`.

## Связанные примеры Norito

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — повторяет workflow compile/run из этого quickstart, чтобы развернуть такой же стартовый контракт из Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — соответствует потокам домена + активов выше и полезно, если нужна ledger‑реализация вместо SDK builders.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — показывает syscall `transfer_asset`, чтобы сравнить контрактные переводы с Python helpers.

Эти блоки позволяют работать с Torii из Python без собственного HTTP‑клея или Norito кодеков. По мере развития SDK появятся более высокоуровневые builders; смотрите README в `python/iroha_python` для актуального статуса и заметок миграции.
