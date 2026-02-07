---
lang: dz
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T18:06:01.646084+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# པའི་ཐོན་ཨེསི་ཌི་ཀེ་ མགྱོགས་མཐའ།

པའི་ཐོན་ཨེསི་ཌི་ཀེ་ (`iroha-python`) གིས་ རཱསི་ཊི་ མཁོ་མངགས་འབད་མི་གྲོགས་རམ་པ་ཚུ་ མེ་ལོང་སྦེ་ འབདཝ་ལས་ ཁྱོད་ཀྱིས་འབད་ཚུགས།
ཡིག་གཟུགས་དང་ དྲན་དེབ་ ཡང་ན་ ཝེབ་རྒྱབ་རྟེན་ཚུ་ལས་ Torii དང་ཅིག་ཁར་ འབྲེལ་བ་འཐབ་དགོ། འདི་མགྱོགས་འགོ་ཚུགས།
གཞི་བཙུགས་དང་ ཚོང་འབྲེལ་ཕུལ་ནི་ དེ་ལས་ བྱུང་ལས་རྒྱུན་ལམ་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན། གཏིང་ཟབ་པའི་དོན་ལུ།
ཁྱབ་ཚད་བལྟ། `python/iroha_python/README.md` མཛོད་ཁང་ནང་།

## 1. གཞི་བཙུགས་འབད་ནི།

I18NF0000008X

གདམ་ཁ་ཅན་གྱི་ཁ་སྐོང་:

- ཁྱོད་ཀྱིས་ མཉམ་མཐུན་མེད་པའི་འགྱུར་ཅན་ཚུ་ གཡོག་བཀོལ་ནི་གི་འཆར་གཞི་བརྩམས་ཏེ་ཡོད་པ་ཅིན་ I18NI000000021X
  རྒྱུན་སྤེལ་གྱི་གྲོགས་རམ་པ་ཚུ།
- ཁྱོད་ལུ་ ཨེསི་ཌི་ཀེ་གི་ཕྱི་ཁར་ ཨི་ཌི་༢༥༥༡༩ ལྡེ་མིག་འབྱུང་ཁུངས་དགོ་པའི་སྐབས་ I18NI0000002X ཨིན།

## 2. མཁོ་མངགས་དང་མིང་རྟགས་བཀོད་མི་ཅིག་གསར་བསྐྲུན་འབད།

I18NF0000009X

I18NI000000023X གིས་ `timeout_ms`, བཟུམ།
I18NI0000025X, དང་ I18NI0000026X. གྲོགས་རམ་འབད་མི་ `resolve_torii_client_config`
ཁྱོད་ཀྱིས་ Rust CLI དང་ཅིག་ཁར་ མཉམ་མཐུན་དགོ་པ་ཅིན་ JSON རིམ་སྒྲིག་སྤྲོད་ལེན་ཅིག་ མིང་དཔྱད་འབདཝ་ཨིན།

## 3. ཚོང་འབྲེལ་ཅིག་ཕུལ་ནི།

ཨེསི་ཌི་ཀེ་གིས་ བཀོད་རྒྱ་བཟོ་བསྐྲུན་པ་དང་ ཚོང་འབྲེལ་གྲོགས་རམ་པ་ཚུ་ སྐྱེལ་འདྲེན་འབདཝ་ལས་ ཁྱོད་ཀྱིས་ དཀོན་དྲགས་སྦེ་བཟོ་བསྐྲུན་འབདཝ་ཨིན།
Norito ལག་པ་གིས་དངུལ་སྤྲོད་ནི།

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

I18NI000000028X གིས་ མིང་རྟགས་བཀོད་ཡོད་པའི་ཡིག་ཤུབས་དང་ མཇུག་གཉིས་ཆ་རང་སླར་ལོག་འབདཝ་ཨིན།
བལྟ་རྟོག་འབད་ཡོད་པའི་གནས་རིམ་ (དཔེར་ན་ I18NI0000029X, `Rejected`). རྟགས་བཀོད་ཡོད་པ་ཅིན།
བརྗེ་སོར་གྱི་ཡིག་ཤུབས་ལག་ལེན། `client.submit_transaction_envelope(envelope)` ཡང་ན་ th
JSON-ལྟེ་བ་ `submit_transaction_json`.

