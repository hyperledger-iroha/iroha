---
id: nexus-operator-onboarding
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus data-space operator onboarding
description: Mirror of `docs/source/sora_nexus_operator_onboarding.md`, tracking the end-to-end release checklist for Nexus operators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/sora_nexus_operator_onboarding.md`ni aks ettiradi. Mahalliylashtirilgan nashrlar portalga kelguniga qadar ikkala nusxani ham bir tekisda saqlang.
:::

# Sora Nexus Data-Space operatorini ishga tushirish

Ushbu qo'llanmada Sora Nexus ma'lumotlar fazosi operatorlari reliz e'lon qilingandan so'ng amal qilishi kerak bo'lgan uchdan-end oqimini qamrab oladi. U tugunni onlayn holatga keltirishdan oldin yuklab olingan toʻplamlar/tasvirlar, manifestlar va konfiguratsiya shablonlarini qanday qilib global chiziqli kutishlarga moslashtirishni tasvirlab, ikki yoʻlli ish kitobini (`docs/source/release_dual_track_runbook.md`) va artefakt tanlash eslatmasini (`docs/source/release_artifact_selection.md`) toʻldiradi.

## Tomoshabinlar va shartlar
- Siz Nexus dasturi tomonidan ma'qullangansiz va ma'lumotlar maydoni tayinlangansiz (yo'lak indeksi, ma'lumotlar maydoni identifikatori/taxallus va marshrutlash siyosati talablari).
- Release Engineering tomonidan chop etilgan imzolangan reliz artefaktlariga (tarballs, tasvirlar, manifestlar, imzolar, ochiq kalitlar) kirishingiz mumkin.
- Siz validator/kuzatuvchi rolingiz uchun ishlab chiqarish kaliti materialini yaratdingiz yoki oldingiz (Ed25519 tugun identifikatori; BLS konsensus kaliti + validatorlar uchun PoP; plyus har qanday maxfiy xususiyatni almashtirish).
- Siz tuguningizni yuklaydigan mavjud Sora Nexus tengdoshlariga kirishingiz mumkin.

## 1-qadam — Chiqarish profilini tasdiqlang
1. Sizga berilgan tarmoq taxallus yoki zanjir identifikatorini aniqlang.
2. Ushbu omborni tekshirishda `scripts/select_release_profile.py --network <alias>` (yoki `--chain-id <id>`) ni ishga tushiring. Yordamchi `release/network_profiles.toml` bilan maslahatlashadi va o'rnatish uchun profilni chop etadi. Sora Nexus uchun javob `iroha3` bo'lishi kerak. Boshqa har qanday qiymat uchun, to'xtating va Release Engineering bilan bog'laning.
3. Relizlar e'loniga havola qilingan versiya tegiga e'tibor bering (masalan, `iroha3-v3.2.0`); siz undan artefaktlar va manifestlarni olish uchun foydalanasiz.

## 2-qadam - Artefaktlarni olish va tasdiqlash
1. `iroha3` toʻplamini (`<profile>-<version>-<os>.tar.zst`) va unga qoʻshimcha fayllarni (`.sha256`, ixtiyoriy `.sig/.pub`, `<profile>-<version>-manifest.json` va agar sizda boʻlsa, I18NI010000) yuklab oling.
2. Paketni ochishdan oldin yaxlitligini tekshiring:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Agar apparat tomonidan qoʻllab-quvvatlanadigan KMS dan foydalansangiz, `openssl` ni tashkilot tomonidan tasdiqlangan tekshirgich bilan almashtiring.
3. Tarball ichidagi `PROFILE.toml` ni tekshiring va JSON tasdiqlaydi:
   - `profile = "iroha3"`
   - `version`, `commit` va `built_at` maydonlari chiqarish e'loniga mos keladi.
   - OS/arxitektura sizning joylashtirish maqsadingizga mos keladi.
4. Agar siz konteyner tasviridan foydalansangiz, `<profile>-<version>-<os>-image.tar` uchun xesh/imzo tekshiruvini takrorlang va `<profile>-<version>-image.json` da yozilgan rasm identifikatorini tasdiqlang.

## 3-qadam — Shablonlardan bosqichli konfiguratsiya
1. To'plamni chiqarib oling va `config/` ni tugun o'z konfiguratsiyasini o'qiy oladigan joyga nusxalang.
2. `config/` ostidagi fayllarni shablon sifatida ko'ring:
   - `public_key`/`private_key` ni ishlab chiqarish Ed25519 kalitlari bilan almashtiring. Agar tugun ularni HSM dan manba qilsa, shaxsiy kalitlarni diskdan olib tashlang; o'rniga HSM ulagichiga ishora qilish uchun konfiguratsiyani yangilang.
   - `trusted_peers`, `network.address` va `torii.address` ni sozlang, shunda ular sizning erishish mumkin bo'lgan interfeyslarni va sizga tayinlangan bootstrap tengdoshlarini aks ettiradi.
   - `client.toml` ni operatorga qaragan Torii so'nggi nuqtasi (agar mavjud bo'lsa, TLS konfiguratsiyasi ham) va operatsion asboblar uchun taqdim etgan hisob ma'lumotlari bilan yangilang.
3. Agar Boshqaruv boshqacha ko'rsatmasa, to'plamda ko'rsatilgan zanjir identifikatorini saqlang - global tarmoq bitta kanonik zanjir identifikatorini kutadi.
4. Tugunni Sora profil bayrog'i bilan boshlashni rejalashtiring: `irohad --sora --config <path>`. Agar bayroq yo'q bo'lsa, konfiguratsiya yuklovchisi SoraFS yoki ko'p qatorli sozlamalarni rad etadi.

## 4-qadam - Ma'lumotlar maydoni metama'lumotlari va marshrutlashni tekislang
1. `config/config.toml` ni tahrirlang, shunda `[nexus]` bo'limi Nexus Kengashi taqdim etgan ma'lumotlar maydoni katalogiga mos keladi:
   - `lane_count` joriy davrda yoqilgan jami chiziqlarga teng bo'lishi kerak.
   - `[[nexus.lane_catalog]]` va `[[nexus.dataspace_catalog]]` dagi har bir yozuv noyob `index`/`id` va kelishilgan taxalluslarni o'z ichiga olishi kerak. Mavjud global yozuvlarni o'chirmang; agar kengash qo'shimcha ma'lumotlar bo'shliqlarini tayinlagan bo'lsa, o'zingizning vakolatli taxalluslaringizni qo'shing.
   - Har bir maʼlumot maydoniga `fault_tolerance (f)` kiritilganligiga ishonch hosil qiling; yo'lak-rele qo'mitalari o'lchamlari `3f+1`.
2. Sizga berilgan siyosatni yozib olish uchun `[[nexus.routing_policy.rules]]` ni yangilang. Standart shablon boshqaruv ko'rsatmalarini `1` qatoriga va shartnomalarni `2` qatoriga yo'naltiradi; maʼlumotlar joyingiz uchun moʻljallangan trafik toʻgʻri chiziq va taxallusga yoʻnaltirilishi uchun qoidalarni qoʻshing yoki oʻzgartiring. Qoida tartibini o'zgartirishdan oldin Release Engineering bilan muvofiqlashtiring.
3. `[nexus.da]`, `[nexus.da.audit]` va `[nexus.da.recovery]` chegaralarini ko‘rib chiqing. Operatorlar kengash tomonidan tasdiqlangan qiymatlarni saqlab qolishlari kutilmoqda; ularni faqat yangilangan siyosat ratifikatsiya qilingan taqdirdagina sozlang.
4. Yakuniy konfiguratsiyani operatsiyalar kuzatuvchingizga yozib oling. Ikki trekli relizlar kitobi bortga chiqish chiptasiga samarali `config.toml` (sirlari tahrirlangan) biriktirilishini talab qiladi.

## 5-qadam — Parvoz oldidan tekshirish
1. Tarmoqqa ulanishdan oldin o'rnatilgan konfiguratsiya tekshiruvchini ishga tushiring:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Bu hal qilingan konfiguratsiyani chop etadi va agar katalog/marshrutlash yozuvlari mos kelmasa yoki genezis va konfiguratsiya mos kelmasa, erta bajarilmaydi.
2. Agar siz konteynerlarni joylashtirsangiz, uni `docker load -i <profile>-<version>-<os>-image.tar` bilan yuklaganingizdan so'ng tasvir ichida xuddi shu buyruqni bajaring (`--sora` ni qo'shishni unutmang).
3. To'ldiruvchi qator/ma'lumotlar maydoni identifikatorlari haqidagi ogohlantirishlar uchun jurnallarni tekshiring. Agar biror narsa paydo boʻlsa, 4-bosqichga qayta tashrif buyuring—ishlab chiqarishni qoʻllash andozalar bilan birga joʻnatilgan toʻldiruvchi identifikatorlariga tayanmasligi kerak.
4. Mahalliy tutun protsedurasini bajaring (masalan, `FindNetworkStatus` soʻrovini `iroha_cli` bilan yuboring, telemetriya soʻnggi nuqtalari `nexus_lane_state_total` koʻrinishini tasdiqlang va oqim kalitlari kerak boʻlganda aylantirilgan yoki import qilinganligini tekshiring).

## 6-qadam - Kesish va topshirish
1. Tasdiqlangan `manifest.json` va imzo artefaktlarini chiqarish chiptasida saqlang, shunda auditorlar cheklaringizni takrorlashi mumkin.
2. Nexus Operatsiyalarga tugun joriy qilishga tayyorligi haqida xabar bering; o'z ichiga oladi:
   - Tugun identifikatori (teng identifikatori, xost nomlari, Torii oxirgi nuqtasi).
   - Samarali yo'lak/ma'lumotlar maydoni katalogi va marshrutlash siyosati qiymatlari.
   - Siz tasdiqlagan ikkilik/tasvirlarning xeshlari.
3. `@nexus-core` bilan yakuniy tengdoshlarni qabul qilishni (g'iybat urug'lari va yo'laklarni belgilash) muvofiqlashtiring. Tasdiqlanmaguningizcha tarmoqqa qo'shilmang; Sora Nexus deterministik chiziqli bandlikni ta'minlaydi va yangilangan qabul manifestini talab qiladi.
4. Tugun jonli bo'lgandan so'ng, ishga tushirish kitoblarini o'zingiz kiritgan har qanday bekor qilish bilan yangilang va keyingi iteratsiya ushbu asosiy chiziqdan boshlanishi uchun chiqarish yorlig'iga e'tibor bering.

## Yo'naltiruvchi nazorat ro'yxati
- [ ] `iroha3` sifatida tasdiqlangan reliz profili.
- [ ] Toʻplam/tasvir xeshlari va imzolar tasdiqlangan.
- [ ] Kalitlar, tengdosh manzillar va Torii oxirgi nuqtalari ishlab chiqarish qiymatlariga yangilandi.
- [ ] Nexus qator/maʼlumotlar fazosi katalogi va marshrutlash siyosatiga mos kengash tayinlash.
- [ ] Konfiguratsiya tekshiruvi (`irohad --sora --config … --trace-config`) ogohlantirishlarsiz o'tadi.
- [ ] Bortga chiqish chiptasida arxivlangan manifestlar/imzolar va Ops xabarnomasi.

Nexus migratsiya bosqichlari va telemetriya kutilmalari boʻyicha kengroq kontekst uchun [Nexus oʻtish qaydlari](./nexus-transition-notes) ni koʻrib chiqing.