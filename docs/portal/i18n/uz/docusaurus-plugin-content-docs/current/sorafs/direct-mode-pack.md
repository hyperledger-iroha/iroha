---
id: direct-mode-pack
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

SoraNet sxemalari SoraFS uchun standart transport bo‘lib qoladi, biroq yo‘l xaritasi **SNNet-5a** bandi tartibga solinadigan orqaga qaytishni talab qiladi, shuning uchun operatorlar anonimlik tugashi bilan deterministik o‘qishga kirishni saqlab qolishi mumkin. Ushbu toʻplam SoraFS ni toʻgʻridan-toʻgʻri Torii/QUIC rejimida maxfiylik transportlariga tegmasdan ishga tushirish uchun zarur boʻlgan CLI/SDK tugmalari, konfiguratsiya profillari, muvofiqlik testlari va joylashtirish nazorat roʻyxatini oladi.

Qayta tiklash SNNet-5 va SNNet-9 o'zlarining tayyorlik eshiklarini tozalaguncha bosqichma-bosqich va tartibga solinadigan ishlab chiqarish muhitlariga taalluqlidir. Quyidagi artefaktlarni odatiy SoraFS joylashtirish garovi bilan birga saqlang, shunda operatorlar so'rov bo'yicha anonim va to'g'ridan-to'g'ri rejimlarni almashtirishlari mumkin.

## 1. CLI va SDK bayroqlari

- `sorafs_cli fetch --transport-policy=direct-only …` o'rni rejalashtirishni o'chirib qo'yadi va Torii/QUIC transportlarini amalga oshiradi. CLI yordami endi qabul qilingan qiymat sifatida `direct-only` ro'yxatini beradi.
- SDK'lar "to'g'ridan-to'g'ri rejim" o'tish tugmasi paydo bo'lganda `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` ni o'rnatishi kerak. `iroha::ClientOptions` va `iroha_android` da yaratilgan ulanishlar bir xil raqamni oldinga siljitadi.
- Gateway jabduqlari (`sorafs_fetch`, Python ulanishlari) umumiy Norito JSON yordamchilari orqali faqat to'g'ridan-to'g'ri almashtirishni tahlil qilishi mumkin, shuning uchun avtomatlashtirish bir xil harakatni oladi.

Bayroqni hamkorlarga mo'ljallangan runbook va sim xususiyatini muhit o'zgaruvchilari emas, balki `iroha_config` orqali almashtirishni hujjatlashtiring.

## 2. Gateway siyosati profillari

Deterministik orkestr konfiguratsiyasini davom ettirish uchun Norito JSON dan foydalaning. `docs/examples/sorafs_direct_mode_policy.json` dagi misol profili kodlaydi:

- `transport_policy: "direct_only"` — faqat SoraNet relay transportlarini reklama qiluvchi provayderlarni rad etish.
- `max_providers: 2` - eng ishonchli Torii/QUIC so'nggi nuqtalariga to'g'ridan-to'g'ri tengdoshlar. Mintaqaviy muvofiqlik imtiyozlari asosida sozlang.
- `telemetry_region: "regulated-eu"` - yorliqli ko'rsatkichlar, shuning uchun telemetriya asboblar paneli va auditlar qayta ishlashni ajratib turadi.
- Noto'g'ri sozlangan shlyuzlarni niqoblashdan qochish uchun konservativ qayta urinish byudjetlari (`retry_budget: 2`, `provider_failure_threshold: 3`).

Operatorlarga siyosatni ochishdan oldin JSON-ni `sorafs_cli fetch --config` (avtomatlashtirish) yoki SDK ulanishlari (`config_from_json`) orqali yuklang. Audit izlari uchun reyting jadvalining chiqishini (`persist_path`) davom ettiring.

Gateway tomonidagi majburlash tugmalari `docs/examples/sorafs_gateway_direct_mode.toml` da tasvirlangan. Shablon `iroha app sorafs gateway direct-mode enable` dan chiqishni aks ettiradi, konvert/qabul tekshiruvlarini o'chirib qo'yadi, simlarni ulash tezligi chegarasi sukut bo'yicha va `direct_mode` jadvalini rejadan olingan xost nomlari va manifest dayjestlari bilan to'ldiradi. Snippetni konfiguratsiyani boshqarishga topshirishdan oldin to'ldiruvchi qiymatlarni tarqatish rejangiz bilan almashtiring.

## 3. Muvofiqlik test to'plami

To'g'ridan-to'g'ri rejimga tayyorlik endi orkestr va CLI qutilarini qamrab oladi:

