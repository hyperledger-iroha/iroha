---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Краткое руководство по Python SDK

Python SDK (`iroha-python`) отражает помощники клиента Rust, поэтому вы можете
взаимодействовать с Torii из сценариев, блокнотов или веб-серверов. Это краткое руководство
охватывает установку, отправку транзакций и потоковую передачу событий. Для более глубокого
покрытие см. в репозитории `python/iroha_python/README.md`.

## 1. Установить

```bash
pip install iroha-python
```

Дополнительные опции:

- `pip install aiohttp`, если вы планируете запускать асинхронные варианты
  потоковые помощники.
- `pip install pynacl`, когда вам нужно получить ключ Ed25519 вне SDK.

## 2. Создайте клиента и подписывающих лиц

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

`ToriiClient` принимает дополнительные аргументы ключевых слов, такие как `timeout_ms`,
`max_retries` и `tls_config`. Помощник `resolve_torii_client_config`
анализирует полезную нагрузку конфигурации JSON, если вы хотите обеспечить четность с интерфейсом командной строки Rust.

## 3. Отправьте транзакцию

В состав SDK входят конструкторы инструкций и помощники транзакций, поэтому вам придется редко создавать
Norito полезные нагрузки вручную:

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

`build_and_submit_transaction` возвращает подписанный конверт и последний
наблюдаемый статус (например, `Committed`, `Rejected`). Если у вас уже есть подписанный
Конверт транзакции используйте `client.submit_transaction_envelope(envelope)` или
JSON-ориентированный `submit_transaction_json`.

## 4. Состояние запроса

Все конечные точки REST имеют помощники JSON, и многие из них предоставляют типизированные классы данных. Для
пример, список доменов:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Помощники с поддержкой разбиения на страницы (например, `list_accounts_typed`) возвращают объект, который
содержит как `items`, так и `next_cursor`.

## 5. Трансляция событий

Torii Конечные точки SSE предоставляются через генераторы. SDK автоматически возобновляет работу
когда `resume=True` и вы предоставляете `EventCursor`.

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

Другие удобные методы включают `stream_pipeline_transactions`,
`stream_events` (со сборщиками типизированных фильтров) и `stream_verifying_key_events`.

## 6. Следующие шаги

- Изучите примеры под `python/iroha_python/src/iroha_python/examples/`.
  для сквозных потоков, охватывающих управление, помощники моста ISO и Connect.
- Используйте `create_torii_client` / `resolve_torii_client_config`, когда хотите.
  загрузите клиент из JSON-файла или среды `iroha_config`.
- Для Norito RPC или API-интерфейсов Connect проверьте специализированные модули, такие как
  `iroha_python.norito_rpc` и `iroha_python.connect`.

С помощью этих строительных блоков вы можете использовать Torii из Python без написания
свой собственный HTTP-клей или кодеки Norito. По мере развития SDK появляются дополнительные высокоуровневые
будут добавлены строители; обратитесь к README в `python/iroha_python`
каталог для последнего статуса и заметок о миграции.