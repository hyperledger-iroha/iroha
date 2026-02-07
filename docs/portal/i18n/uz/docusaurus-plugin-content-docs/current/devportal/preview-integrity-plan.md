---
id: preview-integrity-plan
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Ushbu reja har bir portalni oldindan ko'rish artefaktini nashr qilishdan oldin tekshirilishi uchun zarur bo'lgan qolgan ishlarni belgilaydi. Maqsad, sharhlovchilar CI-da o'rnatilgan aniq suratni yuklab olishlarini, nazorat summasi manifestining o'zgarmasligini va Norito metama'lumotlari bilan SoraFS orqali oldindan ko'rishni aniqlashni kafolatlashdir.

## Maqsadlar

- **Deterministik tuzilmalar:** `npm run build` qayta tiklanadigan mahsulot ishlab chiqarishiga va har doim `build/checksums.sha256` chiqarishiga ishonch hosil qiling.
- **Tasdiqlangan oldindan koʻrishlar:** Har bir oldindan koʻrish artefaktini nazorat summasi manifesti bilan joʻnatishini talab qiling va tekshirish amalga oshmasa, nashrni rad etish.
- **Norito tomonidan nashr etilgan metamaʼlumotlar:** Norito JSON sifatida oldindan koʻrish deskriptorlari (metamaʼlumotlar, nazorat summasi dayjesti, SoraFS CID) davom etadi, shuning uchun boshqaruv vositalari relizlarni tekshirishi mumkin.
- **Operator asboblari:** Iste'molchilar mahalliy sifatida ishlashi mumkin bo'lgan bir bosqichli tekshirish skriptini taqdim eting (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); skript endi nazorat summasi + deskriptorni tekshirish oqimini oxirigacha o'rab oladi. Standart ko‘rib chiqish buyrug‘i (`npm run serve`) endi bu yordamchini `docusaurus serve` dan oldin avtomatik ravishda chaqiradi, shuning uchun mahalliy oniy suratlar nazorat summasi bilan bog‘langan bo‘lib qoladi (`npm run serve:verified` aniq taxallus sifatida saqlanadi).

## 1-bosqich - CI ijrosi

1. `.github/workflows/docs-portal-preview.yml` ni yangilang:
   - `node docs/portal/scripts/write-checksums.mjs` ni Docusaurus qurgandan so'ng ishga tushiring (allaqachon mahalliy sifatida chaqirilgan).
   - `cd build && sha256sum -c checksums.sha256` ni bajaring va nomuvofiqlikda ishni bajarib bo'lmaydi.
   - Qurilish katalogini `artifacts/preview-site.tar.gz` sifatida to'plang, nazorat summasi manifestidan nusxa oling, `scripts/generate-preview-descriptor.mjs` ga qo'ng'iroq qiling va `scripts/sorafs-package-preview.sh` ni JSON konfiguratsiyasi bilan bajaring (qarang: `docs/examples/sorafs_preview_publish.json`), shuning uchun ish jarayoni ham metama'lumotlarni chiqaradi, ham I0NT908 to'plam.
   - Statik saytni, metamaʼlumotlar artefaktlarini (`docs-portal-preview`, `docs-portal-preview-metadata`) va SoraFS toʻplamini (`docs-portal-preview-sorafs`) yuklang, shunda manifest, CAR xulosasi va qurilishni qayta tekshirishsiz rejalashtirish mumkin.
