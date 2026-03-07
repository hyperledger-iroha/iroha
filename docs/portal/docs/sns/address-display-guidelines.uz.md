---
lang: uz
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 91a1116d82eb9845a26805effed00ad8cd13c1ebd034aef3827aebfa4e1eb846
source_last_modified: "2026-01-28T17:11:30.700223+00:00"
translation_last_reviewed: 2026-02-07
id: address-display-guidelines
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for IH58 vs compressed (`sora`) Sora address presentation (ADDR-6).
translator: machine-google-reviewed
---

ExplorerAddressCard-ni '@site/src/components/ExplorerAddressCard' dan import qiling;

::: Eslatma Kanonik manba
Bu sahifa `docs/source/sns/address_display_guidelines.md`ni aks ettiradi va hozir xizmat qiladi
kanonik portal nusxasi sifatida. Manba fayli tarjima PRlari uchun joylashadi.
:::

Hamyonlar, tadqiqotchilar va SDK namunalari hisob manzillarini o'zgarmas deb hisoblashlari kerak
foydali yuklar. Android chakana hamyon namunasi
`examples/android/retail-wallet` endi kerakli UX namunasini namoyish etadi:

- **Ikki nusxa ko'chirish maqsadlari.** Ikkita aniq nusxa ko'chirish tugmalarini yuboring — IH58 (afzal) va
  siqilgan Sora faqat shakli (`sora…`, ikkinchi eng yaxshi). IH58 har doim tashqaridan baham ko'rish uchun xavfsizdir
  va QR foydali yukini quvvatlaydi. Siqilgan variant qatorni o'z ichiga olishi kerak
  ogohlantirish, chunki u faqat Sora-xabar ilovalar ichida ishlaydi. Android chakana savdosi
  hamyon namuna simlari ham Materiallar tugmalari va ularning maslahatlari
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, va
  iOS SwiftUI demosi `AddressPreviewCard` orqali bir xil UXni aks ettiradi
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monobo'shliq, tanlanishi mumkin bo'lgan matn.** Ikkala qatorni ham monofazo shrift bilan ko'rsating va
  `textIsSelectable="true"`, shuning uchun foydalanuvchilar IMEni chaqirmasdan qiymatlarni tekshirishlari mumkin.
  Tahrirlanadigan maydonlardan saqlaning: IME'lar kanani qayta yozishi yoki nol kenglikdagi kod nuqtalarini kiritishi mumkin.
- **Bevosita standart domen maslahatlari.** Selektor yashiringa ishora qilganda
  `default` domeni, operatorlarga qo'shimcha shart emasligini eslatuvchi sarlavha qo'ying.
  Tadqiqotchilar selektorda kanonik domen yorlig'ini ham ta'kidlashlari kerak
  digestni kodlaydi.
- **IH58 QR foydali yuklari.** QR kodlari IH58 qatorini kodlashi kerak. Agar QR ishlab chiqarish
  bajarilmasa, bo'sh rasm o'rniga aniq xatoni ko'rsating.
- **Buferdagi xabarlar.** Siqilgan shakldan nusxa olgandan so'ng, tost yoki
  snackbar foydalanuvchilarga faqat Sora va IME buzilishiga moyil ekanligini eslatib turadi.

Ushbu to'siqlarga rioya qilish Unicode/IME buzilishining oldini oladi va talablarni qondiradi
Hamyon/explorer UX uchun ADDR-6 yo‘l xaritasini qabul qilish mezonlari.

## Skrinshot uchun moslamalar

Tugma yorliqlarini ta'minlash uchun mahalliylashtirishni tekshirish paytida quyidagi qurilmalardan foydalaning,
maslahatlar va ogohlantirishlar platformalar bo'ylab mos keladi:

- Android uchun havola: `/img/sns/address_copy_android.svg`

  ![Android dual copy mos yozuvi](/img/sns/address_copy_android.svg)

- iOS ma'lumotnomasi: `/img/sns/address_copy_ios.svg`

  ![iOS ikki nusxadagi havola](/img/sns/address_copy_ios.svg)

## SDK yordamchilari

