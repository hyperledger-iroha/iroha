---
lang: uz
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito SwiftUI demo ishtirokchilari uchun qoʻllanma

Ushbu hujjat SwiftUI demo-ni a-ga qarshi ishlatish uchun zarur bo'lgan qo'lda sozlash bosqichlarini qamrab oladi
mahalliy Torii tugun va soxta kitob. Bu `docs/norito_bridge_release.md` ni to'ldiradi
kundalik rivojlanish vazifalariga e'tibor qaratish. Integratsiyani chuqurroq o'rganish uchun
Norito ko'prigi/Stekni Xcode loyihalariga ulash, qarang `docs/connect_swift_integration.md`.

## Atrof muhitni sozlash

1. `rust-toolchain.toml` da belgilangan Rust asboblar zanjirini o'rnating.
2. MacOS’da Swift 5.7+ va Xcode buyruq qatori vositalarini o‘rnating.
3. (Ixtiyoriy) Linting uchun [SwiftLint](https://github.com/realm/SwiftLint) ni o'rnating.
4. Xostingizda tugunni kompilyatsiya qilish uchun `cargo build -p irohad` ni ishga tushiring.
5. `examples/ios/NoritoDemoXcode/Configs/demo.env.example` dan `.env` ga nusxa oling va
   muhitingizga mos keladigan qadriyatlar. Ilova ishga tushganda ushbu o'zgaruvchilarni o'qiydi:
   - `TORII_NODE_URL` — asosiy REST URL (WebSocket URL manzillari undan olingan).
   - `CONNECT_SESSION_ID` — 32 baytlik seans identifikatori (base64/base64url).
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` - `/v2/connect/session` tomonidan qaytarilgan tokenlar.
   - `CONNECT_CHAIN_ID` — qoʻl siqish paytida eʼlon qilingan zanjir identifikatori.
   - `CONNECT_ROLE` - UIda oldindan tanlangan standart rol (`app` yoki `wallet`).
   - Qo'lda test qilish uchun qo'shimcha yordamchilar: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`.

## Bootstrapping Torii + soxta kitob

Repozitoriy xotira daftarchasi bilan Torii tugunini ishga tushiradigan yordamchi skriptlarni yuboradi.
demo hisoblar bilan yuklangan:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

Skript chiqaradi:

- Torii tugunlari `artifacts/torii.log` ga qayd qilinadi.
- Buxgalteriya ko'rsatkichlari (Prometheus formati) dan `artifacts/metrics.prom` gacha.
- `artifacts/torii.jwt` ga mijoz kirish tokenlari.

`start.sh` siz `Ctrl+C` tugmachasini bosmaguningizcha demo peer ishini davom ettiradi. U tayyor holatni yozadi
`artifacts/ios_demo_state.json` ga surat (boshqa artefaktlar uchun haqiqat manbai),
faol Torii stdout jurnalidan nusxa oladi, Prometheus qirilib ketguncha `/metrics` so'rovini oladi.
mavjud va sozlangan hisoblarni `torii.jwt` ga (jumladan, shaxsiy kalitlarga) aylantiradi
konfiguratsiya ularni taqdim etganda). Skript chiqishni bekor qilish uchun `--artifacts` ni qabul qiladi
maxsus Torii konfiguratsiyalariga mos keladigan `--telemetry-profile` katalogi va
Interaktiv bo'lmagan CI ishlari uchun `--exit-after-ready`.

`SampleAccounts.json` dagi har bir yozuv quyidagi maydonlarni qo'llab-quvvatlaydi:

- `name` (string, ixtiyoriy) — `alias` hisob metamaʼlumotlari sifatida saqlanadi.
- `public_key` (multihash string, talab qilinadi) — hisob imzolovchi sifatida ishlatiladi.
- `private_key` (ixtiyoriy) - mijoz hisob ma'lumotlarini yaratish uchun `torii.jwt` tarkibiga kiritilgan.
- `domain` (ixtiyoriy) - agar o'tkazib yuborilsa, aktiv domeniga sukut bo'yicha.
- `asset_id` (string, zarur) — hisob uchun aktiv taʼrifi.
- `initial_balance` (string, talab qilinadi) - hisob raqamiga kiritilgan raqamli miqdor.

## SwiftUI demosini ishga tushirish

1. `docs/norito_bridge_release.md` da tavsiflanganidek XCFramework ni yarating va uni birlashtiring
   demo loyihasiga (ma'lumotnomalar loyihada `NoritoBridge.xcframework` kutilmoqda
   ildiz).
2. Xcode'da `NoritoDemoXcode` loyihasini oching.
3. `NoritoDemo` sxemasini tanlang va iOS simulyatori yoki qurilmasini belgilang.
4. `.env` fayliga sxemaning muhit oʻzgaruvchilari orqali havola qilinganligiga ishonch hosil qiling.
   `/v2/connect/session` tomonidan eksport qilingan `CONNECT_*` qiymatlarini toʻldiring, shunda foydalanuvchi interfeysi boʻladi.
   ilova ishga tushirilganda oldindan to'ldirilgan.
