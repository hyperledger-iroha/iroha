---
lang: ka
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

Python SDK (`iroha-python`) ასახავს Rust კლიენტის დამხმარეებს, ასე რომ თქვენ შეგიძლიათ
ურთიერთქმედება Torii-თან სკრიპტებიდან, ნოუთბუქებიდან ან ვებ გვერდიდან. ეს სწრაფი დაწყება
მოიცავს ინსტალაციას, ტრანზაქციის წარდგენას და ღონისძიების სტრიმინგს. უფრო ღრმად
დაფარვა იხილეთ `python/iroha_python/README.md` საცავში.

## 1. დააინსტალირეთ

```bash
pip install iroha-python
```

არჩევითი დამატებები:

- `pip install aiohttp` თუ გეგმავთ ასინქრონული ვარიანტების გაშვებას
  ნაკადის დამხმარეები.
- `pip install pynacl` როდესაც გჭირდებათ Ed25519 გასაღების დერივაცია SDK-ის გარეთ.

## 2. შექმენით კლიენტი და ხელმომწერები

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

`ToriiClient` იღებს საკვანძო სიტყვების დამატებით არგუმენტებს, როგორიცაა `timeout_ms`,
`max_retries` და `tls_config`. დამხმარე `resolve_torii_client_config`
აანალიზებს JSON კონფიგურაციის დატვირთვას, თუ გსურთ პარიტეტი Rust CLI-თან.

## 3. წარადგინეთ ტრანზაქცია

SDK აგზავნის ინსტრუქციების შემქმნელებს და ტრანზაქციის დამხმარეებს, ასე რომ თქვენ იშვიათად აშენებთ
Norito დატვირთვა ხელით:

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

`build_and_submit_transaction` აბრუნებს როგორც ხელმოწერილ კონვერტს, ასევე ბოლო
დაკვირვებული სტატუსი (მაგ., `Committed`, `Rejected`). თუ უკვე გაქვთ ხელმოწერილი
ტრანზაქციის კონვერტის გამოყენება `client.submit_transaction_envelope(envelope)` ან
JSON-ცენტრირებული `submit_transaction_json`.

## 4. შეკითხვის მდგომარეობა

ყველა REST ბოლო წერტილს აქვს JSON დამხმარეები და ბევრი ასახავს აკრეფილ მონაცემთა კლასებს. ამისთვის
მაგალითად, დომენების ჩამონათვალი:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

პაგინაციის მცოდნე დამხმარეები (მაგ., `list_accounts_typed`) აბრუნებენ ობიექტს, რომელიც
შეიცავს `items` და `next_cursor`.

## 5. მოვლენების სტრიმინგი

Torii SSE ბოლო წერტილები გამოვლენილია გენერატორების მეშვეობით. SDK ავტომატურად განახლდება
როდესაც `resume=True` და თქვენ მოგაწოდებთ `EventCursor`.

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

სხვა მოსახერხებელი მეთოდები მოიცავს `stream_pipeline_transactions`,
`stream_events` (აკრეფილი ფილტრის შემქმნელებით) და `stream_verifying_key_events`.

## 6. შემდეგი ნაბიჯები

- გამოიკვლიეთ მაგალითები `python/iroha_python/src/iroha_python/examples/`-ში
  მმართველობის, ISO ხიდის დამხმარეებისა და დაკავშირებისთვის, ბოლოდან ბოლომდე ნაკადებისთვის.
- გამოიყენეთ `create_torii_client` / `resolve_torii_client_config` როცა გინდათ
  ჩატვირთეთ კლიენტი `iroha_config` JSON ფაილიდან ან გარემოდან.
- Norito RPC ან Connect-სპეციფიკური API-ებისთვის, შეამოწმეთ სპეციალიზებული მოდულები, როგორიცაა
  `iroha_python.norito_rpc` და `iroha_python.connect`.

ამ სამშენებლო ბლოკებით შეგიძლიათ ივარჯიშოთ Torii პითონიდან წერის გარეშე
თქვენი საკუთარი HTTP წებო ან Norito კოდეკები. როგორც SDK მწიფდება, უფრო მაღალი დონის
დაემატება მშენებლები; მიმართეთ README-ს `python/iroha_python`-ში
დირექტორია უახლესი სტატუსისა და მიგრაციის შენიშვნებისთვის.