---
id: testnet-rollout
lang: uz
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet Rollout (SNNet-10)
description: Phased activation plan, onboarding kit, and telemetry gates for SoraNet testnet promotions.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

SNNet-10 tarmoq bo'ylab SoraNet anonimlik qoplamasini bosqichma-bosqich faollashtirishni muvofiqlashtiradi. SoraNet standart transportga aylanishidan oldin har bir operator kutilgan narsalarni tushunib olishi uchun yo‘l xaritasi o‘qini aniq natijalarga, ishga tushirish kitoblariga va telemetriya darvozalariga tarjima qilish uchun ushbu rejadan foydalaning.

## Ishga tushirish bosqichlari

| Bosqich | Xronologiya (maqsad) | Qo'llash doirasi | Kerakli artefaktlar |
|-------|-------------------|-------|--------------------|
| **T0 — Yopiq Testnet** | 2026 yil 4-chorak | Asosiy ishtirokchilar tomonidan boshqariladigan ≥3 ASN bo'ylab 20–50 o'rni. | Testnet ishga tushirish to'plami, qo'riqchi tutashuvchi tutun to'plami, asosiy kechikish + PoW ko'rsatkichlari, burg'ulash jurnali. |
| **T1 — Ommaviy Beta** | 2027 yil 1-chorak | ≥100 oʻrni, qoʻriqchi aylanishi yoqilgan, chiqish ulanishi kuchga kirgan, `anon-guard-pq` bilan SoraNet uchun standart SDK beta. | Yangilangan bort to'plami, operatorni tekshirish ro'yxati, ma'lumotnomalarni nashr qilish SOP, telemetriya asboblar paneli to'plami, voqea-hodisalar bo'yicha takroriy hisobotlar. |
| **T2 — Asosiy tarmoq standarti** | 2027 yil 2-chorak (SNNet-6/7/9 tugallanishiga e'tibor qaratildi) | Ishlab chiqarish tarmog'ining sukut bo'yicha SoraNet; obfs/MASQUE transportlari va PQ ratchet ijrosi yoqilgan. | Boshqaruvni tasdiqlash daqiqalari, faqat to'g'ridan-to'g'ri orqaga qaytarish tartibi, pasaytirish signallari, imzolangan muvaffaqiyat ko'rsatkichlari hisoboti. |

**Oʻtkazib yuborish yoʻli yoʻq** — har bir bosqich telemetriya va boshqaruv artefaktlarini ilgari surishdan oldin oldingi bosqichdan joʻnatishi kerak.

## Testnet ishga tushirish to'plami

Har bir relay operatori quyidagi fayllar bilan deterministik paketni oladi:

| Artefakt | Tavsif |
|----------|-------------|
| `01-readme.md` | Umumiy ko'rinish, aloqa nuqtalari va vaqt jadvali. |
| `02-checklist.md` | Parvoz oldidan nazorat ro'yxati (apparat, tarmoqqa kirish imkoniyati, qo'riqlash siyosatini tekshirish). |
| `03-config-example.toml` | Minimal SoraNet relay + orchestrator konfiguratsiyasi SNNet-9 muvofiqlik bloklari bilan moslangan, shu jumladan `guard_directory` bloki, so'nggi guard snapshot xesh. |
| `04-telemetry.md` | SoraNet maxfiylik o'lchovlari asboblar paneli va ogohlantirish chegaralarini ulash bo'yicha ko'rsatmalar. |
| `05-incident-playbook.md` | Eskalatsiya matritsasi bilan qo'ng'iroq qilish/pastga tushirish javob protsedurasi. |
| `06-verification-report.md` | Shablon operatorlari tutun sinovlari o'tgandan so'ng yakunlaydi va qaytib keladi. |

Ko'rsatilgan nusxa `docs/examples/soranet_testnet_operator_kit/` da saqlanadi. Har bir aksiya to'plamni yangilaydi; versiya raqamlari fazani kuzatib boradi (masalan, `testnet-kit-vT0.1`).

Ommaviy beta (T1) operatorlari uchun `docs/source/soranet/snnet10_beta_onboarding.md` dagi ixcham ishga tushirish boʻyicha qisqacha maʼlumot deterministik toʻplam va validator yordamchilariga ishora qilib, dastlabki shartlar, telemetriya natijalari va joʻnatish ish jarayonini umumlashtiradi.

`cargo xtask soranet-testnet-feed` reklama oynasi, oʻtish roʻyxati, koʻrsatkichlar hisoboti, burgʻulash dalillari va sahna darvozasi shabloniga havola qilingan biriktirma xeshlarini jamlaydigan JSON tasmasi ishlab chiqaradi. Tasma `drill_log.signed = true` yozib olishi uchun avval `cargo xtask soranet-testnet-drill-bundle` bilan matkap jurnallari va biriktirmalarni imzolang.

## Muvaffaqiyat ko'rsatkichlari

Fazalar orasidagi targ'ibot kamida ikki hafta davomida to'plangan quyidagi telemetriya bo'yicha amalga oshiriladi:

