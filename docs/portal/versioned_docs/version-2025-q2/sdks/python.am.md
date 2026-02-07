---
lang: am
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

የ Python ኤስዲኬ (`iroha-python`) የ Rust ደንበኛ ረዳቶችን ያንጸባርቃል ስለዚህ እንዲችሉ
ከስክሪፕቶች፣ ከማስታወሻ ደብተሮች ወይም ከድር ጀርባዎች ከ Torii ጋር መስተጋብር መፍጠር። ይህ ፈጣን ጅምር
ጭነትን፣ የግብይት ግቤትን እና የክስተት ዥረትን ይሸፍናል። ለበለጠ
ሽፋን በማከማቻው ውስጥ `python/iroha_python/README.md` ይመልከቱ።

## 1. ጫን

```bash
pip install iroha-python
```

አማራጭ ተጨማሪዎች፡-

- `pip install aiohttp` ያልተመሳሰሉ ልዩነቶችን ለማሄድ ካቀዱ
  የዥረት ረዳቶች.
- ከኤስዲኬ ውጭ የ Ed25519 ቁልፍ መውጣቱን ሲፈልጉ `pip install pynacl`።

## 2. ደንበኛ እና ፈራሚዎችን ይፍጠሩ

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

`ToriiClient` እንደ `timeout_ms` ያሉ ተጨማሪ የቁልፍ ቃል ነጋሪ እሴቶችን ይቀበላል።
`max_retries`፣ እና `tls_config`። ረዳት `resolve_torii_client_config`
ከ Rust CLI ጋር መመሳሰል ከፈለጉ የJSON ውቅር ክፍያን ይተነትናል።

## 3. ግብይት አስገባ

ኤስዲኬ የማስተማሪያ ግንበኞችን እና የግብይት አጋሮችን ይልካል።
Norito የሚጫኑ ጭነቶች በእጅ፡

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

`build_and_submit_transaction` የተፈረመውን እና የመጨረሻውን ሁለቱንም ይመልሳል
የታየበት ሁኔታ (ለምሳሌ፣ `Committed`፣ `Rejected`)። አስቀድሞ የተፈረመ ከሆነ
የግብይት ኤንቨሎፕ `client.submit_transaction_envelope(envelope)` ወይም የ ይጠቀሙ
JSON-centric `submit_transaction_json`.

## 4. የመጠይቅ ሁኔታ

ሁሉም የ REST የመጨረሻ ነጥቦች JSON አጋዥዎች አሏቸው እና ብዙዎቹ የተተየቡ የመረጃ ክፍሎችን አጋልጠዋል። ለ
ለምሳሌ፣ ጎራዎችን መዘርዘር፡-

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

በገጽ የሚያውቁ ረዳቶች (ለምሳሌ፡ `list_accounts_typed`) የሚመልስ ነገር
ሁለቱንም `items` እና `next_cursor` ይዟል።

## 5. የዥረት ክስተቶች

Torii SSE የመጨረሻ ነጥቦች በጄነሬተሮች በኩል ይጋለጣሉ። ኤስዲኬ በራስ-ሰር ከቆመበት ይቀጥላል
`resume=True` እና `EventCursor` ሲያቀርቡ።

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

ሌሎች የምቾት ዘዴዎች `stream_pipeline_transactions`፣
`stream_events` (ከተተየቡ የማጣሪያ ግንበኞች ጋር) እና `stream_verifying_key_events`።

## 6. ቀጣይ ደረጃዎች

- ምሳሌዎችን በ `python/iroha_python/src/iroha_python/examples/` ስር ያስሱ
  አስተዳደርን ለሚሸፍኑ ከጫፍ እስከ ጫፍ ፍሰቶች፣ የ ISO ድልድይ አጋዥ እና ኮኔክሽን።
- ሲፈልጉ `create_torii_client`/`resolve_torii_client_config` ይጠቀሙ
  ደንበኛውን ከ `iroha_config` JSON ፋይል ወይም አካባቢ ማስነሳት ።
- ለNorito RPC ወይም Connect-specific APIs እንደ ልዩ ሞጁሎችን ይመልከቱ።
  `iroha_python.norito_rpc` እና `iroha_python.connect`።

በነዚህ የግንባታ ብሎኮች Torii ከ Python ሳትጽፉ ልምምድ ማድረግ ትችላለህ
የራስዎን የኤችቲቲፒ ሙጫ ወይም Norito ኮዴኮች። ኤስዲኬ ሲያድግ፣ ተጨማሪ ከፍተኛ-ደረጃ
ግንበኞች ይጨምራሉ; በ `python/iroha_python` ውስጥ README ን ያማክሩ
የቅርብ ጊዜ ሁኔታ እና የስደት ማስታወሻዎች ማውጫ።