---
id: nexus-default-lane-quickstart
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/quickstart/default_lane.md`ni aks ettiradi. Ikkala nusxani ham saqlang
mahalliylashtirish portalga tushgunga qadar tekislanadi.
:::

# Standart chiziqli tezkor ishga tushirish (NX-5)

> **Yo‘l xaritasi konteksti:** NX-5 — standart umumiy yo‘lak integratsiyasi. Hozir ish vaqti
> `nexus.routing_policy.default_lane` zaxirasini ochib beradi, shuning uchun Torii REST/gRPC
> oxirgi nuqtalar va har bir SDK trafik tegishli bo'lganda `lane_id` ni xavfsiz tashlab qo'yishi mumkin
> kanonik jamoat yo'lida. Ushbu qo'llanma operatorlarni sozlash bo'yicha yuradi
> katalog, `/status` da qayta tiklashni tekshirish va mijozni mashq qilish
> xatti-harakati oxirigacha.

## Old shartlar

- Sora/Nexus `irohad` (`irohad --sora --config ...` bilan ishlaydi).
- `nexus.*` bo'limlarini tahrirlash uchun konfiguratsiya omboriga kirish.
- `iroha_cli` maqsadli klaster bilan gaplashish uchun sozlangan.
- `curl`/`jq` (yoki ekvivalenti) Torii `/status` foydali yukini tekshirish uchun.

## 1. Yo'lak va ma'lumotlar maydoni katalogini tavsiflang

Tarmoqda mavjud bo'lishi kerak bo'lgan qatorlar va ma'lumotlar bo'shliqlarini e'lon qiling. parcha
quyida (`defaults/nexus/config.toml` dan kesilgan) uchta umumiy yo'lni qayd qiladi
plyus mos keladigan ma'lumotlar maydoni taxalluslari:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Har bir `index` noyob va ulashgan bo'lishi kerak. Ma'lumotlar maydoni identifikatorlari 64 bitli qiymatlardir;
Yuqoridagi misollar aniqlik uchun chiziq indekslari bilan bir xil raqamli qiymatlardan foydalanadi.

## 2. Marshrutlashning birlamchi parametrlarini va ixtiyoriy bekor qilishni o'rnating

`nexus.routing_policy` bo'limi zaxira chizig'ini boshqaradi va sizga imkon beradi
maxsus ko'rsatmalar yoki hisob prefikslari uchun marshrutni bekor qilish. Agar qoida bo'lmasa
mos kelsa, rejalashtiruvchi tranzaktsiyani sozlangan `default_lane` ga yo'naltiradi
va `default_dataspace`. Router mantig'i yashaydi
`crates/iroha_core/src/queue/router.rs` va siyosatni shaffof tarzda qo'llaydi
Torii REST/gRPC sirtlari.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Keyinchalik yangi yo'laklarni qo'shsangiz, avval katalogni yangilang, keyin marshrutni kengaytiring
qoidalar. Orqa yo'lak tutib turadigan umumiy yo'lga ishora qilishda davom etishi kerak

## 3. Qo‘llaniladigan siyosat bilan tugunni ishga tushiring

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Tugun ishga tushirish vaqtida olingan marshrutlash siyosatini qayd qiladi. Har qanday tekshirish xatolari
(etishmayotgan indekslar, takrorlangan taxalluslar, noto'g'ri ma'lumotlar maydoni identifikatorlari) oldin paydo bo'ladi.
g'iybat boshlanadi.

## 4. Yo'laklarni boshqarish holatini tasdiqlang

Tugun onlayn bo'lgandan so'ng, standart chiziq ekanligini tekshirish uchun CLI yordamchisidan foydalaning
muhrlangan (manifest yuklangan) va harakatga tayyor. Xulosa ko'rinishi bitta qatorni chop etadi
har bir qator uchun:

```bash
iroha_cli app nexus lane-report --summary
```

Chiqish misoli:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Agar standart chiziq `sealed` ni ko'rsatsa, oldin qatorlarni boshqarish bo'yicha ish kitobiga amal qiling.
tashqi trafikka ruxsat berish. `--fail-on-sealed` bayrog'i CI uchun qulay.

## 5. Torii holatidagi foydali yuklarni tekshiring

`/status` javobi marshrutlash siyosatini ham, har bir chiziqli rejalashtiruvchini ham ochib beradi.
surat. Konfiguratsiya qilingan standart sozlamalarni tasdiqlash va buni tekshirish uchun `curl`/`jq` dan foydalaning.
zaxira yo'lak telemetriyani ishlab chiqaradi:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Chiqish namunasi:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` qatori uchun jonli rejalashtiruvchi hisoblagichlarni tekshirish uchun:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Bu TEU surati, taxallus metamaʼlumotlari va manifest bayroqlari mos kelishini tasdiqlaydi.
konfiguratsiya bilan. Xuddi shu foydali yuk Grafana panellari uchun foydalanadi
lane-ingest boshqaruv paneli.

