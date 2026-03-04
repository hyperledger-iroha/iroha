---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK Quickstart

Python SDK (`iroha-python`) көҙгө клиент ярҙамсылары Rust, шулай итеп, һеҙ аласыз
үҙ-ара эш итеү менән Torii сценарийҙар, ноутбуктар, йәки веб-бэкэнд. Был тиҙ старт
ҡаплау, транзакция тапшырыу, һәм ваҡиғалар потоковый. Тәрәнерәк өсөн
ҡаплау ҡарағыҙ `python/iroha_python/README.md` һаҡлағысында.

## 1. Ҡуй.

```bash
pip install iroha-python
```

Опциональ өҫтәмәләр:

- `pip install aiohttp`, әгәр һеҙ асинхрон варианттарын эшләтергә планлаштыра,
  потоковый ярҙамсылары.
- `pip install pynacl` ҡасан һеҙгә кәрәк Ed25519 төп сығарылыш тыш SDK.

## 2. Клиент һәм ҡул ҡуйыусылар булдырыу

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

`ToriiClient` өҫтәмә төп һүҙ аргументтарын ҡабул итә, мәҫәлән, `timeout_ms`, .
`max_retries`, һәм `tls_config`. ярҙамсыһы `resolve_torii_client_config`
анализ JSON конфигурацияһы файҙалы йөк, әгәр һеҙ теләйһегеҙ, паритет менән Rust CLI.

## 3. Транзакция тапшырыу

SDK суднолар инструкция төҙөүселәр һәм транзакция ярҙамсылары, шулай итеп, һеҙ һирәк төҙөү
Ҡул менән файҙалы йөктәр Norito.

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
``` X

`build_and_submit_transaction` ҡайтарып, ҡул ҡуйылған конверт һәм һуңғы
күҙәтелгән статус (мәҫәлән, `Committed`, `Rejected`). Әгәр һеҙ инде ҡултамға
транзакция конверты ҡулланыу `client.submit_transaction_envelope(envelope)` йәки
JSON-центрик `submit_transaction_json`.

## 4. Һорау дәүләт

Бөтә REST ос нөктәләрендә JSON ярҙамсылары һәм күптәр типтағы мәғлүмәт кластары фашланған. Өсөн
миҫал, домендарҙы исемлеккә индерә:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Пагинация-аңлы ярҙамсылары (мәҫәлән, `list_accounts_typed`) ҡайтарыу объекты, тип
составында `items` һәм `next_cursor`.

## 5. Ағым ваҡиғалары

Torii SSE ос нөктәләре генераторҙар аша асыҡлана. SDK автоматик рәүештә тергеҙелә
Ҡасан `resume=True` һәм һеҙ Norito тәьмин итә.

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

Башҡа уңайлыҡтар ысулдары `stream_pipeline_transactions`,
`stream_events` (типлаштырылған фильтр төҙөүселәр менән), һәм `stream_verifying_key_events`.

## 6. Киләһе аҙымдар

- `python/iroha_python/src/iroha_python/examples/` буйынса миҫалдарҙы тикшерергә
  идара итеүҙе ҡаплаған ос-ос ағымдар өсөн, ISO күпер ярҙамсылары, һәм Connect.
- Ҡулланыу `create_torii_client` / Torii, ҡасан һеҙ теләйһегеҙ
  клиентты `iroha_config` JSON файлынан йәки тирә-яҡ мөхиттән bootstrap.
- Norito өсөн RPC йәки тоташтырыу-специфик API-лар, махсуслаштырылған модулдәрҙе тикшерергә, мәҫәлән,
  `iroha_python.norito_rpc` һәм `iroha_python.connect`.

Был төҙөлөш блоктары менән һеҙ Torii Python-дан яҙмайынса эшләй алаһығыҙ
үҙ HTTP йәбештереү йәки Norito кодектары. СДК өлгөргән һайын, өҫтәмә юғары кимәлдә
төҙөүселәр өҫтәләсәк; консультация README `python/iroha_python`
каталогы өсөн һуңғы статус һәм миграция иҫкәрмәләр.