Har bir SDK IH58 (afzal) va siqilgan (`sora`, ikkinchi eng yaxshi) ni qaytaradigan qulaylik yordamchisini taqdim etadi.
UI qatlamlari izchil bo'lib qolishi uchun ogohlantirish qatori bilan birga shakllar:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspektori: `inspectAccountId(...)` siqilgan ogohlantirishni qaytaradi
  string va qo'ng'iroq qiluvchilar `sora…` taqdim etganda uni `warnings` ga qo'shadi
  so'zma-so'z, shuning uchun tadqiqotchilar/hamyon asboblar panelida faqat Sora xabarnomasi paydo bo'lishi mumkin
  joylashtirish/validatsiya oqimlari o'rniga faqat ular hosil qilganda
  siqilgan shaklning o'zlari.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Tezkor: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

UI qatlamlarida kodlash mantig'ini qayta tiklash o'rniga ushbu yordamchilardan foydalaning.
JavaScript yordamchisi `domainSummary` da `selector` foydali yukini ham ochib beradi.
(`tag`, `digest_hex`, `registry_id`, `label`) shuning uchun UIlar
selektor Local-12 yoki xom yukni qayta tahlil qilmasdan registr tomonidan qo'llab-quvvatlanadi.

## Explorer asboblari demosi

<ExplorerAddressCard />

Tadqiqotchilar hamyon telemetriyasi va foydalanish imkoniyatini aks ettirishi kerak:

- Nusxalash tugmalari uchun `data-copy-mode="ih58|compressed|qr"` ni qo'llang, shunda old qismlar foydalanish hisoblagichlarini chiqarishi mumkin
  Torii tomoni `torii_address_format_total` metrikasi bilan birga. Yuqoridagi demo komponenti yuboriladi
  `iroha:address-copy` hodisasi `{mode,timestamp}` bilan - buni analitika/telemetriyaga ulang
  asboblar paneli serverni o'zaro bog'lashi uchun quvur liniyasi (masalan, Segmentga surish yoki NORITO tomonidan qo'llab-quvvatlanadigan kollektor)
  mijozning nusxa ko'chirish harakati bilan manzil formatidan foydalanish. Shuningdek, Torii domen hisoblagichlarini aks ettiring
  (`torii_address_domain_total{domain_kind}`) bir xil tasmada, shuning uchun Mahalliy-12 pensiya sharhlari
  30 kunlik `domain_kind="local12"` noldan foydalanish isbotini bevosita `address_ingest` dan eksport qiling
  Grafana doska.
- Har bir boshqaruvni aniq `aria-label`/`aria-describedby` ko'rsatmalari bilan bog'lang.
  literal almashish xavfsiz (`IH58`) yoki faqat Sora (siqilgan `sora`). Yashirin domen sarlavhasini kiriting
  Ta'rif, shuning uchun yordamchi texnologiya vizual ravishda ko'rsatilgan kontekstni yuzaga keltiradi.
- Nusxa natijalarini e'lon qiladigan jonli hududni (masalan, `<output aria-live="polite">…</output>`) ko'rsatish va
  Endi Swift/Android namunalariga ulangan VoiceOver/TalkBack xatti-harakatlariga mos keladigan ogohlantirishlar.

Ushbu qurilma operatorlar Torii yutilishini ham kuzatishi mumkinligini isbotlash orqali ADDR-6b ni qondiradi.
Mahalliy tanlagichlar o'chirilishidan oldin mijoz tomonidan nusxa ko'chirish rejimlari.

## Mahalliy → Global migratsiya vositalari toʻplami

Avtomatlashtirish uchun [Local → Global Toolkit](local-to-global-toolkit.md) dan foydalaning
JSON audit hisoboti va o'zgartirilgan afzal qilingan IH58 / operatorlar biriktiradigan ikkinchi eng yaxshi siqilgan (`sora`) ro'yxati
Tayyorlik chiptalari bilan birga kelgan runbook Grafana ni bog'laydi.
asboblar paneli va Alertmanager qoidalari qat'iy rejimni kesib o'tadi.

## Ikkilik tartibning tezkor ma'lumotnomasi (ADDR-1a)

SDKlar rivojlangan manzil vositalarini (inspektorlar, tekshirish bo'yicha maslahatlar,
manifest quruvchilari), ishlab chiquvchilarni tasvirlangan kanonik sim formatiga ishora qiling
`docs/account_structure.md`. Tartib har doim
`header · selector · controller`, bu erda sarlavha bitlari:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits7‑5) bugun; nolga teng bo'lmagan qiymatlar saqlangan va kerak
  `AccountAddressError::InvalidHeaderVersion` ko'taring.