- `soranet_privacy_circuit_events_total`: 95% sxemalar ishdan chiqish yoki pasaytirish hodisalarisiz tugallandi; qolgan 5% PQ ta'minoti bilan cheklangan.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: Kuniga olib tashlash seanslarining <1% rejalashtirilgan mashqlardan tashqarida ishlamay qolishiga olib keladi.
- `soranet_privacy_gar_reports_total`: kutilayotgan GAR toifasi aralashmasining ±10% ichida dispersiya; o'sishlar tasdiqlangan siyosat yangilanishlari bilan tushuntirilishi kerak.
- PoW chiptasining muvaffaqiyat darajasi: 3s maqsadli oyna ichida ≥99%; `soranet_privacy_throttles_total{scope="congestion"}` orqali xabar qilingan.
- Har bir mintaqa uchun kechikish (95-persentil): sxemalar toʻliq qurilganidan keyin <200ms, `soranet_privacy_rtt_millis{percentile="p95"}` orqali yozib olinadi.

Boshqaruv paneli va ogohlantirish shablonlari `dashboard_templates/` va `alert_templates/` da ishlaydi; ularni telemetriya omboringizga aks ettiring va ularni CI lint tekshiruvlariga qo'shing. Ko‘tarilish so‘rovidan oldin boshqaruv hisobotini yaratish uchun `cargo xtask soranet-testnet-metrics` dan foydalaning.

Bosqichli jo‘natmalar `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` ostida saqlangan nusxa ko‘chirishga tayyor Markdown formasiga havola qiluvchi `docs/source/soranet/snnet10_stage_gate_template.md` ga amal qilishi kerak.

## Tekshirish roʻyxati

Operatorlar har bir bosqichga kirishdan oldin quyidagilardan chiqishlari kerak:

- ✅ Joriy qabul konverti bilan imzolangan e'lonni o'tkazish.
- ✅ Guard aylanish tutuni sinovi (`tools/soranet-relay --check-rotation`) o'tadi.
- ✅ `guard_directory` oxirgi `GuardDirectorySnapshotV2` artefaktini koʻrsatadi va `expected_directory_hash_hex` qoʻmita dayjestiga mos keladi (releyni ishga tushirish tasdiqlangan xeshni qayd qiladi).
- ✅ PQ ratchet ko'rsatkichlari (`sorafs_orchestrator_pq_ratio`) so'ralgan bosqich uchun maqsadli chegaralardan yuqori bo'lib qoladi.
- ✅ GAR muvofiqligi konfiguratsiyasi so'nggi tegga mos keladi (SNNet-9 katalogiga qarang).
- ✅ Signal simulyatsiyasini pasaytirish (kollektorlarni o'chiring, 5 daqiqa ichida ogohlantirishni kuting).
- ✅ PoW/DoS matkapi hujjatlashtirilgan yumshatish bosqichlari bilan bajarilgan.

Oldindan to'ldirilgan shablon bortga kirish to'plamiga kiritilgan. Operatorlar ishlab chiqarish ma'lumotlarini olishdan oldin to'ldirilgan hisobotni boshqaruv yordam xizmatiga taqdim etadilar.

## Boshqaruv va hisobot

- **O'zgarishlarni nazorat qilish:** lavozimga ko'tarilish uchun Boshqaruv kengashining ma'qullanishi kengash bayonnomasida qayd etilgan va holat sahifasiga ilova qilingan.
- **Status dayjesti:** o‘rni soni, PQ nisbati, ishdan chiqish hodisalari va ajoyib harakat elementlarini jamlagan haftalik yangilanishlarni nashr eting (kadans boshlanganidan keyin `docs/source/status/soranet_testnet_digest.md` da saqlanadi).
- **Orqaga qaytarish:** 30 daqiqa ichida tarmoqni oldingi bosqichga qaytaradigan imzolangan orqaga qaytarish rejasini saqlang, jumladan DNS/qo'riqchi keshini bekor qilish va mijoz bilan aloqa shablonlari.

## Yordamchi aktivlar

- `cargo xtask soranet-testnet-kit [--out <dir>]` ishga tushirish to'plamini `xtask/templates/soranet_testnet/` dan maqsadli katalogga o'zgartiradi (birlamchi `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` SNNet-10 muvaffaqiyat ko'rsatkichlarini baholaydi va boshqaruvni tekshirish uchun mos tuzilgan o'tish/qobiliyatsiz hisobotini chiqaradi. Namuna surati `docs/examples/soranet_testnet_metrics_sample.json` da saqlanadi.
- Grafana va Alertmanager shablonlari `dashboard_templates/soranet_testnet_overview.json` va `alert_templates/soranet_testnet_rules.yml` ostida ishlaydi; ularni telemetriya omboringizga ko'chiring yoki CI lint tekshiruvlariga o'tkazing.
- SDK/portal xabar almashish uchun pasaytirilgan aloqa shablonlari `docs/source/soranet/templates/downgrade_communication_template.md` da joylashgan.
- Haftalik status dayjestlari kanonik shakl sifatida `docs/source/status/soranet_testnet_weekly_digest.md` dan foydalanishi kerak.

Chiqish soʻrovlari ushbu sahifani har qanday artefakt yoki telemetriya oʻzgarishlari bilan birga yangilashi kerak, shuning uchun tarqatish rejasi kanonik boʻlib qoladi.