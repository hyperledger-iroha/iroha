---
id: orchestrator-ops
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

Ushbu runbook ko'p manbali olib kelish orkestratorini tayyorlash, tarqatish va ishlatish orqali SRElarni ko'rsatib beradi. U ishlab chiquvchi qoʻllanmasini ishlab chiqarishni yoʻlga qoʻyish uchun sozlangan protseduralar, jumladan bosqichma-bosqich yoqish va tengdoshlarning qora roʻyxati bilan toʻldiradi.

> **Shuningdek qarang:** [Multi-Source Rollout Runbook](./multi-source-rollout.md) flot boʻylab tarqatish toʻlqinlari va favqulodda vaziyatlarda provayderni rad etishga qaratilgan. Kundalik orkestr operatsiyalari uchun ushbu hujjatdan foydalanishda boshqaruv / sahnalashtirishni muvofiqlashtirish uchun unga havola qiling.

## 1. Parvoz oldidan nazorat ro'yxati

1. **Provayder maʼlumotlarini yigʻing**
   - So'nggi provayder reklamalari (`ProviderAdvertV1`) va maqsadli flot uchun telemetriya surati.
   - Sinov ostidagi manifestdan olingan foydali yuk rejasi (`plan.json`).
2. **Deterministik reyting jadvalini ko'rsating**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - `artifacts/scoreboard.json` har bir ishlab chiqaruvchi provayderni `eligible` sifatida ko'rsatishini tasdiqlang.
   - JSON xulosasini jadval bilan birga arxivlang; o'zgartirish so'rovini tasdiqlashda auditorlar qayta urinish hisoblagichlariga tayanadilar.
3. **Asboblar bilan quruq ishga tushirish** — `docs/examples/sorafs_ci_sample/` da ommaviy qurilmalarga nisbatan xuddi shu buyruqni ishlab chiqarish yuklamalariga tegmasdan oldin orkestrator binari kutilgan versiyaga mos kelishiga ishonch hosil qiling.

## 2. Bosqichli ishlab chiqarish tartibi

1. **Kanariya bosqichi (≤2 provayder)**
   - Tabloni qayta tiklang va orkestrni kichik to'plamga mahkamlash uchun `--max-peers=2` bilan ishlating.
   - Monitor:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - To'liq manifestni olish uchun qayta urinish stavkalari 1% dan past bo'lganidan keyin davom eting va hech bir provayderda nosozliklar yig'ilmasa.
2. **Rampa bosqichi (50% provayderlar)**
   - `--max-peers` ni oshiring va yangi telemetriya surati bilan qayta ishga tushiring.
   - `--provider-metrics-out` va `--chunk-receipts-out` bilan har bir yugurishni davom eting. Artefaktlarni ≥7 kun davomida saqlang.
3. **To‘liq ishlab chiqarish**
   - `--max-peers` ni olib tashlang (yoki uni to'liq mos keladigan raqamga o'rnating).
   - Mijozlarni joylashtirishda orkestr rejimini yoqing: konfiguratsiyani boshqarish tizimi orqali doimiy jadval va konfiguratsiya JSON-ni tarqating.
   - `sorafs_orchestrator_fetch_duration_ms` p95/p99 ko'rsatish uchun asboblar panelini yangilang va har bir mintaqada gistogrammalarni qaytadan sinab ko'ring.

## 3. Tengdoshlarning qora ro'yxatiga qo'yish va oshirish

Boshqaruv yangilanishlarini kutmasdan nosog'lom provayderlarni tekshirish uchun CLI reyting siyosatini bekor qilishdan foydalaning.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` roʻyxatdagi taxallusni joriy sessiya uchun koʻrib chiqishdan olib tashlaydi.
- `--boost-provider=<alias>=<weight>` provayderning rejalashtiruvchi vaznini oshiradi. Qiymatlar normallashtirilgan skorbord og'irligiga qo'shimcha hisoblanadi va faqat mahalliy yugurish uchun qo'llaniladi.
- Voqea chiptasiga bekor qilishni yozing va JSON natijalarini biriktiring, shunda asosiy muammo bartaraf etilgandan so'ng egalik jamoasi vaziyatni yarashtirishi mumkin.

Doimiy o'zgartirishlar uchun manba telemetriyasini o'zgartiring (jinoyatchi jazolandi deb belgilang) yoki CLI bekor qilishdan oldin yangilangan oqim byudjetlari bilan reklamani yangilang.

## 4. Muvaffaqiyatsizlik triaji

Qachon qabul qilish muvaffaqiyatsiz tugadi:

1. Qayta ishga tushirishdan oldin quyidagi artefaktlarni suratga oling:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. `session.summary.json` ni odam o‘qishi mumkin bo‘lgan xato qatorini tekshiring:
   - `no providers were supplied` → provayder yo'llari va reklamalarini tekshiring.
   - `retry budget exhausted ...` → `--retry-budget` ni oshiring yoki beqaror tengdoshlarni olib tashlang.
   - `no compatible providers available ...` → qoidabuzar provayderning diapazon imkoniyatlari metamaʼlumotlarini tekshirish.
3. Provayder nomini `sorafs_orchestrator_provider_failures_total` bilan o'zaro bog'lang va agar ko'rsatkich keskin oshsa, keyingi chipta yarating.
4. Nosozlikni aniq takrorlash uchun `--scoreboard-json` va olingan telemetriya bilan oflayn rejimda olishni takrorlang.

## 5. Orqaga qaytarish

Orkestrni qayta tiklash uchun:

2. Tablo neytral vaznga qaytishi uchun `--boost-provider` bekor qilishlarini olib tashlang.
3. Parvozda hech qanday qoldiq olib kelmasligini tasdiqlash uchun kamida bir kun orkestr ko'rsatkichlarini o'chirishda davom eting.

Artefaktni intizomli suratga olish va bosqichma-bosqich ishlab chiqarishni ta'minlash ko'p manbali orkestrni turli xil provayderlar parklarida xavfsiz ishlashini ta'minlaydi, shu bilan birga kuzatuvchanlik va audit talablari saqlanib qoladi.