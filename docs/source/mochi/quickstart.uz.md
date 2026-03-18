---
lang: uz
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2025-12-29T18:16:35.985408+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI Tez boshlash

**MOCHI** mahalliy Hyperledger Iroha tarmoqlari uchun ish stoli nazoratchisi. Ushbu qo'llanma orqali o'tadi
old shartlarni o'rnatish, ilovani yaratish, egui qobig'ini ishga tushirish va dan foydalanish
kundalik ishlab chiqish uchun ish vaqti vositalari (sozlamalar, suratlar, oʻchirish vositalari).

## Old shartlar

- Rust asboblar zanjiri: `rustup default stable` (ish maydoni maqsadlari nashri 2024 / Rust 1.82+).
- platforma asboblar zanjiri:
  - macOS: Xcode buyruq qatori vositalari (`xcode-select --install`).
  - Linux: GCC, pkg-config, OpenSSL sarlavhalari (`sudo apt install build-essential pkg-config libssl-dev`).
- Iroha ish maydoniga bog'liqliklar:
  - `cargo xtask mochi-bundle` uchun qurilgan `irohad`, `kagami` va `iroha_cli` talab qilinadi. Ularni bir marta orqali yarating
    `cargo build -p irohad -p kagami -p iroha_cli`.
- Majburiy emas: mahalliy yuk ikkiliklarini boshqarish uchun `direnv` yoki `cargo binstall`.

MOCHI CLI ikkilik fayllarini o'z ichiga oladi. Ularning atrof-muhit o'zgaruvchilari orqali aniqlanishiga ishonch hosil qiling
quyida yoki PATHda mavjud:

| Ikkilik | Atrof-muhitni bekor qilish | Eslatmalar |
|----------|----------------------|-----------------------------------------|
| `irohad` | `MOCHI_IROHAD` | Tengdoshlarni nazorat qiladi |
| `kagami` | `MOCHI_KAGAMI` | Genezis manifestlari/snapshotlarini yaratadi |
| `iroha_cli` | `MOCHI_IROHA_CLI` | Kelgusi yordamchi funksiyalar uchun ixtiyoriy |

## MOCHI qurish

Repozitoriy ildizidan:

```bash
cargo build -p mochi-ui-egui
```

Bu buyruq ikkala `mochi-core` va egui frontendini quradi. Tarqaladigan to'plamni ishlab chiqarish uchun quyidagilarni bajaring:

```bash
cargo xtask mochi-bundle
```

To'plam vazifasi `target/mochi-bundle` ostida ikkilik, manifest va konfiguratsiya stublarini yig'adi.

## Egui qobig'ini ishga tushirish

UIni to'g'ridan-to'g'ri yukdan boshqaring:

```bash
cargo run -p mochi-ui-egui
```

Odatiy bo'lib, MOCHI vaqtinchalik ma'lumotlar katalogida bitta peerli oldindan o'rnatishni yaratadi:

- Ma'lumotlar ildizi: `$TMPDIR/mochi`.
- Torii asosiy port: `8080`.
- P2P asosiy porti: `1337`.

Ishga tushirishda standart sozlamalarni bekor qilish uchun CLI bayroqlaridan foydalaning:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

Atrof-muhit o'zgaruvchilari CLI bayroqlari o'tkazib yuborilganda bir xil bekor qilishlarni aks ettiradi: `MOCHI_DATA_ROOT` o'rnating,
`MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`, `MOCHI_P2P_START`, `MOCHI_RESTART_MODE`,
`MOCHI_RESTART_MAX` yoki `MOCHI_RESTART_BACKOFF_MS` nazoratchi quruvchini oldindan belgilash uchun; ikkilik yo'llar
`MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI` va `MOCHI_CONFIG` nuqtalarini hurmat qilishda davom eting.
aniq `config/local.toml`.

## Sozlamalar va doimiylik

Nazoratchi konfiguratsiyasini sozlash uchun asboblar panelidagi **Sozlamalar** muloqot oynasini oching:

- **Maʼlumotlar ildizi** — tengdosh konfiguratsiyalar, saqlash, jurnallar va suratlar uchun asosiy katalog.
- **Torii / P2P tayanch portlari** — deterministik ajratish uchun boshlang'ich portlar.
- **Jurnal ko‘rinishi** — jurnalni ko‘rish vositasida stdout/stderr/tizim kanallarini almashtirish.

Supervayzerni qayta ishga tushirish siyosati kabi kengaytirilgan tugmalar mavjud
`config/local.toml`. O'chirish uchun `[supervisor.restart] mode = "never"` sozlang
nosozliklarni tuzatish paytida avtomatik qayta ishga tushirish yoki sozlash
`max_restarts`/`backoff_ms` (konfiguratsiya fayli yoki CLI bayroqlari orqali
Qayta urinishlarni boshqarish uchun `--restart-mode`, `--restart-max`, `--restart-backoff-ms`)
xatti-harakati.O'zgarishlarni qo'llash supervayzerni qayta tiklaydi, ishlaydigan barcha tengdoshlarni qayta ishga tushiradi va bekor qilishlarni yozadi
`config/local.toml`. Konfiguratsiya birlashmasi ilg'or foydalanuvchilar saqlab qolishi uchun bog'liq bo'lmagan kalitlarni saqlaydi
MOCHI tomonidan boshqariladigan qiymatlar bilan bir qatorda qo'lda sozlash.

## Suratlar va oʻchirish/qayta genezis

**Xizmat** muloqot oynasi ikkita xavfsizlik amalini ko‘rsatadi:

- **Eksport snapshot** — tengdosh xotira/konfiguratsiya/jurnallar va joriy genezisdan nusxa oladi
  `snapshots/<label>` faol ma'lumotlar ildizi ostida. Yorliqlar avtomatik ravishda tozalanadi.