## 4. འདྲི་དཔྱད་གནས་སྟངས།

REST མཇུག་སྣོད་ཚུ་ཆ་མཉམ་ལུ་ JSON གྲོགས་རམ་པ་དང་ ཡིག་དཔར་རྐྱབས་ཡོད་པའི་གནད་སྡུད་དབྱེ་རིམ་མང་ཤོས་ཅིག་ཡོདཔ་ཨིན། དོན་ལུ
དཔེར་ན་ ཐོ་ཡིག་མངའ་ཁོངས་ཚུ་:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Pagination-appro གྲོགས་རམ་པ་ (དཔེར་ན་ `list_accounts_typed`) དངོས་པོ་ཅིག་ དེ་སླར་ལོག་འབདཝ་ཨིན།
`items` དང་ `next_cursor` གཉིས་ཀ་ཡོད།

རྩིས་ཐོ་ ཐོ་བཀོད་ཀྱི་གྲོགས་རམ་པ་ཚུ་གིས་ ཁྱོད་རྐྱངམ་ཅིག་སྦེ་ཡོད་པའི་སྐབས་ གདམ་ཁ་ཅན་གྱི་ `asset_id` ཚགས་མ་འདི་དང་ལེན་འབདཝ་ཨིན།
དམིགས་བསལ་རྒྱུ་དངོས་ཅིག་ལུ་བདག་འཛིན་འཐབ་ནི།

