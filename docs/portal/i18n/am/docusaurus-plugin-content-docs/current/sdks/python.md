---
lang: am
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
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
- ከኤስዲኬ ውጭ የ Ed25519 ቁልፍ መውጣቱ ሲፈልጉ `pip install pynacl`።

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
JSON-centric I18NI0000032X.

## 4. የመጠይቅ ሁኔታ

ሁሉም የ REST የመጨረሻ ነጥቦች JSON አጋዥዎች አሏቸው እና ብዙዎቹ የተተየቡ የመረጃ ክፍሎችን አጋልጠዋል። ለ
ለምሳሌ፣ ጎራዎችን መዘርዘር፡-

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

በገጽ የሚያውቁ ረዳቶች (ለምሳሌ፡ `list_accounts_typed`) ዕቃውን የሚመልሱ
ሁለቱንም `items` እና I18NI0000035X ይዟል።

የመለያ ቆጠራ ረዳቶች እርስዎ ብቻ ሲሆኑ አማራጭ `asset_id` ማጣሪያ ይቀበላሉ።
ለአንድ የተወሰነ ንብረት ትኩረት ይስጡ;

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. ከመስመር ውጭ አበል

የኪስ ቦርሳ የምስክር ወረቀቶችን ለመስጠት እና ለመመዝገብ ከመስመር ውጭ አበል የመጨረሻ ነጥቦችን ይጠቀሙ
በእነሱ ላይ-መሪ. `top_up_offline_allowance` ጉዳዩን በሰንሰለት ይይዛል + ደረጃዎችን ይመዝግቡ
(ምንም ነጠላ የመጨመሪያ ነጥብ የለም)

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
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
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

ለማደስ፣ አሁን ካለው የምስክር ወረቀት መታወቂያ ጋር I18NI0000038X ይደውሉ፡

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

ፍሰቱን መከፋፈል ከፈለጉ `issue_offline_certificate` ይደውሉ (ወይም
`issue_offline_certificate_renewal`) ተከትሎ I18NI0000041X
ወይም `renew_offline_allowance`.

## 6. የዥረት ክስተቶች

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

## 7. ቀጣይ ደረጃዎች

- ምሳሌዎችን በ `python/iroha_python/src/iroha_python/examples/` ስር ያስሱ
  አስተዳደርን ለሚሸፍኑ ከጫፍ እስከ ጫፍ ፍሰቶች፣ የ ISO ድልድይ አጋዥ እና ኮኔክሽን።
- ሲፈልጉ `create_torii_client` / `resolve_torii_client_config` ይጠቀሙ
  ደንበኛውን ከ I18NI0000051X JSON ፋይል ወይም አካባቢ ማስነሳት።
- ለNorito RPC ወይም Connect-specific APIs እንደ ልዩ ሞጁሎችን ይመልከቱ።
  `iroha_python.norito_rpc` እና `iroha_python.connect`።

## ተዛማጅ Norito ምሳሌዎች

- [የሀጂማሪ መግቢያ ነጥብ አጽም](../norito/examples/hajimari-entrypoint) - ማጠናቀር/ሩጡን ያንጸባርቃል
  የስራ ፍሰት ከዚህ ፈጣን ጅምር ስለዚህ ተመሳሳዩን የማስጀመሪያ ውል ከ Python ማሰማራት ይችላሉ።
- [ጎራ እና ሚንት ንብረቶችን ይመዝገቡ](../norito/examples/register-and-mint) - ከጎራው ጋር ይዛመዳል +
  ንብረቱ ከላይ ይፈስሳል እና ከኤስዲኬ ግንበኞች ይልቅ የሒሳብ-ጎን ትግበራ ሲፈልጉ ጠቃሚ ነው።
- [ንብረቱን በመለያዎች መካከል ያስተላልፉ](../norito/examples/transfer-asset) - `transfer_asset` ያሳያል
  syscall ስለዚህ በኮንትራት የሚመሩ ዝውውሮችን ከ Python አጋዥ ዘዴዎች ጋር ማወዳደር ይችላሉ።

በእነዚህ የግንባታ ብሎኮች Torii ከ Python ሳይጽፉ ልምምድ ማድረግ ይችላሉ።
የራስዎን የኤችቲቲፒ ሙጫ ወይም I18NT0000003X ኮዴኮች። ኤስዲኬ ሲያድግ፣ ተጨማሪ ከፍተኛ-ደረጃ
ግንበኞች ይጨምራሉ; በ I18NI0000055X ውስጥ README ን ያማክሩ
የቅርብ ጊዜ ሁኔታ እና የስደት ማስታወሻዎች ማውጫ።