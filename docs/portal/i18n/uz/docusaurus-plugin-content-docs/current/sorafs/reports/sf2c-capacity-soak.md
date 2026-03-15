---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c Imkoniyatlarni yig'ish bo'yicha hisobot

Sana: 2026-03-21

## Qo'llash doirasi

Ushbu hisobotda SoraFS deterministik sig'imning to'planishi va to'lov so'rilishi qayd etilgan.
SF-2c yo'l xaritasi treki ostida so'ralgan testlar.

- **30 kunlik ko'p provayder soak:** Mashq qilgan
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Jabduqlar beshta provayderni amalga oshiradi, 30 ta hisob-kitob oynalarini o'z ichiga oladi va
  buxgalteriya hisobi yig'indilari mustaqil hisoblangan ma'lumotnomaga mos kelishini tasdiqlaydi
  proyeksiya. Sinov Blake3 dayjestini (`capacity_soak_digest=...`) chiqaradi, shuning uchun
  CI kanonik suratni olishi va farq qilishi mumkin.
- **To'liq yetkazib berish uchun jarimalar:** tomonidan amalga oshirilgan
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (xuddi shu fayl). Sinov ish tashlash chegaralarini, sovutish vaqtini, garov chegaralarini tasdiqlaydi,
  va daftar hisoblagichlari deterministik bo'lib qolmoqda.

## Amalga oshirish

Lokal tarzda singdirish tekshiruvlarini bajaring:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Sinovlar standart noutbukda bir soniyadan kamroq vaqt ichida yakunlanadi va yo'q
tashqi moslamalar.

## Kuzatish imkoniyati

Torii endi provayderning kredit lavhalarini to'lovlar daftarlari bilan birga ko'rsatadi, shuning uchun asboblar paneli
past balanslar va penaltilar bo'lishi mumkin:

- REST: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` yozuvlarini qaytaradi
  ho'llash testida tasdiqlangan daftar maydonlarini aks ettiring. Qarang
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana import: `dashboards/grafana/sorafs_capacity_penalties.json` chizmalar
  eksport qilingan ish tashlash hisoblagichlari, jarima summalari va qo'ng'iroq bo'yicha bog'langan garov
  xodimlar jonli muhit bilan ho'llash bazasini solishtirish mumkin.

## Kuzatuv

- Suvga cho'milish testini (tutun darajasi) takrorlash uchun CIda haftalik shlyuzlarni rejalashtirish.
- Ishlab chiqarish telemetriyasidan so'ng Grafana taxtasini Torii qirqish nishonlari bilan kengaytiring
  eksport faollashadi.