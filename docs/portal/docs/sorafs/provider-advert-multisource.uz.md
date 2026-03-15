---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bb0965d4125aa2c3a3d483b63f4b36b1c6bf26406a2fd54e645e7a3c0156c264
source_last_modified: "2026-01-05T09:28:11.906678+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ko'p manbali provayder reklamalari va rejalashtirish

Ushbu sahifada kanonik spetsifikatsiyalar distillanadi
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Ushbu hujjatdan so'zma-so'z Norito sxemalari va o'zgarishlar jurnallari uchun foydalaning; portal nusxasi
operator yo'riqnomasini, SDK eslatmalarini va telemetriya ma'lumotnomalarini qolganlarga yaqin tutadi
SoraFS runbooks.

## Norito sxema qo'shimchalari

### Diapazon qobiliyati (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - har bir so'rov uchun eng katta qo'shni oraliq (baytlar), `≥ 1`.
- `min_granularity` - rezolyutsiyani qidiring, `1 ≤ value ≤ max_chunk_span`.
- `supports_sparse_offsets` - bitta so'rovda qo'shni bo'lmagan ofsetlarga ruxsat beradi.
- `requires_alignment` - rost bo'lsa, ofsetlar `min_granularity` bilan mos kelishi kerak.
- `supports_merkle_proof` - PoR guvohlarini qo'llab-quvvatlashni bildiradi.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` kanonik kodlashni amalga oshiradi
shuning uchun g'iybat yuklari deterministik bo'lib qoladi.

### `StreamBudgetV1`
- Maydonlar: `max_in_flight`, `max_bytes_per_sec`, ixtiyoriy `burst_bytes`.
- Tasdiqlash qoidalari (`StreamBudgetV1::validate`):
  - `max_in_flight ≥ 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, mavjud bo'lganda, `> 0` va `≤ max_bytes_per_sec` bo'lishi kerak.

### `TransportHintV1`
- Maydonlar: `protocol: TransportProtocol`, `priority: u8` (0–15 oynasi tomonidan kiritilgan
  `TransportHintV1::validate`).
- Ma'lum protokollar: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Provayder uchun takroriy protokol yozuvlari rad etiladi.

### `ProviderAdvertBodyV1` qo'shimchalar
- Majburiy emas `stream_budget: Option<StreamBudgetV1>`.
- Majburiy emas `transport_hints: Option<Vec<TransportHintV1>>`.
- Ikkala maydon ham endi `ProviderAdmissionProposalV1`, boshqaruv orqali oqadi
  konvertlar, CLI moslamalari va telemetrik JSON.

## Tasdiqlash va boshqaruv majburiyligi

`ProviderAdvertBodyV1::validate` va `ProviderAdmissionProposalV1::validate`
noto'g'ri tuzilgan metama'lumotlarni rad etish:

- Diapazon imkoniyatlari dekodlashi va oraliq/granularlik chegaralarini qondirishi kerak.
- Oqim byudjetlari/transport maslahatlari mos kelishini talab qiladi
  `CapabilityType::ChunkRangeFetch` TLV va bo'sh bo'lmagan maslahatlar ro'yxati.
- Ikki nusxadagi transport protokollari va yaroqsiz ustuvorliklar validatsiyani oshiradi
  reklamalarni g'iybat qilishdan oldingi xatolar.
- Qabul konvertlari diapazon metama'lumotlari bo'yicha taklif/reklamalarni solishtiradi
  `compare_core_fields` shuning uchun mos kelmaydigan g'iybat yuklari erta rad etiladi.

Regressiya qamrovi yashaydi
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Asboblar va jihozlar

- Provayder reklama yuklamalari orasida `range_capability`, `stream_budget` va
  `transport_hints` metama'lumotlari. `/v1/sorafs/providers` javoblari orqali tasdiqlang va
  qabul qilish moslamalari; JSON xulosalari tahlil qilingan qobiliyatni o'z ichiga olishi kerak,
  oqim byudjeti va telemetriyani qabul qilish uchun maslahat massivlari.
- `cargo xtask sorafs-admission-fixtures` oqim byudjetlari va transportni ko'rsatadi
  JSON artefaktlari ichidagi maslahatlar, shuning uchun asboblar paneli funksiyalarning qabul qilinishini kuzatib boradi.
- `fixtures/sorafs_manifest/provider_admission/` ostidagi armaturalarga endi quyidagilar kiradi:
  - kanonik ko'p manbali reklamalar,
  - `multi_fetch_plan.json`, shuning uchun SDK to'plamlari deterministik multi-peerni takrorlashi mumkin
    olish rejasi.

## Orchestrator va Torii integratsiyasi

- Torii `/v1/sorafs/providers` tahlil qilingan diapazon qobiliyati metama'lumotlarini qaytaradi
  `stream_budget` va `transport_hints` bilan. Past darajali ogohlantirishlar qachon yonadi
  provayderlar yangi metama'lumotlarni o'tkazib yuboradi va shlyuz diapazonining so'nggi nuqtalari xuddi shunday qiladi
  to'g'ridan-to'g'ri mijozlar uchun cheklovlar.
- Ko'p manbali orkestr (`sorafs_car::multi_fetch`) endi diapazonni kuchaytiradi
  ishni tayinlashda chegaralar, imkoniyatlarni moslashtirish va oqim byudjetlari. Birlik
  testlar juda katta hajmli, siyrak qidiruv va qisqartiruvchi stsenariylarni qamrab oladi.
- `sorafs_car::multi_fetch` pasaytirish signallarini uzatadi (hizalamadagi nosozliklar,
  to'xtatilgan so'rovlar), shuning uchun operatorlar aniq provayderlar nima uchun ekanligini kuzatishlari mumkin
  rejalashtirish paytida o'tkazib yuborilgan.

## Telemetriya ma'lumotnomasi

Torii diapazonini olish asboblari **SoraFS Fetch kuzatilishi** ni ta'minlaydi.
Grafana asboblar paneli (`dashboards/grafana/sorafs_fetch_observability.json`) va
juftlashtirilgan ogohlantirish qoidalari (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Metrik | Tur | Yorliqlar | Tavsif |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | O'lchagich | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, I18NI0000 |0070X) Provayderlarning reklama diapazoni imkoniyatlari xususiyatlari. |
| `torii_sorafs_range_fetch_throttle_events_total` | Hisoblagich | `reason` (`quota`, `concurrency`, `byte_rate`) | Siyosat bo'yicha guruhlangan bo'g'iq masofani olish urinishlari. |
| `torii_sorafs_range_fetch_concurrency_current` | O'lchagich | — | Birgalikda bir vaqtning o'zida byudjetni sarflaydigan faol himoyalangan oqimlar. |

Misol PromQL snippetlari:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Yoqishdan oldin kvotaning bajarilishini tasdiqlash uchun gaz kelebeği hisoblagichidan foydalaning
ko'p manbali orkestr sozlamalari sukut bo'yicha ishlaydi va parallellik yaqinlashganda ogohlantiradi
flotingiz uchun oqim byudjeti maksimal.