- `direct_only_policy_rejects_soranet_only_providers`, `TransportPolicy::DirectOnly` tez ishlamay qolishini kafolatlaydi, agar har bir nomzod reklamasi faqat SoraNet relesini qo'llab-quvvatlasa.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` mavjud bo'lganda Torii/QUIC transportlaridan foydalanishni va SoraNet o'rni sessiyadan chiqarib tashlanishini ta'minlaydi.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` hujjatlarning yordamchi utilitlarga mos kelishini taʼminlash uchun `docs/examples/sorafs_direct_mode_policy.json`ni tahlil qiladi.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_1cy.json_1
- `fetch_command_respects_direct_transports` masxara qilingan Torii shlyuziga qarshi `sorafs_cli fetch --transport-policy=direct-only` mashqlarini bajaradi, bu to'g'ridan-to'g'ri transportlarni bog'laydigan tartibga solinadigan muhitlar uchun tutun sinovini ta'minlaydi.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` ishga tushirishni avtomatlashtirish uchun JSON siyosati va skorbordning barqarorligi bilan bir xil buyruqni o'rab oladi.

Yangilanishlarni nashr qilishdan oldin fokuslangan to'plamni ishga tushiring:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Agar yuqori oqimdagi o'zgarishlar tufayli ish maydoni kompilyatsiyasi muvaffaqiyatsiz bo'lsa, blokirovka xatosini `status.md` da yozib oling va qaramlik ushlangandan keyin qayta ishga tushiring.

## 4. Avtomatlashtirilgan tutun ishlaydi

