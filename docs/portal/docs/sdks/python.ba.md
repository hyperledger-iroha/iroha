---
lang: ba
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T18:06:01.646084+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK Quickstart

Python SDK (I18NI000000019X) көҙгө клиент ярҙамсылары Rust, шулай итеп, һеҙ аласыз
үҙ-ара эш итеү менән I18NT0000000004X сценарийҙар, ноутбуктар, йәки веб-бэкендтар. Был тиҙ старт
ҡаплау, транзакция тапшырыу, һәм ваҡиғалар потоковый. Тәрәнерәк өсөн
ҡаплау ҡарағыҙ I18NI000000020X һаҡлағысында.

## 1. Ҡуй.

```bash
pip install iroha-python
```

Опциональ өҫтәмәләр:

- I18NI000000021X, әгәр һеҙ асинхрон варианттарын эшләтергә планлаштыра,
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
``` X

I18NI000000023X өҫтәмә төп һүҙ аргументтарын ҡабул итә, мәҫәлән, `timeout_ms`, .
`max_retries`, һәм `tls_config`. Ярҙамсы I18NI000000027X
анализ JSON конфигурацияһы файҙалы йөк, әгәр һеҙ теләйһегеҙ, паритет менән Rust CLI.

## 3. Транзакция тапшырыу

SDK суднолар инструкция төҙөүселәр һәм транзакция ярҙамсылары, шулай итеп, һеҙ һирәк төҙөү
Ҡул менән файҙалы йөктәр I18NT00000000000000000000000000000.

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

I18NI000000028X ҡасып, ҡул ҡуйылған конверт һәм һуңғы
күҙәтелгән статус (мәҫәлән, `Committed`, I18NI0000000300X). Әгәр һеҙ инде ҡултамға
транзакция конверты ҡулланыу I18NI0000000031X йәки
JSON-центрик `submit_transaction_json`.

## 4. Һорау дәүләт

Бөтә REST ос нөктәләрендә JSON ярҙамсылары һәм күптәр типтағы мәғлүмәт кластары фашланған. Өсөн
миҫал, домендарҙы исемлеккә индерә:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Пагинация-аңлы ярҙамсылары (мәҫәлән, I18NI000000033X) ҡайтарыу объекты, тип
`items` һәм I18NI000000035X X.

Иҫәп инвентаризация ярҙамсылар ҡабул итеү опциональ I18NI00000000036X фильтр ҡасан һеҙ генә .
аныҡ актив тураһында ҡайғыртыу:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Офлайн пособиелар

Ҡулланыу офлайн пособие ос нөктәләрен сығарыу өсөн янсыҡ сертификаттары һәм теркәлергә .
уларҙы легаль. I18NI0000000037X мәсьәләне сылбырлай + теркәү аҙымдары
(бер генә лә өҫкә ос нөктәһе юҡ):

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

Яңыртыу өсөн, шылтыратыу I18NI0000000038X менән ағымдағы сертификат id:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Әгәр һеҙгә кәрәк бүлергә ағым, шылтыратыу I18NI000000039X (йәки
I18NI000000040X) `register_offline_allowance`
йәки `renew_offline_allowance`.

## 6. Ағым ваҡиғалары

Torii SSE ос нөктәләре генераторҙар аша асыҡлана. SDK автоматик рәүештә тергеҙелә
Ҡасан I18NI000000043X һәм һеҙ I18NI00000044ХХХХ.

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

Башҡа уңайлыҡтар ысулдары I18NI000000045X,
I18NI000000046X (типлаштырылған фильтр төҙөүселәр менән), һәм I18NI000000047X.

## 7. Киләһе аҙымдар

- I18NI000000048X буйынса миҫалдарҙы тикшерергә
  идара итеүҙе ҡаплаған ос-ос ағымдар өсөн, ISO күпер ярҙамсылары, һәм Connect.
- I18NI000000049X X / I18NI0000000050X ҡулланыу, ҡасан һеҙ теләйһегеҙ
  клиентты `iroha_config` JSON файлынан йәки тирә-яҡ мөхиттән bookstrap.
- I18NT0000000001X өсөн RPC йәки тоташтырыу-специфик API-лар, махсуслаштырылған модулдәрҙе тикшерергә, мәҫәлән,
  `iroha_python.norito_rpc` һәм I18NI000000053X.

## I18NT000000002X миҫалдары менән бәйле

- [Хжимари инеү нөктәһе скелеты] (I18NU000000016X) — компиляция/йүгереүҙе көҙгөләй
  эш ағымы был тиҙ старт, шулай итеп, һеҙ шул уҡ стартер килешеп таратыу мөмкин Python.
- [Регистр домен һәм мәтрүшкә активтары](I18NU000000017X) — доменға тап килә +
  активтар ағымы өҫтә һәм файҙалы, ҡасан һеҙ теләйһегеҙ, леджер-яҡын тормошҡа ашырыу урынына SDK төҙөүселәр.
- [Иҫәптәр араһында күсерергә] (I18NU000000018X) — I18NI000000054X-ны күрһәтә
  syscall, шулай итеп, һеҙ сағыштырырға мөмкин контракт-двигателдәр менән күсермәләр менән Python ярҙамсы ысулдары.

Был төҙөлөш блоктары менән һеҙ I18NT000000007X Python яҙмаһыҙ эшләй ала
үҙ HTTP йәбештереү йәки I18NT0000000003X кодектары. СДК өлгөргән һайын, өҫтәмә юғары кимәлдә
төҙөүселәр өҫтәләсәк; консультация README I18NI000000055X
каталогы өсөн һуңғы статус һәм миграция иҫкәрмәләр.