- **Snapshotni tiklash** — tengdosh xotirasi, oniy rasm ildizlari, konfiguratsiyalar, jurnallar va genezisni qayta namlaydi
  mavjud to'plamdan manifest. `Supervisor::restore_snapshot` mutlaq yo'lni yoki qabul qiladi
  tozalangan `snapshots/<label>` papka nomi; UI bu oqimni aks ettiradi, shuning uchun Maintenance → Restore
  fayllarga qo'lda tegmasdan dalillar to'plamlarini takrorlashi mumkin.
- **Wipe & re-genesis** — tengdoshlarning ishlashini to'xtatadi, saqlash kataloglarini o'chiradi, genezisni qayta tiklaydi.
  Kagami va tozalash tugagach, tengdoshlarni qayta ishga tushiradi.

Ikkala oqim ham regressiya testlari bilan qoplangan (`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`) deterministik natijalarni kafolatlash uchun.

## Jurnallar va oqimlar

Boshqaruv paneli ma'lumotlar/ko'rsatkichlarni bir qarashda ko'rsatadi:

- **Jurnallar** — `irohad` stdout/stderr/tizimning hayot aylanishi haqidagi xabarlarga amal qiladi. Sozlamalarda kanallarni almashtiring.
- **Bloklar / Voqealar** — boshqariladigan oqimlar eksponensial orqaga qaytish va izohli kadrlar bilan avtomatik qayta ulanadi
  Norito-dekodlangan xulosalar bilan.
- **Status** — `/status` so'rovi va navbat chuqurligi, o'tkazish qobiliyati va kechikish uchun sparklines beradi.
- **Ishga tushirishga tayyorlik** — **Start** tugmasini bosgandan so'ng (bitta teng yoki barcha tengdoshlar), MOCHI zondlari
  `/status` chegaralangan orqaga qaytish bilan; banner har bir tengdosh tayyor bo'lganda xabar beradi (kuzatilgan bilan
  navbat chuqurligi) yoki tayyorlik vaqti tugasa, Torii xatosini yuzaga keltiradi.

Davlat tadqiqotchisi va bastakor uchun yorliqlar hisoblar, aktivlar, tengdoshlar va umumiy hisoblarga tezkor kirish imkonini beradi
foydalanuvchi interfeysidan chiqmasdan ko'rsatmalar. Tengdoshlar ko'rinishi `FindPeers` so'rovini aks ettiradi, shuning uchun siz tasdiqlashingiz mumkin
integratsiya testlarini o'tkazishdan oldin qaysi ochiq kalitlar validator to'plamida ro'yxatdan o'tgan.

Imzolovchi organlarni import qilish yoki tahrirlash uchun kompozitor asboblar panelidagi **Imzolash omborini boshqarish** tugmasidan foydalaning. The
dialog oynasi faol tarmoq ildiziga (`<data_root>/<profile>/signers.json`) yozuvlarni yozadi va saqlanadi
kassa kalitlari tranzaktsiyalarni oldindan ko'rish va yuborish uchun darhol mavjud. Kassa bo'lganda
bo'sh bo'lsa, kompozitor birlashtirilgan ishlab chiqish kalitlariga qaytadi, shuning uchun mahalliy ish oqimlari ishlashda davom etadi.
Shakllar endi yalpiz/yoqish/o‘tkazish (jumladan, yashirin qabul qilish), domen/hisob/aktiv ta’rifini qamrab oladi.
ro'yxatdan o'tish, hisobga kirish siyosati, multisig takliflari, kosmik katalog manifestlari (AXT/AMX),
SoraFS pin manifestlari va rollarni berish yoki bekor qilish kabi boshqaruv harakatlari juda keng tarqalgan
yo'l xaritasini yaratish vazifalari Norito foydali yuklarni qo'lda yozmasdan takrorlanishi mumkin.

## Tozalash va muammolarni bartaraf etish- Nazorat ostidagi tengdoshlarni tugatish uchun ilovani to'xtating.
- Barcha holatni tiklash uchun ma'lumotlar ildizini (`rm -rf <data_root>`) olib tashlang.
- Agar Kagami yoki irohad manzillari o'zgarsa, muhit o'zgaruvchilarini yangilang yoki MOCHI-ni qayta ishga tushiring.
  tegishli CLI bayroqlari; Sozlamalar dialog oynasi keyingi qo'llashda yangi yo'llar saqlanib qoladi.

Qo'shimcha avtomatlashtirishni tekshirish uchun `mochi/mochi-core/tests` (nazoratchining hayot aylanishi testlari) va
`mochi/mochi-integration` masxara qilingan Torii stsenariylari uchun. To'plamlarni jo'natish yoki sim o'tkazish uchun
ish stolini CI quvurlariga o'tkazish uchun {doc}`mochi/packaging` qo'llanmasiga qarang.

## Mahalliy sinov eshigi

Yamalar yuborishdan oldin `ci/check_mochi.sh` ni ishga tushiring, shunda umumiy CI darvozasi uchta MOCHIni mashq qiladi.
qutilar:

```bash
./ci/check_mochi.sh
```

Yordamchi `mochi-core`, `mochi-ui-egui` uchun `cargo check`/`cargo test` va
`mochi-integration`, bu armatura drifti (kanonik blok/hodisalarni suratga olish) va egui jabduqlarini ushlaydi
bir zarbada regressiyalar. Agar skript eskirgan qurilmalar haqida xabar bersa, e'tibor berilmagan regeneratsiya sinovlarini qaytadan o'tkazing,
masalan:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

Qayta tiklashdan so'ng darvozani qayta ishga tushirish, siz bosishdan oldin yangilangan baytlarning barqaror bo'lishini ta'minlaydi.