## 6. Mashq mijozining standart sozlamalari

- **Rust/CLI.** `iroha_cli` va Rust mijoz qutisi `lane_id` maydonini o'tkazib yuboradi
  `--lane-id` / `LaneSelector` o'tmaganingizda. Shuning uchun navbat marshrutizatori
  `default_lane` ga qaytadi. Aniq `--lane-id`/`--dataspace-id` bayroqlaridan foydalaning
  faqat standart bo'lmagan chiziqqa mo'ljallanganda.
- **JS/Swift/Android.** Oxirgi SDK versiyalari `laneId`/`lane_id` ixtiyoriy sifatida qabul qilinadi.
  va `/status` tomonidan e'lon qilingan qiymatga qayting. Marshrutlash siyosatini saqlang
  sahnalashtirish va ishlab chiqarish bo'yicha sinxronlash, shuning uchun mobil ilovalar favqulodda vaziyatga muhtoj emas
  qayta konfiguratsiyalar.
- **Quvurlar/SSE testlari.** Tranzaksiya hodisasi filtrlari qabul qilinadi
  `tx_lane_id == <u32>` predikatlar (qarang: `docs/source/pipeline.md`). Obuna bo'ling
  `/v1/pipeline/events/transactions` ushbu filtr bilan yozilganligini isbotlash uchun yuborilgan
  aniq yo'laksiz orqa chiziq identifikatori ostida keladi.

## 7. Kuzatish va boshqarish ilgaklari

- `/status` shuningdek, `nexus_lane_governance_sealed_total` va
  `nexus_lane_governance_sealed_aliases`, shuning uchun Alertmanager istalgan vaqtda ogohlantirishi mumkin
  Lane o'z manifestini yo'qotadi. Ushbu ogohlantirishlarni hatto devnetlar uchun ham yoqilgan holda saqlang.
- Rejalashtiruvchi telemetriya xaritasi va yo'laklarni boshqarish asboblar paneli
  (`dashboards/grafana/nexus_lanes.json`) dan taxallus/slug maydonlarini kutadi
  katalog. Agar taxallus nomini o'zgartirsangiz, tegishli Kura kataloglarini shunday qayta etiketlang
  auditorlar deterministik yo'llarni saqlaydilar (NX-1 ostida kuzatilgan).
- Parlament tomonidan standart yo'llar uchun ruxsatnomalar orqaga qaytish rejasini o'z ichiga olishi kerak. Yozib olish
  manifest xesh va boshqaruv dalillari bilan bir qatorda sizning
  operator runbook, shuning uchun kelajakdagi aylanishlar kerakli holatni taxmin qilmaydi.

Ushbu tekshiruvlardan o'tgandan so'ng, siz `nexus.routing_policy.default_lane` ni tekshirishingiz mumkin
tarmoqdagi kod yo'llari.