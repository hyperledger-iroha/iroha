---
lang: ka
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
- `pip install pynacl`, როდესაც გჭირდებათ Ed25519 გასაღების დერივაცია SDK-ს გარეთ.

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

`build_and_submit_transaction` აბრუნებს როგორც ხელმოწერილ კონვერტს, ასევე უკანასკნელს
დაკვირვებული სტატუსი (მაგ., `Committed`, `Rejected`). თუ უკვე გაქვთ ხელმოწერილი
ტრანზაქციის კონვერტის გამოყენება `client.submit_transaction_envelope(envelope)` ან
JSON-ცენტრული `submit_transaction_json`.

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

ანგარიშის ინვენტარის დამხმარეები იღებენ არასავალდებულო `asset_id` ფილტრს მხოლოდ მაშინ
ზრუნავს კონკრეტულ აქტივზე:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. ოფლაინ შემწეობები

გამოიყენეთ ოფლაინ შემწეობის საბოლოო წერტილები საფულის სერთიფიკატების გასაცემად და რეგისტრაციისთვის
ისინი წიგნში. `top_up_offline_allowance` აკავშირებს საკითხს + დაარეგისტრირებს ნაბიჯებს
(არ არსებობს ერთი დამატების საბოლოო წერტილი):

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
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

განახლებისთვის დარეკეთ `top_up_offline_allowance_renewal` მიმდინარე სერტიფიკატის ID-ით:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

თუ ნაკადის გაყოფა გჭირდებათ, დარეკეთ `issue_offline_certificate` (ან
`issue_offline_certificate_renewal`) მოჰყვა `register_offline_allowance`
ან `renew_offline_allowance`.

## 6. მოვლენების სტრიმინგი

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

## 7. შემდეგი ნაბიჯები

- გამოიკვლიეთ მაგალითები `python/iroha_python/src/iroha_python/examples/`-ში
  მმართველობის, ISO ხიდის დამხმარეებისა და დაკავშირებისთვის, ბოლოდან ბოლომდე ნაკადებისთვის.
- გამოიყენეთ `create_torii_client` / `resolve_torii_client_config` როცა გინდათ
  ჩატვირთეთ კლიენტი `iroha_config` JSON ფაილიდან ან გარემოდან.
- Norito RPC ან Connect-სპეციფიკური API-ებისთვის, შეამოწმეთ სპეციალიზებული მოდულები, როგორიცაა
  `iroha_python.norito_rpc` და `iroha_python.connect`.

## დაკავშირებული Norito მაგალითები

- [Hajimari შესვლის წერტილის ჩონჩხი] (../norito/examples/hajimari-entrypoint) — ასახავს კომპილაციას/გაშვებას
  სამუშაო პროცესი ამ სწრაფი დაწყებიდან, ასე რომ თქვენ შეგიძლიათ განათავსოთ იგივე დამწყები კონტრაქტი პითონიდან.
- [დომენის და ზარაფხანის აქტივების რეგისტრაცია] (../norito/examples/register-and-mint) — შეესაბამება დომენს +
  აქტივი მიედინება ზემოთ და გამოსადეგია, როცა SDK შემქმნელების ნაცვლად გსურთ ledger-ის განხორციელება.
- [აქტივის გადაცემა ანგარიშებს შორის](../norito/examples/transfer-asset) — აჩვენებს `transfer_asset`
  syscall, ასე რომ თქვენ შეგიძლიათ შეადაროთ კონტრაქტზე ორიენტირებული გადარიცხვები Python დამხმარე მეთოდებთან.

ამ სამშენებლო ბლოკებით შეგიძლიათ ივარჯიშოთ Torii პითონიდან წერის გარეშე
თქვენი საკუთარი HTTP წებო ან Norito კოდეკები. როგორც SDK მწიფდება, უფრო მაღალი დონის
დაემატება მშენებლები; მიმართეთ README-ს `python/iroha_python`-ში
დირექტორია უახლესი სტატუსისა და მიგრაციის შენიშვნებისთვის.