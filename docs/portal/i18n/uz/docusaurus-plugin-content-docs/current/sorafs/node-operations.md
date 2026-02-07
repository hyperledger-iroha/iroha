---
id: node-operations
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Nometall `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Sphinx to'plami tugagunga qadar ikkala versiyani ham sinxronlashtiring.
:::

## Umumiy ko'rinish

Ushbu runbook operatorlarni Torii ichiga o'rnatilgan `sorafs-node` joylashtirishni tekshirish orqali olib boradi. Har bir bo'lim to'g'ridan-to'g'ri SF-3 etkazib berish natijalariga mos keladi: pin/olib kelish, qayta tiklashni qayta boshlash, kvotani rad etish va PoR namunasini olish.

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
- Deklaratsiya yozib olingandan so'ng, tugun kutilgan quvvatni `GET /v1/sorafs/capacity/state` orqali reklama qilishini tasdiqlang.
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

### PoR isbotini mashq qilish

Operatorlar endi Torii ga yuklashdan oldin boshqaruv tomonidan chiqarilgan PoR artefaktlarini mahalliy sifatida qayta ko‘rishlari mumkin. CLI bir xil `sorafs-node` qabul qilish yo'lidan qayta foydalanadi, shuning uchun mahalliy yugurishlar HTTP API qaytaradigan aniq tekshirish xatolarini yuzaga chiqaradi.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Buyruq JSON xulosasini chiqaradi (manifest dayjesti, provayder identifikatori, dalil dayjesti, namunalar soni, ixtiyoriy hukm natijasi). Saqlangan manifest chaqiriqlar dayjestiga mos kelishini taʼminlash uchun `--manifest-id=<hex>` va audit dalillari uchun xulosani asl artefaktlar bilan arxivlamoqchi boʻlsangiz, `--json-out=<path>` ni taqdim eting. Jumladan, `--verdict` HTTP API-ga qo'ng'iroq qilishdan oldin butun sinov → dalil → hukmni oflayn rejimda takrorlash imkonini beradi.

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

## 5. Saqlash / GC tekshiruvi (faqat o'qish uchun)

1. Saqlash katalogiga qarshi mahalliy saqlashni skanerlang:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Faqat muddati o'tgan manifestlarni tekshiring (faqat quruq, o'chirilmaydi):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Xostlar yoki hodisalar bo'yicha hisobotlarni taqqoslashda baholash oynasini mahkamlash uchun `--now` yoki `--grace-secs` dan foydalaning.

GC CLI ataylab faqat o'qish uchun mo'ljallangan. Undan audit izlari uchun saqlash muddatlari va muddati o'tgan manifest inventarlarini olish uchun foydalaning; ishlab chiqarishda ma'lumotlarni qo'lda olib tashlamang.

## 6. PoR namuna olish probi

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

## 7. Avtomatlashtirish ilgaklari

- CI/tutun testlari qo'shilgan maqsadli tekshiruvlardan qayta foydalanishi mumkin:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection` va `por_sampling_returns_verified_proofs` ni qamrab oladi.
- Boshqaruv paneli quyidagilarni kuzatishi kerak:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` va `torii_sorafs_storage_fetch_inflight`
  - PoR muvaffaqiyat/qobiliyatsiz hisoblagichlari `/v1/sorafs/capacity/state` orqali paydo bo'ldi
  - `sorafs_node_deal_publish_total{result=success|failure}` orqali hisob-kitoblarni nashr etish urinishlari

Ushbu mashqlardan so'ng, o'rnatilgan xotira xodimi ma'lumotlarni qabul qilishini, qayta ishga tushirishda omon qolishini, sozlangan kvotani hurmat qilishini va tugun kengroq tarmoqqa sig'imini reklama qilishdan oldin aniqlangan PoR dalillarini yaratishini ta'minlaydi.