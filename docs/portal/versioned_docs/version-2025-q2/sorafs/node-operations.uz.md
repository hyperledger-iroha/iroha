---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations-uz
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
slug: /sorafs/node-operations-uz
---

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Ikkala nusxani ham nashrlar bo'ylab tekislang.
:::

## Umumiy ko'rinish

Ushbu runbook operatorlarga Torii ichiga o'rnatilgan `sorafs-node` o'rnatilishini tekshirish orqali boradi. Har bir bo'lim to'g'ridan-to'g'ri SF-3 etkazib berish natijalariga mos keladi: pin/olib kelish, qayta tiklashni qayta boshlash, kvotani rad etish va PoR namunasini olish.

## 1. Old shartlar

- `torii.sorafs.storage` da saqlash ishchisini yoqing:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Torii jarayonining `data_dir` ga o'qish/yozish ruxsati borligiga ishonch hosil qiling.
- Deklaratsiya qayd etilgandan so'ng, tugun kutilgan quvvatni `GET /v1/sorafs/capacity/state` orqali reklama qilishini tasdiqlang.
- Yumshoqlash yoqilganda, asboblar panellari nuqta qiymatlari bilan bir qatorda jittersiz tendentsiyalarni ta'kidlash uchun ham xom, ham silliqlangan GiB·soat/PoR hisoblagichlarini ko'rsatadi.

### CLI quruq yugurish (ixtiyoriy)

HTTP so'nggi nuqtalarini ko'rsatishdan oldin siz to'plamdagi CLI bilan saqlash orqa qismini aqlliligini tekshirishingiz mumkin.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

Buyruqlar Norito JSON xulosalarini chop etadi va chunk-profil yoki digest nomuvofiqliklarini rad etadi, bu ularni Torii simlarini ulashdan oldin CI tutunini tekshirish uchun foydali qiladi.【crates/sorafs_node/tests/cli.rs#L1】

Torii jonli efirga chiqqach, HTTP orqali bir xil artefaktlarni olishingiz mumkin:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ikkala so'nggi nuqta ham o'rnatilgan saqlash xodimi tomonidan xizmat qiladi, shuning uchun CLI tutun sinovlari va shlyuz problari sinxronlashtiriladi.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#1】L

## 2. PIN → Bog'lanishni olib kelish

1. Manifest + foydali yuk to'plamini yarating (masalan, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` bilan).
2. Manifestni base64 kodlash bilan yuboring:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON soʻrovida `manifest_b64` va `payload_b64` boʻlishi kerak. Muvaffaqiyatli javob `manifest_id_hex` va foydali yuk hazm qilishni qaytaradi.
3. Belgilangan ma'lumotlarni oling:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-`data_b64` maydonini dekodlang va uning asl baytlarga mos kelishini tekshiring.

## 3. Qayta tiklash matkapini qayta ishga tushiring

1. Yuqoridagi kabi kamida bitta manifestni mahkamlang.
2. Torii jarayonini (yoki butun tugunni) qayta ishga tushiring.
3. Olib olish so‘rovini qayta yuboring. Foydali yuk hali ham olinishi mumkin va qaytarilgan dayjest qayta ishga tushirishdan oldingi qiymatga mos kelishi kerak.
4. `bytes_used` qayta ishga tushirilgandan keyin davom etuvchi manifestlarni aks ettirishini tasdiqlash uchun `GET /v1/sorafs/storage/state` ni tekshiring.

## 4. Kvotani rad etish testi

1. `torii.sorafs.storage.max_capacity_bytes` ni vaqtincha kichik qiymatga tushiring (masalan, bitta manifest hajmi).
2. Bitta manifestni belgilang; so'rov muvaffaqiyatli bo'lishi kerak.
3. Shunga o'xshash o'lchamdagi ikkinchi manifestni o'rnatishga harakat qiling. Torii soʻrovni HTTP `400` va `storage capacity exceeded` oʻz ichiga olgan xato xabari bilan rad qilishi kerak.
4. Tugatgandan so'ng normal sig'im chegarasini tiklang.

## 5. PoR namuna olish probi

1. Manifestni mahkamlang.
2. PoR namunasini talab qiling:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Javobda so'ralgan hisob bilan `samples` borligini va har bir dalil saqlangan manifest ildiziga nisbatan tasdiqlanishini tekshiring.

## 6. Avtomatlashtirish ilgaklari

- CI/tutun testlari qo'shilgan maqsadli tekshiruvlardan qayta foydalanishi mumkin:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ````pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` va `por_sampling_returns_verified_proofs` ni qamrab oladi.
- Boshqaruv paneli quyidagilarni kuzatishi kerak:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` va `torii_sorafs_storage_fetch_inflight`
  - PoR muvaffaqiyat/qobiliyatsiz hisoblagichlari `/v1/sorafs/capacity/state` orqali paydo bo'ldi
  - `sorafs_node_deal_publish_total{result=success|failure}` orqali hisob-kitoblarni nashr etish urinishlari

Ushbu mashqlardan so'ng, o'rnatilgan xotira xodimi ma'lumotlarni qabul qilishini, qayta ishga tushirishda omon qolishini, sozlangan kvotani hurmat qilishini va tugun kengroq tarmoqqa sig'imini reklama qilishdan oldin aniqlangan PoR dalillarini yaratishini ta'minlaydi.