2. Pull so‘rovlarida nazorat summasini tekshirish natijasini jamlovchi CI nishoni sharhini qo‘shing (✅ `docs-portal-preview.yml` GitHub skripti sharhlash bosqichi orqali amalga oshiriladi).
3. Ish jarayonini `docs/portal/README.md` (CI bo'limi) da hujjatlashtiring va nashrni tekshirish ro'yxatidagi tekshirish bosqichlariga havola qiling.

## Tekshirish skripti

`docs/portal/scripts/preview_verify.sh` yuklab olingan oldindan ko'rish artefaktlarini qo'lda `sha256sum` chaqiruvlarini talab qilmasdan tasdiqlaydi. Skriptni ishga tushirish uchun `npm run serve` (yoki aniq `npm run serve:verified` taxallus) dan foydalaning va mahalliy suratlarni almashishda `docusaurus serve`ni bir qadamda ishga tushiring. Tekshirish mantig'i:

1. `build/checksums.sha256` ga qarshi tegishli SHA vositasini (`sha256sum` yoki `shasum -a 256`) ishga tushiradi.
2. Ixtiyoriy ravishda oldindan ko‘rish deskriptorining `checksums_manifest` dayjesti/fayl nomi va taqdim etilganda oldindan ko‘rish arxivi dayjesti/fayl nomini solishtiradi.
3. Har qanday nomuvofiqlik aniqlanganda noldan farq qiladi, shunda ko'rib chiquvchilar o'zgartirilgan oldindan ko'rishlarni bloklashi mumkin.

Foydalanish misoli (CI artefaktlarini ajratib olgandan keyin):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI va reliz muhandislari oldindan ko'rish to'plamini yuklab olganlarida yoki reliz chiptasiga artefakt biriktirganda skriptga qo'ng'iroq qilishlari kerak.

## 2-bosqich — SoraFS nashriyot

1. Ko'rib chiqish ish jarayonini quyidagi vazifa bilan kengaytiring:
   - `sorafs_cli car pack` va `manifest submit` yordamida qurilgan saytni SoraFS staging shlyuziga yuklaydi.
   - Qaytgan manifest dayjestini va SoraFS CIDni yozib oladi.
   - `{ commit, branch, checksum_manifest, cid }`-ni Norito JSON (`docs/portal/preview/preview_descriptor.json`) ga seriyalashtiradi.
2. Deskriptorni qurilish artefaktining yonida saqlang va tortish so'rovi sharhida CIDni ko'rsating.
3. Kelajakdagi o'zgarishlar metama'lumotlar sxemasini barqaror ushlab turishini ta'minlash uchun `sorafs_cli` ni quruq rejimda ishlatadigan integratsiya testlarini qo'shing.

## 3-bosqich — Boshqaruv va audit

1. `docs/portal/schemas/` ostida deskriptor tuzilishini tavsiflovchi Norito sxemasini (`PreviewDescriptorV1`) nashr eting.
2. Quyidagilarni talab qilish uchun DOCS-SORA nashriyot roʻyxatini yangilang:
   - Yuklangan CIDga qarshi `sorafs_cli manifest verify` ishga tushirildi.
   - Relizning PR tavsifida nazorat summasi manifest dayjestini va CIDni yozib olish.
3. Ovoz berish jarayonida identifikatorni nazorat summasi manifestiga nisbatan o'zaro tekshirish uchun boshqaruvni avtomatlashtirishga ulang.

## Yetkazib berish va egalik qilish

| Muhim bosqich | Ega(lar)i | Maqsad | Eslatmalar |
|----------|----------|--------|-------|
| CI nazorat summasi ijrosi tushdi | Hujjatlar infratuzilmasi | 1-hafta | Muvaffaqiyatsizlik eshigi + yuklangan artefaktlarni qo'shadi. |
| SoraFS oldindan ko'rish nashri | Hujjatlar infratuzilmasi / Saqlash jamoasi | 2-hafta | Sting hisob ma'lumotlari va Norito sxema yangilanishlariga kirishni talab qiladi. |
| Boshqaruv integratsiyasi | Docs/DevRel Lead / Governance WG | 3-hafta | Sxemani nashr etadi + nazorat ro'yxatlarini va yo'l xaritasi yozuvlarini yangilaydi. |

## Ochiq savollar

- Qaysi SoraFS muhiti oldindan ko'rish artefaktlarini saqlashi kerak (sahnalash va ajratilgan ko'rish chizig'i)?
- Nashr qilishdan oldin oldindan ko'rish deskriptorida ikkita imzo (Ed25519 + ML-DSA) kerakmi?
- `sorafs_cli` ishga tushirilganda manifestlarni takrorlanishi uchun CI ish oqimi orkestr konfiguratsiyasini (`orchestrator_tuning.json`) mahkamlashi kerakmi?

`docs/portal/docs/reference/publishing-checklist.md`-da qarorlarni qabul qiling va noma'lumlar hal qilinganidan keyin ushbu rejani yangilang.