- `addr_class` bitta (`0`) va multisig (`1`) kontrollerlarini ajratib turadi.
- `norm_version = 1` Normv1 selektor qoidalarini kodlaydi. Kelajakdagi normalar qayta qo'llaniladi
  bir xil 2 bitli maydon.
- `ext_flag` har doim `0` - o'rnatilgan bitlar qo'llab-quvvatlanmaydigan foydali yuk kengaytmalarini bildiradi.

Selektor darhol sarlavhani kuzatib boradi:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI va SDK sirtlari selektor turini ko'rsatishga tayyor bo'lishi kerak:

- `0x00` = yashirin standart domen (yuksiz).
- `0x01` = mahalliy dayjest (12 bayt `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = global registrga kirish (katta-endian `registry_id:u32`).

Hamyon asboblari hujjatlar/testlarga ulanishi yoki joylashtirishi mumkin bo'lgan kanonik olti burchakli misollar:

| Selektor turi | Kanonik hex |
|-------------|---------------|
| Yashirin sukut | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Mahalliy dayjest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Global registr (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

To'liq selektor/holat uchun `docs/source/references/address_norm_v1.md` ga qarang
to'liq bayt diagrammasi uchun jadval va `docs/account_structure.md`.

## Kanonik shakllarni qo'llash

satrlar ADDR-5 ostida hujjatlashtirilgan CLI ish jarayoniga mos kelishi kerak:

1. `iroha tools address inspect` endi IH58 bilan tuzilgan JSON xulosasini chiqaradi,
   siqilgan va kanonik olti burchakli foydali yuklar. Xulosa shuningdek, `domain` ni ham o'z ichiga oladi
   `kind`/`warning` maydonlariga ega ob'ekt va har qanday taqdim etilgan domen orqali aks sado beradi.
   `input_domain` maydoni. `kind` `local12` bo'lsa, CLI ogohlantirishni chop etadi.
   stderr va JSON xulosasi bir xil ko'rsatmalarni aks ettiradi, shuning uchun CI quvurlari va SDKlar
   uni yuzaga keltirishi mumkin. Konvertatsiya qilishni xohlagan vaqtda `legacy  suffix` ga o'ting
   kodlash `<ih58>@<domain>` sifatida takrorlandi.
2. SDK JavaScript yordamchisi orqali bir xil ogohlantirish/xulosa chiqarishi mumkin:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Yordamchi literaldan aniqlangan IH58 prefiksini saqlaydi, agar siz bo'lmasa
  `networkPrefix` ni aniq taqdim eting, shuning uchun standart bo'lmagan tarmoqlar uchun xulosalar
  standart prefiks bilan jimgina qayta ko'rsatmang.

3. `ih58.value` yoki `compressed` dan qayta foydalanish orqali kanonik foydali yukni aylantiring.
   Xulosadagi maydonlarni (yoki `--format` orqali boshqa kodlashni talab qiling). Bular
   strings allaqachon tashqaridan almashish uchun xavfsiz.
4. Manifestlar, registrlar va mijozlarga tegishli hujjatlarni yangilang
   kanonik shakl va kontragentlarga mahalliy tanlovchilar bo'lishi haqida xabar bering
   kesish tugagach, rad etiladi.
5. Ommaviy ma'lumotlar to'plamlari uchun ishga tushiring
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Buyruq
   yangi satr bilan ajratilgan harflarni o'qiydi (`#` bilan boshlangan izohlar e'tiborga olinmaydi va
   `--input -` yoki hech qanday bayroq STDIN ishlatmaydi), JSON hisobotini chiqaradi
   kanonik/afzal IH58/ikkinchi-eng yaxshi siqilgan (`sora`) har bir yozuv uchun xulosalar va ikkala tahlilni ham hisoblaydi
   keraksiz qatorlarni o'z ichiga olgan axlatxonalar va `strict CI post-check` bilan eshiklarni avtomatlashtirish
   operatorlar CI-da Mahalliy selektorlarni bloklashga tayyor bo'lganda.
