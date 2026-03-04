---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator GA Paritet hisoboti

Deterministik ko'p qabul qilish pariteti endi SDK uchun kuzatilmoqda, shuning uchun reliz muhandislari buni tasdiqlashlari mumkin
foydali yuk baytlari, bo'lak kvitansiyalari, provayder hisobotlari va reyting natijalari bir xil bo'lib qoladi
amalga oshirishlar. Har bir jabduq ostidagi kanonik ko'p provayderlar to'plamini iste'mol qiladi
SF1 rejasini paketlovchi `fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, provayder
metadata, telemetriya oniy tasviri va orkestr opsiyalari.

## Rust Baseline

- **Buyruq:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Qoʻl:** `MultiPeerFixture` rejasini jarayondagi orkestr orqali ikki marta ishga tushiradi va tasdiqlaydi
  yig'ilgan foydali yuk baytlari, chunk tushumlari, provayder hisobotlari va skorbord natijalari. Asboblar
  shuningdek, eng yuqori parallellik va samarali ishchi to'plam hajmini (`max_parallel × max_chunk_length`) kuzatib boradi.
- **Ishlash qo'riqchisi:** Har bir yugurish CI uskunasida 2 soniya ichida bajarilishi kerak.
- **Ishchi to'plam shipi:** SF1 profili bilan jabduqlar `max_parallel = 3` ni ta'minlaydi, natijada
  ≤196608bayt oyna.

Jurnal chiqishi namunasi:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK jabduqlari

- **Buyruq:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** `iroha_js_host::sorafsMultiFetchLocal` orqali foydali yuklarni solishtirgan holda bir xil moslamani takrorlaydi,
  tushumlar, provayder hisobotlari va ketma-ket yugurishlar bo'yicha skorbord snapshotlari.
- **Ijro qo'riqchisi:** Har bir ijro 2 soniya ichida yakunlanishi kerak; jabduqlar o'lchaganini chop etadi
  davomiylik va ajratilgan baytli shift (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Xulosa qatoriga misol:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK jabduqlari

- **Buyruq:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` da belgilangan paritet to‘plamini ishga tushiradi,
  SF1 moslamasini Norito ko'prigi (`sorafsLocalFetch`) orqali ikki marta takrorlash. Jabduqlar tasdiqlaydi
  bir xil deterministikdan foydalangan holda foydali yuk baytlari, bo'lak kvitantsiyalari, provayder hisobotlari va skorbord yozuvlari
  Rust/JS to'plamlari sifatida provayder metama'lumotlari va telemetriya oniy tasvirlari.
- **Ko'prik yuklagichi:** Jabduqlar talab va yuklarga qarab `dist/NoritoBridge.xcframework.zip` ni o'ramidan chiqaradi.
  `dlopen` orqali macOS tilim. Agar xcframework yo'qolsa yoki SoraFS ulanishlari bo'lmasa, u
  `cargo build -p connect_norito_bridge --release` ga qaytadi va unga qarshi bog'lanadi
  `target/release/libconnect_norito_bridge.dylib`, shuning uchun CIda qo'lda sozlash shart emas.
- **Ishlash qo'riqchisi:** Har bir ijro CI apparatida 2 soniya ichida yakunlanishi kerak; jabduqlar chop etadi
  o'lchangan davomiylik va ajratilgan baytli shift (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`).

Xulosa qatoriga misol:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python bog'lovchi jabduqlar

- **Buyruq:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- ** Qo'llanish sohasi:** Yuqori darajadagi `iroha_python.sorafs.multi_fetch_local` o'ramini va uning terishini mashq qiladi
  ma'lumotlar sinflari, shuning uchun kanonik moslama g'ildirak iste'molchilari chaqiradigan bir xil API orqali oqadi. Sinov
  `providers.json` dan provayder metamaʼlumotlarini qayta quradi, telemetriya suratini kiritadi va tekshiradi
  Rust/JS/Swift kabi foydali yuk baytlari, bo'lak kvitansiyalari, provayder hisobotlari va skorbord tarkibi
  suitlar.
- **Oldin talab:** `maturin develop --release` ni ishga tushiring (yoki g'ildirakni o'rnating), shunda `_crypto`
  pytestni chaqirishdan oldin `sorafs_multi_fetch_local` ulanishi; bog'lash paytida jabduqlar avtomatik ravishda o'tkazib yuboriladi
  mavjud emas.
- **Ishlash qo'riqchisi:** Rust to'plami bilan bir xil ≤2s byudjet; pytest yig'ilgan bayt sonini qayd qiladi
  va reliz artefakt uchun provayder ishtiroki sarhisobi.

Bo'shatish shlyuzi har bir jabduqdan (Rust, Python, JS, Swift) xulosa chiqarishi kerak, shuning uchun
Arxivlangan hisobot qurilishni targ'ib qilishdan oldin foydali yuk tushumlari va ko'rsatkichlarni bir xilda farq qilishi mumkin. Yugurish
`ci/sdk_sorafs_orchestrator.sh` har bir paritet to'plamini (Rust, Python ulanishlari, JS, Swift) bajarish uchun.
bitta o'tish; CI artefaktlari o'sha yordamchidan olingan jurnaldan ko'chirma va hosil qilinganini biriktirishi kerak
`matrix.md` (SDK/status/davomiylik jadvali) reliz chiptasi, shunda sharhlovchilar paritetni tekshirishlari mumkin
to'plamni mahalliy sifatida qayta ishga tushirmasdan matritsa.