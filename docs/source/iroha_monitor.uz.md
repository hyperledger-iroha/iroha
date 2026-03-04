---
lang: uz
direction: ltr
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2025-12-29T18:16:35.968850+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Monitor

Qayta tiklangan Iroha monitori engil terminal interfeysini animatsiya bilan birlashtiradi.
festivali ASCII san'ati va an'anaviy Etenraku mavzusi.  U ikkitaga e'tibor beradi
oddiy ish jarayonlari:

- **Spawn-lite rejimi** – tengdoshlarga taqlid qiluvchi vaqtinchalik holat/ko‘rsatkichlar stublarini ishga tushirish.
- **Birikish rejimi** – monitorni mavjud Torii HTTP so‘nggi nuqtalariga yo‘naltiring.

UI har bir yangilanishda uchta hududni ko'rsatadi:

1. **Torii osmon chizig‘i sarlavhasi** – animatsion torii darvozasi, Fudzi tog‘i, koi to‘lqinlari va yulduz
   yangilash kadensi bilan sinxronlashadigan maydon.
2. **Xulosa chizig‘i** – yig‘ilgan bloklar/tranzaksiyalar/gaz va yangilanish vaqti.
3. **Tengdoshlar stoli va festival shivirlari** – chap tomondagi tengdoshlar qatorlari, aylanadigan tadbir
   ogohlantirishlarni (vaqt tugashi, katta hajmdagi foydali yuklar va h.k.) ushlaydigan o'ngdagi tizimga kiring.
4. **Ixtiyoriy gaz trendi** – uchqun chizig‘ini qo‘shish uchun `--show-gas-trend` ni yoqing
   barcha tengdoshlar bo'yicha umumiy gazdan foydalanishni sarhisob qilish.

Ushbu refaktorda yangi:

- Koi, torii va chiroqlar bilan yapon uslubidagi animatsion ASCII sahnasi.
- Soddalashtirilgan buyruq yuzasi (`--spawn-lite`, `--attach`, `--interval`).
- Gagaku mavzusini ixtiyoriy audio tinglash bilan kirish banneri (tashqi MIDI
  platforma/audio stek qo'llab-quvvatlasa, pleer yoki o'rnatilgan yumshoq sintez).
- CI yoki tez tutun uchun `--no-theme` / `--no-audio` bayroqlari.
- Eng so'nggi ogohlantirish, bajarilgan vaqt yoki ish vaqti ko'rsatilgan "kayfiyat" ustuni.

## Tez boshlash

Monitorni yarating va uni o'ralgan tengdoshlarga qarshi ishlating:

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

Mavjud Torii so'nggi nuqtalariga biriktiring:

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

CI-do'st chaqiruv (kirish animatsiyasi va audioni o'tkazib yuborish):

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### CLI bayroqlari

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## Mavzuga kirish

Odatiy bo'lib, ishga tushirish Etenraku ball olganda qisqa ASCII animatsiyasini o'ynaydi
boshlanadi.  Audio tanlash tartibi:

1. Agar `--midi-player` taqdim etilsa, demo MIDI ni yarating (yoki `--midi-file` dan foydalaning)
   va buyruqni yarating.
2. Aks holda, macOS/Windows da (yoki `--features iroha_monitor/linux-builtin-synth` bilan Linux)
   O'rnatilgan gagaku yumshoq sintezi (tashqi audio yo'q) yordamida ballni yarating
   zarur aktivlar).
3. Agar audio o'chirilgan bo'lsa yoki ishga tushirish muvaffaqiyatsiz bo'lsa, kirish hali ham chop etadi
   animatsiya va darhol TUIga kiradi.

CPAL bilan ishlaydigan sinxronlash macOS va Windows-da avtomatik ravishda yoqiladi. Linuxda shunday
ish maydonini yaratishda ALSA/Pulse sarlavhalarini yo'qotmaslik uchun ro'yxatdan o'ting; uni yoqing
tizimingiz ta'minlasa, `--features iroha_monitor/linux-builtin-synth` bilan
ishlaydigan audio to'plami.

CI yoki boshsiz qobiqlarda ishlayotganda `--no-theme` yoki `--no-audio` dan foydalaning.

Yumshoq sintez endi *MIDI synth dizaynida tasvirlangan tartibga amal qiladi
Rust.pdf*: hichiriki va ryūteki shō paytida geterofonik ohangni baham ko'radi.
hujjatda tasvirlangan aitake pedlarini taqdim etadi.  Vaqtli eslatma ma'lumotlari amal qiladi
`etenraku.rs` da; u CPAL qayta qo'ng'iroqni va yaratilgan demo MIDIni quvvatlaydi.
Ovoz chiqishi mavjud bo'lmaganda, monitor ijroni o'tkazib yuboradi, lekin baribir ko'rsatadi
ASCII animatsiyasi.

## UI umumiy ko'rinishi- **Header art** – har bir freym `AsciiAnimator` tomonidan yaratilgan; koi, torii chiroqlari,
  va to'lqinlar uzluksiz harakatni berish uchun siljiydi.
- **Xulosa chizig'i** - onlayn tengdoshlar, hisobot qilingan tengdoshlar soni, bloklar jami,
  bo'sh bo'lmagan bloklar yig'indisi, tx tasdiqlash/rad qilish, gazdan foydalanish va yangilanish tezligi.