Faqatgina CLI qamrovi atrof-muhitga xos regressiyalarni (masalan, shlyuz siyosatining o'zgarishi yoki aniq nomuvofiqliklarni) ko'rsatmaydi. Maxsus tutun yordamchisi `scripts/sorafs_direct_mode_smoke.sh`-da yashaydi va `sorafs_cli fetch`-ni to'g'ridan-to'g'ri orkestr siyosati, skorbordning qat'iyligi va xulosa yozish bilan o'rab oladi.

Foydalanish misoli:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Skript CLI bayroqlari va kalit=value konfiguratsiya fayllarini hurmat qiladi (qarang: `docs/examples/sorafs_direct_mode_smoke.conf`). Manifest dayjestini va provayder reklama yozuvlarini ishga tushirishdan oldin ishlab chiqarish qiymatlari bilan to'ldiring.
- `--policy` sukut bo'yicha `docs/examples/sorafs_direct_mode_policy.json` ga o'rnatiladi, lekin `sorafs_orchestrator::bindings::config_to_json` tomonidan ishlab chiqarilgan har qanday orkestr JSON ta'minlanishi mumkin. CLI siyosatni `--orchestrator-config=PATH` orqali qabul qiladi, bu esa qo'lda sozlash bayroqlarisiz takrorlanadigan ishga tushirish imkonini beradi.
- `sorafs_cli` `PATH` da bo'lmasa, yordamchi uni quyidagidan tuzadi.
  `sorafs_orchestrator` sandiq (bo'shatish profili), shuning uchun tutun yuguradi
  etkazib berish to'g'ridan-to'g'ri rejimli sanitariya-tesisat.
- Chiqishlar:
  - Yig'ilgan foydali yuk (`--output`, standart `artifacts/sorafs_direct_mode/payload.bin`).
  - Xulosa olish (`--summary`, foydali yuk bilan bir qatorda sukut bo'yicha) telemetriya hududi va provayder hisobotlarini tarqatish uchun foydalaniladi.
  - Scoreboard snapshoti JSON siyosatida eʼlon qilingan yoʻlda saqlanib qoldi (masalan, `fetch_state/direct_mode_scoreboard.json`). Buni oʻzgartirish chiptalaridagi xulosa bilan birga arxivlang.
- Qabul qilish eshigini avtomatlashtirish: qabul qilish tugallangandan so'ng, yordamchi doimiy jadval va xulosa yo'llari yordamida `cargo xtask sorafs-adoption-check` ni chaqiradi. Kerakli kvorum buyruq satrida taqdim etilgan provayderlar soniga mos keladi; Agar kattaroq namuna kerak bo'lsa, uni `--min-providers=<n>` bilan bekor qiling. Qabul qilish hisobotlari xulosaning yonida yoziladi (`--adoption-report=<path>` maxsus joylashuvni o'rnatishi mumkin) va yordamchi sukut bo'yicha `--require-direct-only` (qayta tiklashga mos) va mos keladigan CLI bayrog'ini taqdim qilganingizda `--require-telemetry` ni uzatadi. Qo'shimcha xtask argumentlarini yo'naltirish uchun `XTASK_SORAFS_ADOPTION_FLAGS` dan foydalaning (masalan, tasdiqlangan pasayish paytida `--allow-single-source`, shuning uchun darvoza to'xtatilishiga toqat qiladi va uni amalga oshiradi). Mahalliy diagnostika paytida faqat `--skip-adoption-check` bilan qabul qilish eshigini o'tkazib yuboring; yo'l xaritasi qabul qilish hisoboti to'plamini o'z ichiga olishi uchun har bir tartibga solinadigan to'g'ridan-to'g'ri rejimni talab qiladi.

## 5. Chiqarish nazorat ro'yxati

1. **Konfiguratsiyani muzlatish:** To‘g‘ridan-to‘g‘ri rejimdagi JSON profilini `iroha_config` omboringizda saqlang va o‘zgartirish chiptangizga xeshni yozib oling.
2. **Gateway auditi:** Torii so'nggi nuqtalari TLS, qobiliyat TLVlari va audit jurnalini to'g'ridan-to'g'ri aylantirishdan oldin amalga oshirishini tasdiqlang. Operatorlar uchun shlyuz siyosati profilini nashr qilish.
3. **Muvofiqlikni ro‘yxatdan o‘tkazish:** Yangilangan o‘quv kitobini muvofiqlik/tartibga solish tekshiruvchilari bilan baham ko‘ring va anonimlik qoplamasidan tashqarida ishlash uchun ruxsatlarni oling.
4. **Quruq ishga tushirish:** Muvofiqlik test to‘plamini va yaxshi ma’lum bo‘lgan Torii provayderlariga nisbatan bosqichma-bosqich yuklashni bajaring. Skorlar jadvali natijalari va CLI xulosalarini arxivlash.
5. **Ishlab chiqarishni qisqartirish:** O'zgartirish oynasini e'lon qiling, `transport_policy` ni `direct_only` ga o'tkazing (agar siz `soranet-first` ni tanlagan bo'lsangiz) va to'g'ridan-to'g'ri boshqaruv panellarini kuzatib boring (I18NI000000071X, provayderning nosozliklari). Qayta tiklash rejasini hujjatlang, shunda SoraNet-ga birinchi marta SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` bitiruvchisiga qaytishingiz mumkin.
6. **Oʻzgartirishdan keyingi koʻrib chiqish:** Oʻzgartirish chiptasiga skorbordning oniy suratlarini, xulosalarni olish va monitoring natijalarini biriktiring. `status.md` ni amal qilish sanasi va har qanday anomaliyalar bilan yangilang.

Tekshirish ro'yxatini `sorafs_node_ops` runbook bilan birga saqlang, shunda operatorlar jonli o'tishdan oldin ish jarayonini takrorlashlari mumkin. SNNet-5 GA ni tamomlaganida, ishlab chiqarish telemetriyasidagi paritetni tasdiqlagandan so'ng, zaxirani bekor qiling.

## 6. Dalillar va farzand asrab olish darvozasiga qo'yiladigan talablar

To'g'ridan-to'g'ri rejimda suratga olish hali ham SF-6c qabul qilish eshigini qondirishi kerak. ni to'plang
skorbord, xulosa, manifest konvert va har bir yugurish uchun qabul qilish hisoboti
`cargo xtask sorafs-adoption-check` orqaga qaytish holatini tasdiqlashi mumkin. Yo'qolgan
maydonlar darvozani ishlamay qolishiga majbur qiladi, shuning uchun o'zgarishda kutilgan metama'lumotlarni yozib oling
chiptalar.

- **Transport metamaʼlumotlari:** `scoreboard.json` eʼlon qilishi kerak
  `transport_policy="direct_only"` (va `transport_policy_override=true` ni aylantiring
  pastga tushirishga majbur qilganingizda). Birlashtirilgan anonimlik siyosati maydonlarini saqlang
  Ular birlamchi qiymatlarni meros qilib olganlarida ham to'ldiriladi, shunda sharhlovchilar sizni ko'rishlari mumkin
  bosqichli anonimlik rejasidan chetga chiqdi.
- **Provayder hisoblagichlari:** Faqat shlyuz seanslari davom etishi kerak `provider_count=0`
  va `gateway_provider_count=<n>` ni Torii provayderlari bilan to'ldiring
  ishlatilgan. JSON-ni qo'lda tahrirlashdan saqlaning - CLI/SDK allaqachon hisoblarni va
  qabul qilish darvozasi bo'linishni o'tkazib yuboradigan suratlarni rad etadi.
- **Ochiq dalillar:** Torii shlyuzlari ishtirok etganda, imzolangan hujjatni topshiring
  `--gateway-manifest-envelope <path>` (yoki SDK ekvivalenti) shunday
  `gateway_manifest_provided` va `gateway_manifest_id`/`gateway_manifest_cid`
  `scoreboard.json` da qayd etilgan. `summary.json` mos kelishiga ishonch hosil qiling
  `manifest_id`/`manifest_cid`; agar biron bir fayl bo'lsa, qabul qilish tekshiruvi muvaffaqiyatsiz tugadi
  juftlikni sog'indim.
- **Telemetriya kutilmalari:** Telemetriya suratga olish bilan birga kelganda, uni ishga tushiring
  gate `--require-telemetry` bilan, shuning uchun qabul qilish hisoboti ko'rsatkichlarni tasdiqlaydi
  chiqarilgan. Havo bo'sh bo'lgan mashqlar bayroqni o'tkazib yuborishi mumkin, ammo CI va chiptalarni o'zgartirish
  yo'qligini hujjatlashtirish kerak.

Misol:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json`ni tablo, xulosa, manifest bilan birga ilova qiling
konvert va tutun jurnali to'plami. Ushbu artefaktlar CI qabul qilish ishini aks ettiradi
(`ci/check_sorafs_orchestrator_adoption.sh`) to'g'ridan-to'g'ri rejimni qo'llaydi va ushlab turadi
pasaytirishlar tekshirilishi mumkin.