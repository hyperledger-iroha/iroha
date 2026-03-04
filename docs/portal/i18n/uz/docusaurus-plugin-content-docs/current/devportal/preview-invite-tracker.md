---
id: preview-invite-tracker
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite tracker
sidebar_label: Preview tracker
description: Wave-by-wave status log for the checksum-gated docs portal preview program.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Ushbu kuzatuvchi har bir hujjatlar portalini oldindan ko'rish to'lqinini yozib oladi, shuning uchun DOCS-SORA egalari va
boshqaruv tekshiruvchilari qaysi kogorta faol ekanligini, takliflarni kim ma'qullaganini ko'rishlari mumkin,
va qaysi artefaktlar hali ham e'tiborga muhtoj. Takliflar yuborilganda uni yangilang,
bekor qilingan yoki kechiktirilgan, shuning uchun audit izi ombor ichida qoladi.

## To'lqin holati

| To'lqin | Kohort | Kuzatuvchi muammosi | Tasdiqlovchi(lar) | Holati | Maqsadli oyna | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 – Asosiy saqlovchilar** | Docs + SDK saqlovchilari nazorat summasi oqimini tasdiqlaydi | `DOCS-SORA-Preview-W0` (GitHub/ops kuzatuvchisi) | Docs/DevRel yetakchisi + Portal TL | 🈴 Tugallandi | 2025-yilning 2-chorak 1–2-haftalari | Takliflar 2025‑03‑25 da yuborilgan, telemetriya yashil bo‘lib qoldi, chiqish xulosasi 2025‑04‑08 da chop etilgan. |
| **W1 – Hamkorlar** | SoraFS operatorlari, NDA ostidagi Torii integratorlari | `DOCS-SORA-Preview-W1` | Docs/DevRel yetakchisi + Boshqaruv bilan aloqa | 🈴 Tugallandi | 2025-yilning 2-chorak 3-hafta | Takliflar 2025‑04‑12 → 2025‑04‑26 bo‘lib, barcha sakkiz hamkor qabul qilindi; [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) va chiqish dayjesti [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) da olingan dalillar. |
| **W2 – Hamjamiyat** | Tanlangan hamjamiyat kutish roʻyxati (bir vaqtning oʻzida ≤25) | `DOCS-SORA-Preview-W2` | Docs/DevRel yetakchisi + Hamjamiyat menejeri | 🈴 Tugallandi | 2025-yilning 3-chorak 1-hafta (taxminiy) | Takliflar 2025‑06‑15 → 2025‑06‑29-da telemetriya yashil rangda; dalillar + [`preview-feedback/w2/summary.md`] (./preview-feedback/w2/summary.md) da olingan topilmalar. |
| **W3 – Beta kogortalari** | Moliya/kuzatish beta + SDK hamkori + ekotizim himoyachisi | `DOCS-SORA-Preview-W3` | Docs/DevRel yetakchisi + Boshqaruv bilan aloqa | 🈴 Tugallandi | 2026-yilning 1-chorak 8-hafta | Takliflar 2026‑02‑18 → 2026‑02‑28; digest + `preview-20260218` to'lqini orqali yaratilgan portal ma'lumotlari (qarang [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Eslatma: har bir kuzatuvchi muammosini tegishli oldindan ko'rish so'rovi chiptalariga bog'lang va
> tasdiqlashlar saqlanib qolishi uchun ularni `docs-portal-preview` loyihasi ostida arxivlang
> kashf qilish mumkin.

## Faol vazifalar (W0)

- ✅ Parvoz oldidan artefaktlar yangilandi (GitHub Actions `docs-portal-preview` 2025‑03‑24 ishlaydi, deskriptor `preview-2025-03-24` tegi yordamida `scripts/preview_verify.sh` orqali tasdiqlangan).
- ✅ Telemetriyaning asosiy koʻrsatkichlari olindi (`docs.preview.integrity`, `TryItProxyErrors` asboblar panelining surati W0 kuzatuvchisi muammosiga saqlangan).
- ✅ Tashkilot nusxasi [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) yordamida `preview-2025-03-24` oldindan koʻrish yorligʻi bilan qulflangan.
- ✅ Qabul qilish so'rovlari birinchi beshta xizmat ko'rsatuvchi uchun qayd etilgan (chiptalar `DOCS-SORA-Preview-REQ-01` … `-05`).
- ✅ Birinchi beshta taklif 2025‑03‑25 10:00–10:20 UTC ketma-ket etti yashil telemetriya kunidan so‘ng yuborildi; `DOCS-SORA-Preview-W0` da saqlangan tasdiqlar.
- ✅ Telemetriyani kuzatib boring + mezbon ish soatlari (2025‑03‑31 gacha har kuni ro'yxatdan o'tish; quyida nazorat punkti jurnali).
- ✅ Oʻrta nuqtadagi fikr-mulohaza/muammolarni toʻplang va ularni `docs-preview/w0` deb belgilang (qarang [W0 dayjest](./preview-feedback/w0/summary.md)).
- ✅ To'lqinlar sarhisobini nashr qilish + chiqish taklifini tasdiqlash (chiqish to'plami 2025‑04‑08; qarang [W0 dayjest](./preview-feedback/w0/summary.md)).
- ✅ W3 beta to'lqini kuzatildi; boshqaruvni ko'rib chiqishdan keyin kerak bo'lganda rejalashtirilgan kelajakdagi to'lqinlar.

## W1 hamkori toʻlqini xulosasi

- ✅ **Huquqiy va boshqaruvni tasdiqlash.** Hamkor qo'shimchasi imzolangan 2025‑04‑05; tasdiqlashlar `DOCS-SORA-Preview-W1` ga yuklangan.
- ✅ **Telemetriya + Sahnalashtirishni sinab koʻring.** `OPS-TRYIT-147` chiptasini I18NI000000000X, `docs.preview.integrity`, I18NI00000000000 va I18NI0000000000000000 uchun Grafana suratlari bilan 2025‑04‑06 bajarilgan oʻzgartiring. arxivlangan.
- ✅ **Artefact + checksum prep.** `preview-2025-04-12` to'plami tasdiqlangan; `artifacts/docs_preview/W1/preview-2025-04-12/` ostida saqlangan deskriptor/checksum/prob jurnallari.
- ✅ **Taklif roʻyxati + joʻnatish.** Barcha sakkizta hamkor soʻrovlari (`DOCS-SORA-Preview-REQ-P01…P08`) maʼqullandi; takliflar 2025‑04‑12 15:00–15:21 UTC kunlari yuborilgan, har bir ko‘rib chiquvchining tasdig‘i qayd etilgan.
- ✅ **Tekshiruv asboblari.** Kundalik ish vaqti + telemetriya nazorat punktlari qayd etilgan; digest uchun [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) ga qarang.
- ✅ **Yakuniy roʻyxat/chiqish jurnali.** [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) endi 2025-04-26 holatiga koʻra taklif/qabul qilish vaqt belgilari, telemetriya dalillari, viktorina eksporti va artefakt koʻrsatkichlarini yozib oladi, shuning uchun boshqaruv toʻlqinni qayta koʻrsatishi mumkin.

## Takliflar jurnali — W0 asosiy saqlovchilari

| Sharhlovchi ID | Rol | Chipta so'rash | Taklif yuborildi (UTC) | Kutilayotgan chiqish (UTC) | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal boshqaruvchisi | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Faol | Qabul qilingan nazorat summasini tekshirish; Nav/yon panelni ko'rib chiqishga e'tibor qaratish. |
| sdk-rust-01 | Rust SDK yetakchisi | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Faol | SDK retseptlarini sinash + Norito tezkor ishga tushirish. |
| sdk-js-01 | JS SDK boshqaruvchisi | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Faol | Tekshirilmoqda Konsol + ISO oqimlarini sinab ko'ring. |
| sorafs-ops-01 | SoraFS operator aloqasi | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Faol | Auditing SoraFS runbooks + orkestratsiya hujjatlari. |
| kuzatuvchanlik-01 | Kuzatish qobiliyati TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Faol | Telemetriya/indent qo'shimchalarini ko'rib chiqish; Alertmanager qamroviga egalik qiladi. |

Barcha takliflar bir xil `docs-portal-preview` artefaktiga ishora qiladi (2025-03-24-da ishga tushirish,
yorlig'i `preview-2025-03-24`) va olingan tasdiqlash transkripti
`DOCS-SORA-Preview-W0`. Har qanday qo'shimchalar/pauzalar ikkala jadvalda ham ro'yxatga olinishi kerak
yuqoridagi va keyingi to'lqinga o'tishdan oldin kuzatuvchi muammosi.

## Tekshirish nuqtasi jurnali - W0

| Sana (UTC) | Faoliyat | Eslatmalar |
| --- | --- | --- |
| 2025-03-26 | Telemetriyani ko'rib chiqish + ish vaqti | `docs.preview.integrity` + `TryItProxyErrors` yashil rangda qoldi; ish soatlari barcha tekshiruvchilar nazorat summasini tekshirishni yakunlaganligini tasdiqladi. |
| 2025-03-27 | O'rta nuqtadagi fikr-mulohazalar dayjesti e'lon qilindi | Xulosa [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) da olingan; ikkita kichik navbatchilik muammosi `docs-preview/w0` yorlig'i sifatida qayd etilgan, hech qanday hodisa qayd etilmagan. |
| 2025-03-31 | Yakuniy hafta telemetriya nuqta tekshiruvi | Chiqishdan oldingi oxirgi ish soatlari; ko'rib chiquvchilar hujjatlarning qolgan vazifalarini yo'lda ekanligini tasdiqladilar, hech qanday ogohlantirish berilmadi. |
| 2025-04-08 | Chiqish xulosasi + yopilishlarni taklif qilish | Tugallangan sharhlar, vaqtinchalik ruxsat bekor qilingan, arxivlangan topilmalar [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); treker W1 ni tayyorlashdan oldin yangilanadi. |

## Takliflar jurnali — W1 hamkorlari

| Sharhlovchi ID | Rol | Chipta so'rash | Taklif yuborildi (UTC) | Kutilayotgan chiqish (UTC) | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operatori (EI) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Tugallandi | 2025‑04‑20. Orkestrator operatsiyasi haqida fikr-mulohaza yuborildi; chiqish 15:05 UTC. |
| sorafs-op-02 | SoraFS operatori (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Tugallandi | `docs-preview/w1` da ro'yxatdan o'tgan yo'riqnoma sharhlari; chiqish 15:10 UTC. |
| sorafs-op-03 | SoraFS operatori (AQSh) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Tugallandi | Eʼtiroz/qora roʻyxat tahrirlari topshirildi; chiqish 15:12UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Tugallandi | Sinab ko'ring, autentifikatsiya usuli qabul qilindi; chiqish 15:14UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Tugallandi | RPC/OAuth doc sharhlari qayd etilgan; chiqish 15:16UTC. |
| sdk-partner-01 | SDK hamkori (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Tugallandi | Oldindan ko'rish yaxlitligi haqidagi fikr-mulohaza birlashtirildi; chiqish 15:18UTC. |
| sdk-partner-02 | SDK hamkori (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Tugallandi | Telemetriya/redaktsiya ko'rib chiqildi; chiqish 15:22UTC. |
| Gateway-ops-01 | Gateway operatori | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Tugallandi | Gateway DNS runbook sharhlari topshirildi; chiqish 15:24UTC. |

## Tekshirish nuqtasi jurnali - W1

| Sana (UTC) | Faoliyat | Eslatmalar |
| --- | --- | --- |
| 2025-04-12 | Taklif jo'natish + artefaktni tekshirish | Barcha sakkiz hamkor `preview-2025-04-12` deskriptor/arxiv bilan elektron pochta orqali jo'natilgan; trekerda saqlangan tasdiqlar. |
| 2025-04-13 | Telemetriya asosini ko'rib chiqish | `docs.preview.integrity`, `TryItProxyErrors` va `DocsPortal/GatewayRefusals` asboblar panellari ko'rib chiqildi - kengash bo'ylab yashil; ish vaqti tasdiqlandi nazorat summasini tekshirish tugallandi. |
| 2025-04-18 | O'rta to'lqinli ish soatlari | `docs.preview.integrity` yashil rangda qoldi; `docs-preview/w1` ostida tizimga kirgan ikkita doc nits (nav so'zlari + Skrinshotni sinab ko'ring). |
| 2025-04-22 | Yakuniy telemetriya nuqta tekshiruvi | Proksi + boshqaruv paneli hali ham sog'lom; hech qanday yangi muammolar ko'tarilmadi, chiqish oldidan trekerda qayd etilgan. |
| 2025-04-26 | Chiqish xulosasi + yopilishlarni taklif qilish | Barcha hamkorlar ko‘rib chiqish tugallanganligini tasdiqladi, takliflar bekor qilindi, dalillar [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) da arxivlandi. |

## W3 beta kohort xulosasi

- ✅ 2026‑02‑18 sanasida yuborilgan takliflar nazorat summasini tekshirish va shu kunning o‘zida qayd etilgan tasdiqlar bilan.
- ✅ `DOCS-SORA-Preview-20260218` boshqaruv muammosi bilan `docs-preview/20260218` ostida to'plangan fikr-mulohazalar; digest + `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` orqali yaratilgan xulosa.
- ✅ Yakuniy telemetriya tekshiruvidan so'ng 2026‑02‑28 da kirish bekor qilindi; treker + portal jadvallari W3 ni tugallanganligini ko'rsatish uchun yangilandi.

## Takliflar jurnali — W2 hamjamiyati| Sharhlovchi ID | Rol | Chipta so'rash | Taklif yuborildi (UTC) | Kutilayotgan chiqish (UTC) | Holati | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Hamjamiyat sharhlovchisi (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Tugallandi | Qabul qilish 16:06UTC; SDK tezkor ishga tushirishga e'tibor qaratish; chiqish 2025-06-29 tasdiqlangan. |
| comm-vol-02 | Jamiyat sharhlovchisi (boshqaruv) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Tugallandi | Boshqaruv/SNS tekshiruvi amalga oshirildi; chiqish 2025-06-29 tasdiqlangan. |
| comm-tom-03 | Hamjamiyat sharhlovchisi (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Tugallandi | Norito tahliliy fikr-mulohaza jurnali; chiqish ack 2025-06-29. |
| comm-tom-04 | Hamjamiyat sharhlovchisi (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Tugallandi | SoraFS runbook tekshiruvi tugallandi; chiqish ack 2025-06-29. |
| comm-vol-05 | Hamjamiyat sharhlovchisi (Mavjudlik) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Tugallandi | Foydalanish imkoniyati/UX qaydlari ulashildi; chiqish ack 2025-06-29. |
| comm-tom-06 | Hamjamiyat sharhlovchisi (Mahalliylashtirish) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Tugallandi | Mahalliylashtirish bo'yicha fikr-mulohazalar qayd etildi; chiqish ack 2025-06-29. |
| comm-vol-07 | Hamjamiyat sharhlovchisi (Mobil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Tugallandi | Mobil SDK hujjat tekshiruvlari yetkazib berildi; chiqish ack 2025-06-29. |
| comm-vol-08 | Hamjamiyat sharhlovchisi (Kuzatilishi mumkin) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Tugallandi | Kuzatish mumkin bo'lgan ilovani ko'rib chiqish amalga oshirildi; chiqish ack 2025-06-29. |

## Tekshirish nuqtasi jurnali - W2

| Sana (UTC) | Faoliyat | Eslatmalar |
| --- | --- | --- |
| 2025-06-15 | Taklif jo'natish + artefaktni tekshirish | `preview-2025-06-15` deskriptor/arxivi 8 ta hamjamiyat sharhlovchilari bilan bo'lingan; trekerda saqlangan tasdiqlar. |
| 2025-06-16 | Telemetriya asosini ko'rib chiqish | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` asboblar paneli yashil; Sinab ko‘ring. Proksi jurnallari faol hamjamiyat tokenlarini ko‘rsatadi. |
| 2025-06-18 | Ish vaqti va masalalarni aniqlash | Ikkita taklif toʻplandi (`docs-preview/w2 #1` asboblar maslahati matni, `#2` mahalliylashtirish yon paneli) — ikkalasi ham Hujjatlarga yoʻnaltirildi. |
| 2025-06-21 | Telemetriya tekshiruvi + hujjat tuzatishlari | `docs-preview/w2 #1/#2` manzilli hujjatlar; asboblar paneli hali ham yashil, hech qanday hodisa. |
| 2025-06-24 | Yakuniy hafta ish vaqti | Taqrizchilar qolgan fikr-mulohazalarni tasdiqladilar; ogohlantirish yong'in yo'q. |
| 2025-06-29 | Chiqish xulosasi + yopilishlarni taklif qilish | Yozib olingan ma'lumotlar, oldindan ko'rish ruxsati bekor qilindi, telemetriya suratlari + arxivlangan artefaktlar (qarang: [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Ish vaqti va masalalarni aniqlash | `docs-preview/w1` ostida qayd etilgan ikkita hujjat taklifi; hech qanday hodisalar yoki ogohlantirishlar tetiklanmagan. |

## Hisobot ilgaklari

- Har chorshanba kuni yuqoridagi kuzatuvchi jadvalini va faol taklif masalasini yangilang
  qisqa status qaydi bilan (takliflar yuborilgan, faol sharhlovchilar, hodisalar).
- To'lqin yopilganda, fikr-mulohazalarni sarhisob qilish yo'lini qo'shing (masalan,
  `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) va uni bog'lang
  `status.md`.
- Agar [taklif oqimini oldindan ko'rish] (./preview-invite-flow.md) dan pauza mezonlari bo'lsa
  ishga tushiring, takliflarni davom ettirishdan oldin bu yerga tuzatish bosqichlarini qo'shing.