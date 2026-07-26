---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# པའི་ཐོན་ཨེསི་ཌི་ཀེ་ མགྱོགས་མཐའ།

པའི་ཐོན་ཨེསི་ཌི་ཀེ་ (`iroha-python`) གིས་ རཱསི་ཊི་ མཁོ་སྤྲོད་པ་ གྲོགས་རམ་འབད་མི་ཚུ་ མེ་ལོང་ནང་ བཙུགས་ཏེ་ ཁྱོད་ཀྱིས་འབད་ཚུགས།
ཡིག་གཟུགས་དང་དྲན་དེབ་ ཡང་ན་ ཝེབ་རྒྱབ་ཐག་ཚུ་ལས་ I1NT00000003X དང་ཅིག་ཁར་ འབྲེལ་བ་འཐབ་དགོ། འདི་མགྱོགས་འགོ་ཚུགས།
གཞི་བཙུགས་དང་ ཚོང་འབྲེལ་ཕུལ་ནི་ དེ་ལས་ བྱུང་ལས་རྒྱུན་ལམ་ཚུ་ ཁྱབ་ཚུགསཔ་ཨིན། གཏིང་ཟབ་པའི་དོན་ལུ།
ཁྱབ་ཚད་བལྟ། `python/iroha_python/README.md` མཛོད་ཁང་ནང་།

## 1. གཞི་བཙུགས་འབད་ནི།

```bash
pip install iroha-python
```

གདམ་ཁ་ཅན་གྱི་ཁ་སྐོང་:

- ཁྱོད་ཀྱིས་ མཉམ་མཐུན་མེད་པའི་འགྱུར་བ་ཚུ་ གཡོག་བཀོལ་ནི་གི་འཆར་གཞི་བརྩམས་ཏེ་ཡོད་པ་ཅིན་ Torii
  རྒྱུན་སྤེལ་གྱི་གྲོགས་རམ་པ་ཚུ།
- ཁྱོད་ལུ་ ཨེསི་ཌི་ཀེ་གི་ཕྱི་ཁར་ ཨི་ཌི་༢༥༥༡༩ ལྡེ་མིག་འབྱུང་ཁུངས་དགོ་པའི་སྐབས་ `pip install pynacl` ཨིན།

## 2. མཁོ་མངགས་དང་མིང་རྟགས་བཀོད་མི་ཅིག་གསར་བསྐྲུན་འབད།

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

`ToriiClient` གིས་ `timeout_ms` བཟུམ་གྱི་ ལྡེ་མིག་ཚིག་ཡིག་སྒྲུབ་རྟགས་ཁ་སྐོང་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན།
`max_retries`, དང་ `tls_config`. རོགས་རམ་འབད་མི་ `resolve_torii_client_config`
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

`build_and_submit_transaction` མིང་རྟགས་བཀོད་ཡོད་པའི་ཡིག་ཤུབས་དང་མཇུག་གཉིས་ཆ་རང་སླར་ལོག་འབདཝ་ཨིན།
བལྟ་རྟོག་འབད་ཡོད་པའི་གནས་རིམ་ (དཔེར་ན་ `Committed`, `Rejected`). རྟགས་བཀོད་ཡོད་པ་ཅིན།
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

## 5. རྒྱུན་རིམ་གྱི་བྱུང་རིམ།

Torii SSE མཐའ་མཚམས་ཚུ་ གློག་ཤུགས་འཕྲུལ་ཆས་བརྒྱུད་དེ་ གསལ་སྟོན་འབད་ཡོདཔ་ཨིན། ཨེསི་ཌི་ཀེ་ རང་བཞིན་གྱིས་ སླར་འབྱུང་འབདཝ་ཨིན།
དང་ `resume=True` དང་ ཁྱོད་ཀྱིས་ `EventCursor` བྱིནམ་ཨིན།

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

## 6. གོ་རིམ་རྗེས་མ།

- `python/iroha_python/src/iroha_python/examples/` འོག་ལུ་དཔེ་ཚུ་བལྟ།
  མཐའ་མཇུག་ལས་མཇུག་ཚུན་ཚོད་ གཞུང་སྐྱོང་དང་ ISO ཟམ་གྱི་གྲོགས་རམ་པ་ དེ་ལས་ མཐུད་འབྲེལ་གྱི་དོན་ལུ་ཨིན།
- ཁྱོད་ཀྱིས་དགོ་འདོད་ཡོད་པའི་སྐབས་ `create_torii_client` / `resolve_torii_client_config` ལག་ལེན་འཐབ།
  མཁོ་སྤྲོད་འབད་མི་འདི་ `iroha_config` JSON ཡིག་སྣོད་ཡང་ན་མཐའ་འཁོར་ལས་ བུཊི་པ་ཨིན།
- Norito RPC ཡང་ན་ Connect-specific APIs གི་དོན་ལུ་ དམིགས་བསལ་གྱི་ཚད་གཞི་ཚུ་ དཔེར་ན་ བཟུམ།
  `iroha_python.norito_rpc` དང་ `iroha_python.connect`.

འདི་དག་གིས་ ཁྱོད་ཀྱིས་ Torii འདི་ བྲིས་མ་དགོ་པར་ པའི་ཐོན་ལས་ ལག་ལེན་འཐབ་ཚུགས།
ཁྱོད་རའི་ཨེཆ་ཊི་ཊི་པི་གུ་ལུ་ཡང་ན་ Norito གསང་གྲངས་ཚུ། ཨེསི་ཌི་ཀེ་ རྒས་འགྱོཝ་ད་ མཐོ་རིམ་ཁ་སྐོང་ ཁ་སྐོང་།
བཟོ་བསྐྲུན་པ་ཚུ་ཁ་སྐོང་འབད་འོང་། README འདི་ `python/iroha_python` ནང་ལུ་དང་ འཁྲིལ་དགོ།
གསར་ཤོས་གནས་རིམ་དང་ གནས་སྤོའི་དྲན་ཐོ་ཚུ་གི་དོན་ལུ་ སྣོད་ཐོ།