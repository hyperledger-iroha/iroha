---
lang: uz
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI qadoqlash bo'yicha qo'llanma

Ushbu qo'llanma MOCHI ish stoli supervayzer to'plamini qanday yaratishni, tekshirishni tushuntiradi
yaratilgan artefaktlarni ko'ring va bilan birga yuboriladigan ish vaqtini bekor qiling
to'plam. Bu takrorlanadigan qadoqlashga e'tibor qaratib, tezkor boshlashni to'ldiradi
va CI foydalanish.

## Old shartlar

- Rust asboblar zanjiri (nashr 2024 / Rust 1.82+) ish maydoniga bog'liqliklari bilan
  allaqachon qurilgan.
- `irohad`, `iroha_cli` va `kagami` kerakli maqsad uchun tuzilgan. The
  bundler `target/<profile>/` dan ikkilik fayllarni qayta ishlatadi.
- `target/` yoki moslashtirilgan to'plam uchun disk maydoni etarli
  maqsad.

Bundlerni ishga tushirishdan oldin bir marta bog'liqliklarni yarating:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## To'plamni yaratish

Repozitoriy ildizidan ajratilgan `xtask` buyrug'ini chaqiring:

```bash
cargo xtask mochi-bundle
```

Odatiy bo'lib, bu `target/mochi-bundle/` ostida relizlar to'plamini ishlab chiqaradi
asosiy operatsion tizim va arxitekturadan olingan fayl nomi (masalan,
`mochi-macos-aarch64-release.tar.gz`). Sozlash uchun quyidagi bayroqlardan foydalaning
qurilish:

- `--profile <name>` – yuk profilini tanlang (`release`, `debug` yoki
  maxsus profil).
- `--no-archive` - kengaytirilgan katalogni `.tar.gz` yaratmasdan saqlang
  arxiv (mahalliy test uchun foydali).
- `--out <path>` - o'rniga to'plamlarni maxsus katalogga yozing
  `target/mochi-bundle/`.
- `--kagami <path>` - dasturga kiritish uchun oldindan tuzilgan `kagami` bajariladigan faylni taqdim eting
  arxiv. O'tkazib yuborilganda, bundler ikkilik fayldan qayta foydalanadi (yoki quradi).
  tanlangan profil.
- `--matrix <path>` – toʻplam metamaʼlumotlarini JSON matritsa fayliga qoʻshish (agar yaratilgan boʻlsa)
  etishmayotgan) shuning uchun CI quvurlari a da ishlab chiqarilgan har bir xost/profil artefaktini yozib olishi mumkin
  yugur. Yozuvlarga toʻplam katalogi, manifest yoʻli va ixtiyoriy SHA-256 kiradi
  arxiv joylashuvi va oxirgi tutun sinovi natijasi.
- `--smoke` - qadoqlangan `mochi --help` ni engil tutun eshigi sifatida bajaring
  yig'ilgandan keyin; nosozliklar e'lon qilishdan oldin etishmayotgan bog'liqliklarni yuzaga chiqaradi
  artefakt.
- `--stage <path>` - tayyor to'plamni (va ishlab chiqarilganda arxivdan) nusxalash
  ko'p platformali tuzilmalar artefaktlarni bittasiga joylashtirishi mumkin bo'lgan sahnalash katalogi
  qo'shimcha skriptsiz joylashuv.

Buyruq `mochi-ui-egui`, `kagami`, `LICENSE`, namunani nusxalaydi
konfiguratsiya va `mochi/BUNDLE_README.md` to'plamga. Deterministik
`manifest.json` ikkilik fayllar bilan birga yaratilgan, shuning uchun CI ishlari faylni kuzatishi mumkin
xeshlar va o'lchamlar.

## To'plamning joylashuvi va tekshiruvi

Kengaytirilgan to'plam `BUNDLE_README.md` da hujjatlashtirilgan tartibga amal qiladi:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

`manifest.json` faylida SHA-256 xesh bilan har bir artefakt ro'yxati keltirilgan. Tasdiqlash
to'plamni boshqa tizimga nusxalashdan keyin:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI quvurlari kengaytirilgan katalogni keshlashi, arxivga imzo qo'yishi yoki nashr etishi mumkin
nashr yozuvlari bilan birga manifest. Manifest generatorni o'z ichiga oladi
kelib chiqishini kuzatishga yordam berish uchun profil, maqsadli uchlik va yaratish vaqt tamg'asi.

## Ish vaqti bekor qilinadi

MOCHI CLI bayroqlari orqali yordamchi ikkilik va ish vaqti manzillarini topadi
atrof-muhit o'zgaruvchilari:- `--data-root` / `MOCHI_DATA_ROOT` - tengdosh uchun ishlatiladigan ish maydonini bekor qilish
  konfiguratsiyalar, saqlash va jurnallar.
- `--profile` - topologiyani oldindan o'rnatish (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` - ajratishda ishlatiladigan asosiy portlarni o'zgartiring
  xizmatlar.
- `--irohad` / `MOCHI_IROHAD` - ma'lum bir `irohad` ikkilik faylida nuqta.
- `--kagami` / `MOCHI_KAGAMI` - to'plamdagi `kagami` ni bekor qilish.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – ixtiyoriy CLI yordamchisini bekor qilish.
- `--restart-mode <never|on-failure>` - avtomatik qayta ishga tushirishni o'chiring yoki majburlang
  eksponentsial orqaga qaytish siyosati.
- `--restart-max <attempts>` – qachon qayta ishga tushirishga urinishlar sonini bekor qiladi
  `on-failure` rejimida ishlaydi.
- `--restart-backoff-ms <millis>` - avtomatik qayta ishga tushirish uchun asosiy orqa o'chirishni o'rnating.
- `MOCHI_CONFIG` - maxsus `config/local.toml` yo'lini taqdim eting.

CLI yordami (`mochi --help`) to'liq bayroq ro'yxatini chop etadi. Atrof-muhit ustunlik qiladi
ishga tushirilganda kuchga kiradi va ichidagi Sozlamalar dialog oynasi bilan birlashtirilishi mumkin
UI.

## CI foydalanish bo'yicha maslahatlar

- Katalog yaratish uchun `cargo xtask mochi-bundle --no-archive` ni ishga tushiring
  platformaga xos asboblar bilan ziplangan bo'lishi mumkin (Windows uchun ZIP, tarballs uchun
  Unix).
- `cargo xtask mochi-bundle --matrix dist/matrix.json` yordamida toʻplam metamaʼlumotlarini yozib oling
  shuning uchun bo'shatish ishlari har bir xost/profil ro'yxatini ko'rsatadigan yagona JSON indeksini nashr etishi mumkin
  quvur liniyasida ishlab chiqarilgan artefakt.
- Har birida `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (yoki shunga o'xshash) dan foydalaning
  to'plamni va arxivni umumiy katalogga yuklash uchun agent yaratish
  nashriyot ishi iste'mol qilishi mumkin.
- Operatorlar paketni tekshirishi uchun arxivni ham, `manifest.json`ni ham nashr qiling
  yaxlitlik.
- Yaratilgan katalogni tutun sinovlari uchun qurilish artefakti sifatida saqlang
  nazoratchini deterministik tarzda paketlangan ikkilik fayllar bilan mashq qiling.
- To'plam xeshlarini nashr yozuvlarida yoki kelajak uchun `status.md` jurnalida yozib oling
  kelib chiqishini tekshirish.