6. Yangi satrdan yangi qatorga qayta yozish kerak bo'lganda, foydalaning
  Mahalliy tanlagichni tuzatish elektron jadvallari uchun foydalaning
  bir o'tishda kanonik kodlashlar, ogohlantirishlar va tahlil xatolarini ta'kidlaydigan `input,status,format,…` CSV ni eksport qilish.
   Yordamchi sukut bo'yicha Lokal bo'lmagan qatorlarni o'tkazib yuboradi, qolgan har bir yozuvni o'zgartiradi
   so'ralgan kodlashga (IH58 afzal/siqilgan (`sora`) ikkinchi eng yaxshi/hex/JSON) kiritadi va saqlaydi
   `legacy  suffix` o'rnatilganda asl domen. Uni `--allow-errors` bilan bog'lang
   axlatda noto'g'ri shakllangan harflar bo'lsa ham skanerlashni davom ettirish.
7. CI/lint avtomatizatsiyasi chiqaradigan `ci/check_address_normalize.sh`-ni ishga tushirishi mumkin.
   `fixtures/account/address_vectors.json` dan mahalliy selektorlar, o'zgartiradi
   ularni `iroha tools address normalize` orqali va qayta o'ynaydi
   Relizlar endi chiqmasligini isbotlash uchun `iroha tools address audit`
   Mahalliy hazm qilish.`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` va
Grafana taxtasi `dashboards/grafana/address_ingest.json` ijroni ta'minlaydi
signal: ishlab chiqarish asboblar panelida nol qonuniy Mahalliy taqdimotlar va
30 kun ketma-ket nol Lokal-12 to'qnashuvi, Torii Local-8ni aylantiradi
Mainnet tarmog'ida qattiq muvaffaqiyatsizlikka uchragan darvoza, keyin esa global domenlar mavjud bo'lganda Local-12
mos keladigan ro'yxatga olish kitobi yozuvlari. Operatorga tegishli bildirishnomaning CLI chiqishini ko'rib chiqing
bu muzlatish uchun - bir xil ogohlantirish qatori SDK asboblar maslahatlarida ishlatiladi va
yo'l xaritasidan chiqish mezonlari bilan tenglikni saqlash uchun avtomatlashtirish. Torii endi sukut bo'yicha
regressiyalarni tashxislashda. `torii_address_domain_total{domain_kind}` aks ettirishda davom eting
Grafana (`dashboards/grafana/address_ingest.json`) ichiga, shuning uchun ADDR-7 dalillar to'plami
oldin talab qilinadigan 30 kunlik oyna uchun `domain_kind="local12"` nolda qolganini isbotlay oladi
(`dashboards/alerts/address_ingest_rules.yml`) uchta himoya panjarasini qo'shadi:

- `AddressLocal8Resurgence` sahifalar har safar kontekst yangi Local-8 haqida xabar berganda
  oshirish. Qattiq rejimni ishga tushirishni to'xtating, SDK-ning buzilgan joyini toping
  signal nolga qaytgunicha - keyin standartni tiklang (`true`).
- `AddressLocal12Collision` ikkita Local-12 yorlig'i bir xil bo'lganda yonadi
  hazm qilish. Manifest reklamalarini to'xtatib turing, tekshirish uchun Mahalliy → Global asboblar to'plamini ishga tushiring
  dayjest xaritasini tuzing va uni qayta chiqarishdan oldin Nexus boshqaruvi bilan muvofiqlashtiring.
  registrga kirish yoki quyi oqimlarni qayta yoqish.
- `AddressInvalidRatioSlo` flot bo'ylab yaroqsiz nisbatda ogohlantiradi (istisno).
  Mahalliy-8/qat'iy rejimni rad etish) o'n daqiqa davomida 0,1% SLO dan oshadi. Foydalanish
  `torii_address_invalid_total` mas'ul kontekstni/sababni va
  qat'iy rejimni qayta yoqishdan oldin egasi SDK jamoasi bilan muvofiqlashtiring.

### Eslatma parchasini chiqarish (hamyon va tadqiqotchi)

Yuborilganda hamyon/tadqiqotchi relizlar qaydlariga quyidagi o‘qni qo‘shing
kesish:

> **Manzillar:** `iroha tools address normalize` qo‘shildi
> yordamchi va uni CI (`ci/check_address_normalize.sh`) ga ulab qo'ydi, shuning uchun hamyon/explorer
> Local-8/Local-12 asosiy tarmoqda bloklanishidan oldin. Har qanday maxsus eksportlarni yangilang
> buyruqni bajaring va normallashtirilgan ro'yxatni chiqarish dalillari to'plamiga qo'shing.