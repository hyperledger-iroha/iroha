---
lang: az
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Python SDK Quickstart

Python SDK (`iroha-python`) Rust müştəri köməkçilərini əks etdirir ki, siz
skriptlərdən, noutbuklardan və ya veb arxa uçlardan Torii ilə qarşılıqlı əlaqə qurun. Bu sürətli başlanğıc
quraşdırma, əməliyyatların təqdim edilməsi və hadisə axınını əhatə edir. Daha dərin üçün
əhatə dairəsi depoda `python/iroha_python/README.md`-ə baxın.

## 1. Quraşdırın

```bash
pip install iroha-python
```

İsteğe bağlı əlavələr:

- `pip install aiohttp` asinxron variantlarını işə salmağı planlaşdırırsınızsa
  axın köməkçiləri.
- `pip install pynacl`, SDK-dan kənarda Ed25519 açar əldə etməyə ehtiyacınız olduqda.

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

Səhifədən xəbərdar köməkçilər (məsələn, `list_accounts_typed`) obyekti qaytarır
həm `items`, həm də `next_cursor` ehtiva edir.

Hesab inventar köməkçiləri isteğe bağlı `asset_id` filtrini yalnız siz etdiyiniz zaman qəbul edir
müəyyən bir aktivə diqqət yetirin:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline V2 readiness

Use `GET /v1/offline/v2/readiness` through `get_offline_v2_readiness()` for offline feature discovery.
Offline V2 note issuance, redemption, and audit payloads are submitted as transaction instructions;
legacy offline allowance, reserve, revocation, transfer-history, and cash HTTP routes are no longer published by Torii.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
readiness = client.get_offline_v2_readiness()
print("offline notes", readiness.offline_note_v2)
```
## 6. Hadisələri yayımlayın

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

## 7. Növbəti addımlar

- `python/iroha_python/src/iroha_python/examples/` altındakı nümunələri araşdırın
  idarəetmə, ISO körpü köməkçiləri və Qoşulmanı əhatə edən uçdan-uca axınlar üçün.
- İstədiyiniz zaman `create_torii_client` / `resolve_torii_client_config` istifadə edin
  müştərini `iroha_config` JSON faylından və ya mühitindən yükləyin.
- Norito RPC və ya Connect üçün xüsusi API-lər üçün, məsələn, xüsusi modulları yoxlayın.
  `iroha_python.norito_rpc` və `iroha_python.connect`.

## Əlaqədar Norito nümunələri

- [Hajimari giriş nöqtəsi skeleti](../norito/examples/hajimari-entrypoint) — kompilyasiya/çalışmanı əks etdirir
  Python-dan eyni başlanğıc müqaviləsini yerləşdirə bilmək üçün bu sürətli başlanğıcdan iş axını.
- [Domen və pul aktivlərini qeydiyyatdan keçirin](../norito/examples/register-and-mint) — domenə uyğundur +
  aktiv yuxarıda axır və SDK qurucuları əvəzinə mühasibat kitabçası tərəfində tətbiq etmək istədiyiniz zaman faydalıdır.
- [Hesablar arasında aktiv köçürmə](../norito/examples/transfer-asset) — `transfer_asset`-i nümayiş etdirir
  syscall vasitəsilə müqaviləyə əsaslanan köçürmələri Python köməkçi metodları ilə müqayisə edə bilərsiniz.

Bu tikinti blokları ilə siz yazmadan Python-dan Torii məşq edə bilərsiniz
öz HTTP yapışqanınız və ya Norito kodekləriniz. SDK yetkinləşdikcə əlavə yüksək səviyyəli
inşaatçılar əlavə olunacaq; `python/iroha_python`-də README ilə məsləhətləşin
son status və miqrasiya qeydləri üçün kataloq.