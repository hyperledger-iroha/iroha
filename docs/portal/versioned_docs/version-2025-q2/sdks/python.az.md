---
lang: az
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

Python SDK (`iroha-python`) Rust müştəri köməkçilərini əks etdirir ki, siz
skriptlərdən, noutbuklardan və ya veb backendlərdən Torii ilə qarşılıqlı əlaqə qurun. Bu sürətli başlanğıc
quraşdırma, əməliyyatların təqdim edilməsi və hadisə axınını əhatə edir. Daha dərin üçün
əhatə dairəsi depoda `python/iroha_python/README.md`-ə baxın.

## 1. Quraşdırın

```bash
pip install iroha-python
```

İsteğe bağlı əlavələr:

- `pip install aiohttp` asinxron variantlarını işə salmağı planlaşdırırsınızsa
  axın köməkçiləri.
- `pip install pynacl`, SDK-dan kənarda Ed25519 açarının əldə edilməsinə ehtiyacınız olduqda.

## 2. Müştəri və imzalayanlar yaradın

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

`ToriiClient`, `timeout_ms` kimi əlavə açar söz arqumentlərini qəbul edir,
`max_retries` və `tls_config`. Köməkçi `resolve_torii_client_config`
Rust CLI ilə paritet istəyirsinizsə, JSON konfiqurasiya yükünü təhlil edir.

## 3. Əməliyyat təqdim edin

SDK təlimat qurucuları və əməliyyat köməkçiləri göndərir ki, siz nadir hallarda qurursunuz
Norito əl ilə faydalı yüklər:

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

`build_and_submit_transaction` həm imzalanmış zərfi, həm də sonuncunu qaytarır
müşahidə statusu (məsələn, `Committed`, `Rejected`). Əgər artıq imzanız varsa
əməliyyat zərfində `client.submit_transaction_envelope(envelope)` və ya istifadə edin
JSON mərkəzli `submit_transaction_json`.

## 4. Sorğu vəziyyəti

Bütün REST son nöqtələrində JSON köməkçiləri və çoxlu tipli məlumat sinifləri var. üçün
Məsələn, domenlərin siyahısı:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Səhifədən xəbərdar köməkçilər (məsələn, `list_accounts_typed`) obyekti qaytarır ki,
həm `items`, həm də `next_cursor` ehtiva edir.

## 5. Hadisələri yayımlayın

Torii SSE son nöqtələri generatorlar vasitəsilə ifşa olunur. SDK avtomatik olaraq davam edir
`resume=True` və siz `EventCursor` təqdim etdiyiniz zaman.

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

Digər rahatlıq üsullarına `stream_pipeline_transactions`,
`stream_events` (yazılmış filtr qurucuları ilə) və `stream_verifying_key_events`.

## 6. Növbəti addımlar

- `python/iroha_python/src/iroha_python/examples/` altındakı nümunələri araşdırın
  idarəetmə, ISO körpü köməkçiləri və Qoşulmanı əhatə edən uçdan-uca axınlar üçün.
- İstədiyiniz zaman `create_torii_client` / `resolve_torii_client_config` istifadə edin
  müştərini `iroha_config` JSON faylından və ya mühitindən yükləyin.
- Norito RPC və ya Connect üçün xüsusi API-lər üçün, məsələn, xüsusi modulları yoxlayın.
  `iroha_python.norito_rpc` və `iroha_python.connect`.

Bu tikinti blokları ilə siz yazmadan Python-dan Torii məşq edə bilərsiniz
öz HTTP yapışqanınız və ya Norito kodekləriniz. SDK yetkinləşdikcə əlavə yüksək səviyyəli
inşaatçılar əlavə olunacaq; `python/iroha_python`-də README ilə məsləhətləşin
son status və miqrasiya qeydləri üçün kataloq.