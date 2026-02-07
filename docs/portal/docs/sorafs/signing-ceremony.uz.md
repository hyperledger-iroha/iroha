---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a4be274ad087f292559c4b83a120ad20316a1e1dfe0ccbfb9aad42235ac136b
source_last_modified: "2026-01-05T09:28:11.909870+00:00"
translation_last_reviewed: 2026-02-07
id: signing-ceremony
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
translator: machine-google-reviewed
---

> Yo‘l xaritasi: **SF-1b — Sora Parlamenti qarorlarini tasdiqlash.**

SoraFS chunker moslamalari uchun qo'lda imzo qo'yish marosimi eskirgan. Hammasi
Tasdiqlar endi **Sora Parlamenti** orqali o'tadi, bu saralash asosidagi DAO
Nexusni boshqaradi. Parlament a'zolari fuqarolikni olish uchun XORni bog'lashadi, bo'ylab aylanadilar
panellar va moslamani ma'qullash, rad etish yoki orqaga qaytarish uchun zanjir bo'yicha ovoz berish
relizlar. Ushbu qo'llanma jarayon va ishlab chiquvchi vositalarini tushuntiradi.

## Parlamentga umumiy nuqtai

- **Fuqarolik** — Operatorlar fuqaro sifatida ro'yxatdan o'tish uchun kerakli XORni bog'laydilar va
  saralash huquqiga ega bo'ladi.
- **Panellar** - Mas'uliyatlar aylanadigan panellar bo'ylab taqsimlanadi (infratuzilma,
  Moderatsiya, G'aznachilik, ...). Infratuzilma paneli SoraFS moslamasiga ega
  tasdiqlashlar.
- **Saralash va aylanish** — Panel oʻrindiqlari bandida koʻrsatilgan kadans boʻyicha qayta chiziladi
  Parlament konstitutsiyasi, shuning uchun hech bir guruh monopoliyani tasdiqlamaydi.

## Fiksturni tasdiqlash oqimi

1. **Taklifni yuborish**
   - Tooling WG nomzod `manifest_blake3.json` bundle plus yuklaydi
     armatura `sorafs.fixtureProposal` orqali zanjirdagi registrdan farq qiladi.
   - Taklif BLAKE3 dayjestini, semantik versiyasini va o'zgartirish qaydlarini qayd etadi.
2. **Ko'rib chiqish va ovoz berish**
   - Infratuzilma kengashi topshiriqni parlament topshirig'i orqali oladi
     navbat.
   - Panel a'zolari CI artefaktlarini tekshiradilar, paritet testlarini o'tkazadilar va o'lchanadi
     zanjir bo'yicha ovoz berish.
3. **Yakunlash**
   - Kvorum bajarilgandan so'ng, ish vaqti ma'qullash hodisasini chiqaradi
     kanonik manifest dayjesti va Merkle armatura yukiga sodiqligi.
   - Voqea SoraFS registrida aks ettirilgan, shuning uchun mijozlar uni olishlari mumkin.
     parlament tomonidan tasdiqlangan oxirgi manifest.
4. **Taqsimot**
   - CLI yordamchilari (`cargo xtask sorafs-fetch-fixture`) tasdiqlangan manifestni tortadi
     Nexus RPC dan. Omborning JSON/TS/Go konstantalari sinxronlashtiriladi
     `export_vectors` ni qayta ishga tushirish va zanjirdagi dayjestni tekshirish
     rekord.

## Ishlab chiquvchining ish jarayoni

- Asboblarni qayta tiklash:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Tasdiqlangan konvertni yuklab olish, tekshirish uchun Parlamentni olib kelish yordamchisidan foydalaning
  imzolar va mahalliy moslamalarni yangilang. `--signatures` nuqtasida
  Parlament tomonidan nashr etilgan konvert; yordamchi hamrohlik qiluvchi manifestni hal qiladi,
  BLAKE3 dayjestini qayta hisoblab chiqadi va kanonikni amalga oshiradi
  `sorafs.sf1@1.0.0` profili.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Agar manifest boshqa URL manzilida yashasa, `--manifest` ga o'ting. Imzosiz konvertlar
mahalliy tutun oqimlari uchun `--allow-unsigned` o'rnatilmasa, rad etiladi.

- Manifestni bosqichma-bosqich shlyuz orqali tekshirishda, maqsad o'rniga Torii
  mahalliy yuklamalar:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Mahalliy CI endi `signer.json` ro'yxatini talab qilmaydi.
  `ci/check_sorafs_fixtures.sh` repo holatini oxirgi bilan solishtiradi
  zanjirdagi majburiyat va ular bir-biridan ajralib chiqqanda muvaffaqiyatsizlikka uchraydi.

## Boshqaruv eslatmalari

- Parlament konstitutsiyasi kvorum, rotatsiya va eskalatsiyani boshqaradi - yo'q
  kassa darajasidagi konfiguratsiya kerak.
- Favqulodda vaziyatni qaytarish parlament moderatorlik paneli orqali amalga oshiriladi. The
  Infratuzilma paneli oldingi manifestga havola qilingan qaytarish taklifini yuboradi
  tasdiqlangandan so'ng nashrning o'rnini bosadigan digest.
- Tarixiy tasdiqlar sud ekspertizasi uchun SoraFS reestrida mavjud bo'lib qoladi.
  takrorlash.

## Tez-tez so'raladigan savollar

- **`signer.json` qayerga ketdi?**  
  U olib tashlandi. Barcha imzolovchi atributlari zanjirda saqlanadi; `manifest_signatures.json`
  omborda faqat eng so'nggisiga mos kelishi kerak bo'lgan ishlab chiquvchi moslamasi mavjud
  tasdiqlash hodisasi.

- **Bizga hali ham mahalliy Ed25519 imzolari kerakmi?**  
  Yoʻq. Parlament ruxsati zanjirdagi artefaktlar sifatida saqlanadi. Mahalliy jihozlar mavjud
  takrorlanuvchanlik uchun, lekin parlament dayjestiga qarshi tasdiqlangan.

- **Guruhlar tasdiqlashlarni qanday nazorat qiladi?**  
  `ParliamentFixtureApproved` hodisasiga obuna bo'ling yoki orqali ro'yxatga olish kitobini so'rang
  Joriy manifest dayjestini va panel chaqiruvini olish uchun Nexus RPC.