---
id: developer-sdk-index
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

SoraFS asboblar zanjiri bilan yuboriladigan har bir til yordamchilarini kuzatish uchun ushbu markazdan foydalaning.
Rust-ga xos parchalar uchun [Rust SDK snippetlari](./developer-sdk-rust.md) ga oʻting.

## Til yordamchilari

- **Python** — `sorafs_multi_fetch_local` (mahalliy orkestrning tutun sinovlari) va
  `sorafs_gateway_fetch` (shlyuz E2E mashqlari) endi ixtiyoriy qabul qiladi
  `telemetry_region` va `transport_policy` bekor qilish
  (`"soranet-first"`, `"soranet-strict"` yoki `"direct-only"`), CLIni aks ettiradi
  aylantirish tugmalari. Mahalliy QUIC proksi-server ishga tushganda,
  `sorafs_gateway_fetch` ostidagi brauzer manifestini qaytaradi
  `local_proxy_manifest`, shuning uchun testlar ishonchli to'plamni brauzer adapterlariga topshirishi mumkin.
- **JavaScript** — `sorafsMultiFetchLocal` Python yordamchisini aks ettiradi va qaytib keladi
  foydali yuk baytlari va kvitansiya xulosalari, `sorafsGatewayFetch` mashqlari esa
  Torii shlyuzlari, mahalliy proksi-serverni ko'rsatadi va bir xil narsalarni ochib beradi
  telemetriya/transport CLI sifatida bekor qiladi.
- **Rust** — xizmatlar rejalashtiruvchini bevosita orqali joylashtirishi mumkin
  `sorafs_car::multi_fetch`; [Rust SDK parchalari](./developer-sdk-rust.md)
  proof-stream yordamchilari va orkestr integratsiyasi uchun ma'lumotnoma.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP-dan qayta foydalanadi
  ijrochi va faxriy `GatewayFetchOptions`. U bilan birlashtiring
  `ClientConfig.Builder#setSorafsGatewayUri` va PQ yuklash bo'yicha maslahat
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) qachon yuklamalar yopishishi kerak
  Faqat PQ yo'llari.

## Hisoblar paneli va siyosat tugmalari

Python (`sorafs_multi_fetch_local`) va JavaScript ham
(`sorafsMultiFetchLocal`) yordamchilari telemetriyadan xabardor rejalashtiruvchi jadvalni ochib beradi
CLI tomonidan qo'llaniladi:

- Ikkilik ishlab chiqarish ko'rsatkichlar jadvalini sukut bo'yicha yoqadi; `use_scoreboard=True` o'rnating
  (yoki `telemetry` yozuvlarini taqdim eting) fikstürlarni qayta o'ynatishda yordamchi natija olishi uchun
  reklama meta-ma'lumotlari va so'nggi telemetriya suratlaridan buyurtma berishning vaznli provayderi.
- Hisoblangan og'irliklarni bo'lak bilan birga qabul qilish uchun `return_scoreboard=True` ni o'rnating
  kvitansiyalar, shuning uchun CI jurnallari diagnostikani yozib olishi mumkin.
- Tengdoshlarni rad etish yoki qo'shish uchun `deny_providers` yoki `boost_providers` massivlaridan foydalaning.
  Rejalashtiruvchi provayderlarni tanlaganda `priority_delta`.
- Agar pasaytirish amalga oshirilmasa, standart `"soranet-first"` holatini saqlang; ta'minlash
  `"direct-only"` faqat muvofiqlik mintaqasi o'rni oldini olish kerak bo'lganda yoki qachon
  SNNet-5a zaxirasini takrorlash va `"soranet-strict"`ni faqat PQ uchun zaxiralash
  boshqaruv roziligi bilan uchuvchilar.
- Gateway yordamchilari shuningdek, `scoreboardOutPath` va `scoreboardNowUnixSecs` ni ko'rsatadi.
  Hisoblangan ko'rsatkichlar jadvalini davom ettirish uchun `scoreboardOutPath` ni o'rnating (CLIni aks ettiradi)
  `--scoreboard-out` bayrog'i) shuning uchun `cargo xtask sorafs-adoption-check` tasdiqlashi mumkin
  SDK artefaktlari va armatura barqaror kerak bo'lganda `scoreboardNowUnixSecs` dan foydalaning
  Qayta tiklanadigan metama'lumotlar uchun `assume_now` qiymati. JavaScript yordamchisida siz
  qo'shimcha ravishda `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` o'rnatishi mumkin;
  yorliq o'tkazib yuborilsa, u `region:<telemetryRegion>` (orqaga tushadi
  `sdk:js`). Python yordamchisi avtomatik ravishda `telemetry_source="sdk:python"` ni chiqaradi
  u har doim skorbordni saqlab qolsa va yashirin metama'lumotlarni o'chirib qo'yadi.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```