```python
asset_id = "rose#wonderland#alice@test"
assets = client.list_account_assets("alice@test", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("alice@test", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. ཟུར་ཐོའི་འཐུས་སྐོར།

དངུལ་ཁུག་ལག་ཁྱེར་སྤྲོད་ཞིནམ་ལས་ ཐོ་བཀོད་འབད་ནི་ལུ་ ཨོཕ་ལ་ཡིན་གྱི་འཐུས་ཚུ་ལག་ལེན་འཐབ།
དེ་ཚུ་ ཞལ་འཛོམས་ནང་ལུ། I18NI000000037X གནད་དོན་+ ཐོ་བཀོད་གོ་རིམ་གྱི་རྒྱུན་རིམ་ཚུ།
(མགོ་ཐོག་མཐའ་མཇུག་གཅིག་ཡང་མེད།):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "ih58:...",
    "allowance": {"asset": "usd#wonderland", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

བསྐྱར་གསོ་འབད་ནིའི་དོན་ལུ་ ད་ལྟོའི་ལག་ཁྱེར་ཨའི་ཌི་: དང་གཅིག་ཁར་ I18NI000000038X ལུ་ཁ་པར་གཏང་།

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

ཁྱོད་ཀྱིས་ ཕོལོ་འདི་ བགོ་བཤའ་རྐྱབ་དགོ་པ་ཅིན་ `issue_offline_certificate` ལུ་ ཁ་བརྡ་འབད།
`issue_offline_certificate_renewal` དེ་ནས་`register_offline_allowance`
ཡང་ན་ `renew_offline_allowance`.

## 6. རྒྱུན་རིམ་གྱི་བྱུང་རིམ།

Torii SSE མཐའ་མཚམས་ཚུ་ གློག་ཤུགས་འཕྲུལ་ཆས་བརྒྱུད་དེ་ གསལ་སྟོན་འབད་ཡོདཔ་ཨིན། ཨེསི་ཌི་ཀེ་ རང་བཞིན་གྱིས་ སླར་འབྱུང་འབདཝ་ཨིན།
`resume=True` དང་ ཁྱོད་ཀྱིས་ `EventCursor` བྱིནམ་ཨིན།

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

གཞན་ཡང་སྟབས་བདེ་བའི་ཐབས་ལམ་ནང་ `stream_pipeline_transactions`, དང་།
`stream_events` (ཡིག་དཔར་རྐྱབ་ཡོད་པའི་ཚགས་མ་བཟོ་མི་ཚུ་དང་གཅིག་ཁར་) དང་ `stream_verifying_key_events`.

## 7. གོམ་པ་ཤུལ་མམ།

- I18NI0000048X འོག་ལུ་དཔེ་ཚུ་བལྟ།
  མཐའ་མཇུག་ལས་མཇུག་ཚུན་ཚོད་ གཞུང་སྐྱོང་དང་ ISO ཟམ་གྱི་གྲོགས་རམ་པ་ དེ་ལས་ མཐུད་འབྲེལ་གྱི་དོན་ལུ་ཨིན།
- ཁྱོད་ཀྱིས་འདོད་པའི་དུས་ལུ་ `create_torii_client` / I18NI000000050X ལག་ལེན་འཐབ།
  མཁོ་སྤྲོད་འབད་མི་འདི་ `iroha_config` JSON ཡིག་སྣོད་ཡང་ན་མཐའ་འཁོར་ལས་ བུཊི་པ་ཨིན།
- I18NT000000001X RPC ཡང་ན་ Connect-specific APIs གི་དོན་ལུ་ དམིགས་བསལ་གྱི་ཚད་གཞི་ཚུ་ དཔེར་ན་ བཟུམ།
  `iroha_python.norito_rpc` དང་ I18NI0000003X.

## འབྲེལ་ཡོད་ Norito དཔེར་ན།

- [ཧ་ཇི་མ་རི་འཛུལ་སྒོ་ ཀེང་རུས་](../norito/examples/hajimari-entrypoint) — བསྡུ་སྒྲིག་/རྒྱུག་འགྲན།
  ལཱ་གི་རྒྱུན་རིམ་འདི་མགྱོགས་འགོ་བཙུགས་ཡོདཔ་ལས་ ཁྱོད་ཀྱིས་ པའི་ཐོན་ལས་ འགོ་བཙུགས་པའི་གན་རྒྱ་གཅིག་བཀྲམ་སྤེལ་འབད་ཚུགས།
- [མངའ་ཁོངས་དང་ མིན་ཊི་རྒྱུ་དངོས་ཐོ་འགོད་](../norito/examples/register-and-mint) — མངའ་ཁོངས་ + དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
  རྒྱུ་དངོས་འདི་ གོང་ལུ་བཞུརཝ་ཨིནམ་དང་ ཁྱོད་ཀྱིས་ ཨེསི་ཌི་ཀེ་བཟོ་བསྐྲུན་པ་ཚུ་གི་ཚབ་ལུ་ ལེ་ཇར་ཕྱོགས་ལག་ལེན་འཐབ་དགོཔ་ད་ ཕན་ཐོགས་ཅན་ཅིག་ཨིན།
- [རྩིས་ཐོ་ཚུ་གི་བར་ན་ ཊཱན་སིཕ་རྒྱུ་དངོས་ ](../norito/examples/transfer-asset) — `transfer_asset` སྟོནམ་ཨིན།
  syscall དེ་ ཁྱོད་ཀྱིས་ པའི་ཐོན་གྲོགས་རམ་གྱི་ཐབས་ལམ་ཚུ་དང་ གན་རྒྱ་དང་འཁྲིལ་ སྤོ་བཤུད་འབད་མི་ཚུ་ ག་བསྡུར་འབད་ཚུགས།

འདི་དག་གིས་ ཁྱོད་ཀྱིས་ མ་བྲིས་པར་ པའི་ཐོན་ལས་ Torii ལག་ལེན་འཐབ་ཚུགས།
ཁྱོད་རའི་ཨེཆ་ཊི་ཊི་པི་གུ་ལུ་ཡང་ན་ Norito གསང་གྲངས་ཚུ། ཨེསི་ཌི་ཀེ་ རྒས་འགྱོཝ་ད་ མཐོ་རིམ་ཁ་སྐོང་ ཁ་སྐོང་།
བཟོ་བསྐྲུན་པ་ཚུ་ཁ་སྐོང་འབད་འོང་། README འདི་ I18NI000005X ནང་དང་ འཁྲིལ་དགོ།
གསར་ཤོས་གནས་རིམ་དང་ གནས་སྤོའི་དྲན་ཐོ་ཚུ་གི་དོན་ལུ་ སྣོད་ཐོ།