- **Tengdosh jadval** – taxallus/oxirgi nuqta, bloklar, tranzaktsiyalar, navbat hajmi uchun ustunlar,
  gazdan foydalanish, kechikish va "kayfiyat" haqida maslahat (ogohlantirishlar, bajarilish vaqti, ish vaqti).
- **Festival shivirlari** – ogohlantirishlar jurnali (ulanish xatolari, foydali yuk)
  chegara buzilishi, sekin so'nggi nuqtalar).  Xabarlar teskari (oxirgi tepada).

Klaviatura yorliqlari:

- `n` / O'ngga / Pastga - diqqatni keyingi tengdoshga o'tkazing.
- `p` / Chap / Yuqori - diqqatni oldingi tengdoshga o'tkazing.
- `q` / Esc / Ctrl-C - terminaldan chiqish va tiklash.

Monitor muqobil ekranli buferli krossterm + kalamushlardan foydalanadi; undan chiqishda
kursorni tiklaydi va ekranni tozalaydi.

## Tutun sinovlari

Kassa ikkala rejim va HTTP cheklovlarini qo'llaydigan integratsiya testlarini yuboradi:

- `spawn_lite_smoke_renders_frames`
- `attach_mode_with_stubs_runs_cleanly`
- `invalid_endpoint_surfaces_warning`
- `status_limit_warning_is_rendered`
- `attach_mode_with_slow_peer_renders_multiple_frames`

Faqat monitor sinovlarini bajaring:

```bash
cargo test -p iroha_monitor -- --nocapture
```

Ish joyida og'irroq integratsiya testlari mavjud (`cargo test --workspace`). Yugurish
monitor sinovlari alohida-alohida amalga oshirilganda ham tez tekshirish uchun foydalidir
to'liq to'plam kerak emas.

## Skrinshotlar yangilanmoqda

Hujjatlar namoyishi endi torii silueti va tengdoshlar jadvaliga qaratilgan.  Yangilash uchun
aktivlar, ishga tushirish:

```bash
make monitor-screenshots
```

Bu `scripts/iroha_monitor_demo.sh` ni o'radi (pawn-lite rejimi, sobit urug '/ko'rish oynasi,
intro/audio, shafaq palitrasi, art-speed 1, boshsiz qalpoq 24) va yozadi
SVG/ANSI ramkalari va `manifest.json` va `checksums.json`
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
ikkala CI himoyasini o'rab oladi (`ci/check_iroha_monitor_assets.sh` va
`ci/check_iroha_monitor_screenshots.sh`) shuning uchun generator xeshlari, manifest maydonlari,
va nazorat summalari sinxronlashtiriladi; skrinshot tekshiruvi ham sifatida yuboriladi
`python3 scripts/check_iroha_monitor_screenshots.py`. `--no-fallback` manziliga o'ting
demo skript, agar siz suratga olish o'rniga qaytish o'rniga muvaffaqiyatsiz bo'lishini istasangiz
monitor chiqishi bo'sh bo'lganda pishirilgan ramkalar; zaxira xom ashyodan foydalanilganda
`.ans` fayllari pishirilgan ramkalar bilan qayta yoziladi, shuning uchun manifest/nazorat summalari qoladi
deterministik.

## Deterministik skrinshotlar

Yuborilgan suratlar `docs/source/images/iroha_monitor_demo/` da jonli:

![Monitorning umumiy koʻrinishi](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![quvurni kuzatish](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

Ularni qattiq ko'rish maydoni/urug'i bilan takrorlang:

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

Suratga olish yordamchisi `LANG`/`LC_ALL`/`TERM`, oldinga siljiydi.
`IROHA_MONITOR_DEMO_SEED`, ovozni o'chiradi va badiiy mavzuni/tezlikni o'rnatadi, shuning uchun
ramkalar platformalar bo'ylab bir xil ko'rsatiladi. U `manifest.json` (generator
xeshlar + o'lchamlar) va `checksums.json` (SHA-256 dayjestlari) ostida
`docs/source/images/iroha_monitor_demo/`; CI ishlaydi
`ci/check_iroha_monitor_assets.sh` va `ci/check_iroha_monitor_screenshots.sh`
aktivlar qayd etilgan manifestlardan chetga chiqqanda muvaffaqiyatsizlikka uchraydi.

## Muammolarni bartaraf qilish; nosozliklarni TUZATISH- **Ovoz chiqishi yo‘q** – monitor o‘chirilgan ijroga qaytadi va davom etadi.
- **Headless backback erta tugaydi** – monitor boshsiz yugurishni juftlikka yopadi
  o'zgartira olmaganda o'nlab kadrlar (standart intervalda taxminan 12 soniya).
  terminalni xom rejimga o'tkazish; ishlashini ta'minlash uchun `--headless-max-frames 0` dan o'ting
  cheksiz muddatga.
- **O'lchamdagi yuklamalar** – tengdoshlarning kayfiyat ustuni va festival jurnali
  konfiguratsiya qilingan chegara (`128 KiB`) bilan `body exceeds …` ni ko'rsatish.
- **Slow peers** – hodisalar jurnali kutish tugashi haqidagi ogohlantirishlarni qayd qiladi; o'sha tengdoshga e'tibor qarating
  qatorni ajratib ko'rsatish.

Festival manzarasidan rohatlaning!  Qo'shimcha ASCII motivlari uchun hissalar yoki
ko'rsatkichlar panellari qabul qilinadi - ularni deterministik saqlang, shunda klasterlar bir xil ko'rsatiladi
terminaldan qat'i nazar, ramkaga.