5. Uskuna tezlashuvining standart sozlamalarini tekshiring: `App.swift` chaqiruvlari
   `DemoAccelerationConfig.load().apply()`, shuning uchun demo ikkalasini ham tanlaydi
   `NORITO_ACCEL_CONFIG_PATH` muhitni bekor qilish yoki to'plam
   `acceleration.{json,toml}`/`client.{json,toml}` fayli. Agar kerak bo'lsa, ushbu kirishlarni olib tashlang/sozlang
   ishga tushirishdan oldin protsessorni qayta tiklashga majburlamoqchi.
6. Ilovani yarating va ishga tushiring. Bosh ekran Torii URL/tokenini so'raydi, agar bo'lmasa
   allaqachon `.env` orqali o'rnatilgan.
7. Hisob yangilanishlariga obuna bo'lish yoki so'rovlarni tasdiqlash uchun "Ulanish" seansini boshlang.
8. IRH uzatishni yuboring va Torii jurnallari bilan birga ekrandagi jurnal chiqishini tekshiring.

### Uskuna tezlashuvi (Metal / NEON)

`DemoAccelerationConfig` Rust tugun konfiguratsiyasini aks ettiradi, shuning uchun ishlab chiquvchilar mashq qilishlari mumkin
Qattiq kodlash chegaralari bo'lmagan metall/NEON yo'llari. Yuklovchi quyidagilarni qidiradi
ishga tushirish joylari:

1. `NORITO_ACCEL_CONFIG_PATH` (`.env`/sxema argumentlarida belgilangan) — mutlaq yo‘l yoki
   `iroha_config` JSON/TOML fayliga `tilde` kengaytirilgan ko'rsatgich.
2. `acceleration.{json,toml}` yoki `client.{json,toml}` nomli birlashtirilgan konfiguratsiya fayllari.
3. Agar biron bir manba mavjud bo'lmasa, standart sozlamalar (`AccelerationSettings()`) qoladi.

Misol `acceleration.toml` parchasi:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

`nil` maydonlarini tark etish ish maydonining standart sozlamalarini meros qilib oladi. Salbiy raqamlar e'tiborga olinmaydi,
va etishmayotgan `[accel]` bo'limlari protsessorning deterministik xatti-harakatiga qaytadi. Ishlayotganda
Metall qo'llab-quvvatlamaydigan simulyator, ko'prik skaler yo'lni jimgina ushlab turadi, hatto bo'lsa ham
konfiguratsiya so'rovlari Metal.

## Integratsiya testlari

- Integratsiya testlari `Tests/NoritoDemoTests` da joylashgan (macOS CI o'rnatilgandan keyin qo'shiladi)
  mavjud).
- Yuqoridagi skriptlar yordamida testlar Torii ni aylantiradi va WebSocket obunalarini, tokenni ishlatadi.
  balanslar va Swift to'plami orqali o'tkazma oqimlari.
- Test sinovlari jurnallari `artifacts/tests/<timestamp>/` da ko'rsatkichlar va ko'rsatkichlar bilan birga saqlanadi.
  daftarning namunalari.

## CI paritetini tekshirish

- Demo yoki umumiy jihozlarga tegadigan PR yuborishdan oldin `make swift-ci` ni ishga tushiring. The
  target moslamalar paritetini tekshirishni amalga oshiradi, asboblar panelidagi tasmalarni tasdiqlaydi va ko'rsatadi
  mahalliy xulosalar. CIda bir xil ish jarayoni Buildkite metama'lumotlariga bog'liq
  (`ci/xcframework-smoke:<lane>:device_tag`) shuning uchun asboblar paneli natijalarni
  to'g'ri simulyator yoki StrongBox chizig'i - agar sozlashni o'rnatsangiz, metama'lumotlar mavjudligini tekshiring
  quvur liniyasi yoki agent teglari.
- `make swift-ci` bajarilmasa, `docs/source/swift_parity_triage.md` dagi amallarni bajaring.
  va qaysi qator talab qilinishini aniqlash uchun ko'rsatilgan `mobile_ci` chiqishini ko'rib chiqing.
  regeneratsiya yoki hodisani kuzatish.

## Nosozliklarni bartaraf etish

- Agar demo Torii ga ulana olmasa, tugun URL manzili va TLS sozlamalarini tekshiring.
- JWT tokeni (agar kerak bo'lsa) haqiqiy va muddati o'tmaganligiga ishonch hosil qiling.
- Server tomonidagi xatolar uchun `artifacts/torii.log` ni tekshiring.
- WebSocket muammolari uchun mijoz jurnali oynasini yoki Xcode konsolining chiqishini tekshiring.