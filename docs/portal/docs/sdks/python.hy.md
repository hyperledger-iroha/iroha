---
lang: hy
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

Python SDK-ն (`iroha-python`) արտացոլում է Rust հաճախորդի օգնականներին, որպեսզի կարողանաք
փոխազդել Torii-ի հետ սկրիպտներից, նոթատետրերից կամ վեբ հետնամասերից: Այս արագ մեկնարկը
ընդգրկում է տեղադրումը, գործարքների ներկայացումը և իրադարձությունների հոսքը: Ավելի խորության համար
ծածկույթը տես `python/iroha_python/README.md` պահոցում:

## 1. Տեղադրեք

```bash
pip install iroha-python
```

Ընտրովի հավելումներ.

- `pip install aiohttp`, եթե նախատեսում եք գործարկել ասինխրոն տարբերակները
  հոսքային օգնականներ.
- `pip install pynacl`, երբ ձեզ անհրաժեշտ է Ed25519 ստեղնաշարի ստացում SDK-ից դուրս:

## 2. Ստեղծեք հաճախորդ և ստորագրողներ

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

`ToriiClient` ընդունում է լրացուցիչ հիմնաբառերի փաստարկներ, ինչպիսիք են `timeout_ms`,
`max_retries` և `tls_config`: Օգնական `resolve_torii_client_config`
վերլուծում է JSON կազմաձևման օգտակար բեռը, եթե ցանկանում եք հավասարություն Rust CLI-ի հետ:

## 3. Ներկայացրե՛ք գործարք

SDK-ն ուղարկում է հրահանգներ կառուցողներ և գործարքների օգնականներ, այնպես որ դուք հազվադեպ եք կառուցում
Norito բեռնատարներ ձեռքով.

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

`build_and_submit_transaction`-ը վերադարձնում է և՛ ստորագրված ծրարը, և՛ վերջինը
դիտարկված կարգավիճակը (օրինակ՝ `Committed`, `Rejected`): Եթե դուք արդեն ստորագրված եք
գործարքի ծրարի օգտագործումը `client.submit_transaction_envelope(envelope)` կամ
JSON-կենտրոն `submit_transaction_json`:

## 4. Հարցման վիճակ

Բոլոր REST վերջնակետերն ունեն JSON օգնականներ և շատերը բացահայտում են մուտքագրված տվյալների դասակարգերը: Համար
օրինակ՝ ցուցակագրելով տիրույթները.

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Էջագրման տեղյակ օգնականները (օրինակ՝ `list_accounts_typed`) վերադարձնում են օբյեկտ, որը
պարունակում է և՛ `items`, և՛ `next_cursor`:

Հաշվի գույքագրման օգնականները ընդունում են կամընտիր `asset_id` զտիչ, երբ դուք միայն
հոգ տանել կոնկրետ ակտիվի մասին.

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Անցանց նպաստներ

Օգտագործեք անցանց նպաստի վերջնակետերը՝ դրամապանակի վկայագրեր տրամադրելու և գրանցելու համար
դրանք գրանցամատյանում: `top_up_offline_allowance`-ը կապում է խնդիրը + գրանցման քայլերը
(չկա մեկ լիցքավորման վերջնակետ):

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

Թարմացումների համար զանգահարեք `top_up_offline_allowance_renewal`՝ ընթացիկ վկայագրի ID-ով.

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Եթե Ձեզ անհրաժեշտ է բաժանել հոսքը, զանգահարեք `issue_offline_certificate` (կամ
`issue_offline_certificate_renewal`), որին հաջորդում է `register_offline_allowance`
կամ `renew_offline_allowance`.

## 6. Հեռարձակեք իրադարձությունները

Torii SSE վերջնակետերը բացահայտվում են գեներատորների միջոցով: SDK-ն ավտոմատ կերպով վերսկսվում է
երբ `resume=True` և դուք տրամադրում եք `EventCursor`:

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

Այլ հարմար մեթոդներ ներառում են `stream_pipeline_transactions`,
`stream_events` (տպագրված ֆիլտր կառուցողներով) և `stream_verifying_key_events`:

## 7. Հաջորդ քայլերը

- Ուսումնասիրեք `python/iroha_python/src/iroha_python/examples/`-ի օրինակները
  կառավարման, ISO կամուրջի օգնականների և Connect-ի համար ծայրից ծայր հոսքերի համար:
- Օգտագործեք `create_torii_client` / `resolve_torii_client_config`, երբ ցանկանում եք
  բեռնել հաճախորդը `iroha_config` JSON ֆայլից կամ միջավայրից:
- Norito RPC կամ Connect-ի հատուկ API-ների համար ստուգեք մասնագիտացված մոդուլները, ինչպիսիք են.
  `iroha_python.norito_rpc` և `iroha_python.connect`:

## Առնչվող Norito օրինակներ

- [Hajimari մուտքի կետի կմախք] (../norito/examples/hajimari-entrypoint) — արտացոլում է կոմպիլյացիան/գործարկումը
  աշխատանքային հոսք այս արագ մեկնարկից, որպեսզի կարողանաք գործարկել նույն մեկնարկային պայմանագիրը Python-ից:
- [Գրանցեք տիրույթի և դրամահատարանի ակտիվները] (../norito/examples/register-and-mint) — համապատասխանում է տիրույթին +
  Ակտիվը հոսում է վերևում և օգտակար է, երբ ցանկանում եք մատյանում իրականացնել SDK ստեղծողների փոխարեն:
- [Ակտիվների փոխանցում հաշիվների միջև] (../norito/examples/transfer-asset) — ցուցադրում է `transfer_asset`-ը
  syscall, որպեսզի կարողանաք համեմատել պայմանագրով պայմանավորված փոխանցումները Python-ի օգնական մեթոդների հետ:

Այս շինարարական բլոկների միջոցով դուք կարող եք իրականացնել Torii Python-ից առանց գրելու
ձեր սեփական HTTP սոսինձը կամ Norito կոդեկները: Քանի որ SDK-ն հասունանում է, լրացուցիչ բարձր մակարդակ
կավելացվեն շինարարներ; խորհրդակցեք README-ի հետ `python/iroha_python`-ում
տեղեկատու կարգավիճակի և միգրացիայի վերջին նշումների համար: