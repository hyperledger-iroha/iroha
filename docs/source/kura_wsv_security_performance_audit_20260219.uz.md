<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / WSV Xavfsizlik va ishlash auditi (2026-02-19)

## Qo'llash doirasi

Ushbu audit quyidagilarni qamrab oldi:

- Kura qat'iyligi va byudjet yo'llari: `crates/iroha_core/src/kura.rs`
- Ishlab chiqarish WSV/davlat majburiyati/so'rov yo'llari: `crates/iroha_core/src/state.rs`
- IVM WSV soxta xost sirtlari (sinov/ishlab chiqish doirasi): `crates/ivm/src/mock_wsv.rs`

Qo'llash doirasi tashqarida: aloqador bo'lmagan qutilar va to'liq tizimli sinovlar.

## Xavf haqida xulosa

- Kritik: 0
- Yuqori: 4
- O'rta: 6
- Past: 2

## Topilmalar (Jiddatlilik bo'yicha tartiblangan)

### Yuqori

1. **Kura yozuvchisi kirish/chiqarishdagi nosozliklardan vahima qilmoqda (tugunning mavjudligi xavfi)**
- Komponent: Kura
- Turi: Xavfsizlik (DoS), Ishonchlilik
- Tafsilot: qayta tiklanadigan xatolarni qaytarish o'rniga qo'shish/indeks/fsync xatolarida yozuvchi tsikli vahima qo'yadi, shuning uchun vaqtinchalik diskdagi nosozliklar tugun jarayonini tugatishi mumkin.
- Dalillar:
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- Ta'sir: masofaviy yuk + mahalliy disk bosimi buzilish/qayta ishga tushirish davrlarini keltirib chiqarishi mumkin.2. **Kura koʻchirish `block_store` mutex ostida toʻliq maʼlumotlarni/indekslarni qayta yozadi**
- Komponent: Kura
- Turi: ishlash, mavjudlik
- Batafsil: `evict_block_bodies` `block_store` qulfini ushlab turganda vaqtinchalik fayllar orqali `blocks.data` va `blocks.index` ni qayta yozadi.
- Dalillar:
  - Qulfni olish: `crates/iroha_core/src/kura.rs:834`
  - To'liq qayta yozish tsikllari: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Atom almashtirish/sinxronlash: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Ta'sir: ko'chirish voqealari katta tarixlarda uzoq vaqt davomida yozish/o'qishni to'xtatib qo'yishi mumkin.

3. **Davlat majburiyati og'ir ishlar bo'yicha qo'pol `view_lock`ni o'z ichiga oladi**
- Komponent: ishlab chiqarish WSV
- Turi: ishlash, mavjudlik
- Tafsilotlar: blokirovka qilish eksklyuziv `view_lock` ni o'z ichiga oladi, shu bilan birga tranzaktsiyalar, blok xeshlari va dunyo holatini amalga oshiradi, bu esa og'ir bloklar ostida o'quvchilarning ochligini keltirib chiqaradi.
- Dalillar:
  - Qulfni ushlab turish boshlanadi: `crates/iroha_core/src/state.rs:17456`
  - Ichki qulfda ishlash: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Ta'sir: doimiy og'ir majburiyatlar so'rovlar/konsensusning javob berish qobiliyatini pasaytirishi mumkin.4. **IVM JSON administrator taxalluslari qo‘ng‘iroq qiluvchi tekshiruvisiz imtiyozli mutatsiyalarga ruxsat beradi (test/dev hosti)**
- Komponent: IVM WSV soxta xost
- Turi: Xavfsizlik (sinov/ishlab chiquvchi muhitda imtiyozlarni oshirish)
- Batafsil: JSON taxallus bilan ishlov beruvchilar to'g'ridan-to'g'ri rol/ruxsat/teng mutatsion usullariga yo'naltiriladi, ular qo'ng'iroq qiluvchining ruxsati tokenlarini talab qilmaydi.
- Dalillar:
  - Administrator taxalluslari: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Birlashtirilmagan mutatorlar: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Fayl hujjatlaridagi ko'lamli eslatma (sinov/ishlab chiqish maqsadi): `crates/ivm/src/mock_wsv.rs:295`
- Ta'sir: sinov shartnomalari/vositalari integratsiya jabduqlarida xavfsizlik bo'yicha taxminlarni o'z-o'zidan oshirishi va bekor qilishi mumkin.

### O'rta

5. **Kura byudjeti tekshiruvi kutilayotgan bloklarni har bir navbatda qayta kodlaydi (har bir yozish uchun O(n))**
- Komponent: Kura
- Turi: ishlash
- Tafsilot: har bir navbat kutilayotgan bloklarni takrorlash va har birini kanonik sim oʻlchami yoʻli orqali ketma-ketlashtirish orqali kutilayotgan navbat baytlarini qayta hisoblab chiqadi.
- Dalillar:
  - Navbatni skanerlash: `crates/iroha_core/src/kura.rs:2509`
  - Blok uchun kodlash yo'li: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Navbatda byudjet tekshiruvi chaqirildi: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Ta'sir: kechikishlar ostida o'tkazish qobiliyatining pasayishini yozing.6. **Kura byudjeti tekshiruvlari har bir navbatdagi blok-doʻkon metamaʼlumotlarini takroriy oʻqishni amalga oshiradi**
- Komponent: Kura
- Turi: ishlash
- Tafsilot: har bir tekshiruvda `block_store` qulflanganda barqaror indekslar soni va fayl uzunligi o'qiladi.
- Dalillar:
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- Ta'sir: issiq navbat yo'lida oldini olish mumkin bo'lgan kiritish/chiqarish/qulflash.

7. **Kurani olib tashlash navbatdagi byudjet yoʻlidan inline ishga tushirildi**
- Komponent: Kura
- Turi: ishlash, mavjudlik
- Tafsilot: navbatdagi yo'l yangi bloklarni qabul qilishdan oldin sinxron ravishda ko'chirishni chaqirishi mumkin.
- Dalillar:
  - Qo'ng'iroqlar zanjiri: `crates/iroha_core/src/kura.rs:2050`
  - Inline evakuatsiya chaqiruvi: `crates/iroha_core/src/kura.rs:2603`
- Ta'sir: byudjetga yaqin bo'lganda tranzaksiya/blokni qabul qilishda kechikish tezligi oshadi.

8. **`State::view` tortishuv ostida qo'pol qulfni olmasdan qaytishi mumkin**
- Komponent: ishlab chiqarish WSV
- Turi: Barqarorlik/Ishlash koeffitsienti
- Tafsilot: yozishni blokirovka qilish bo'yicha, `try_read` qayta ishlash dizayni bo'yicha qo'pol himoyasiz ko'rinishni qaytaradi.
- Dalillar:
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- Ta'sir: jonlilik yaxshilandi, ammo qo'ng'iroq qiluvchilar tortishuv ostida zaifroq o'zaro komponent atomligiga toqat qilishlari kerak.9. **`apply_without_execution` DA kursorini oshirishda qattiq `expect` dan foydalanadi**
- Komponent: ishlab chiqarish WSV
- Turi: Xavfsizlik (DoS orqali panic-on-invariant-break), Ishonchlilik
- Tafsilot: agar DA kursorini ilgari surish invariantlari muvaffaqiyatsiz bo'lsa, belgilangan blok yo'l panikasini qo'llaydi.
- Dalillar:
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- Ta'sir: yashirin tekshirish/indekslash xatolari tugunni o'ldirishda nosozliklarga aylanishi mumkin.

10. **IVM TLV publish syscall ajratilishidan oldin aniq konvert oʻlchamiga ega emas (test/ishlab chiquvchi xost)**
- Komponent: IVM WSV soxta xost
- Turi: Xavfsizlik (xotira DoS), Ishlash
- Tafsilot: sarlavha uzunligini o'qiydi, so'ngra ushbu yo'lda xost darajasidagi cheklovsiz to'liq TLV foydali yukini ajratadi/nusxalaydi.
- Dalillar:
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- Ta'sir: zararli sinov yuklari katta ajratishga majbur qilishi mumkin.

### Past

11. **Kura xabar berish kanali cheklanmagan (`std::sync::mpsc::channel`)**
- Komponent: Kura
- Turi: Ishlash/Xotira gigienasi
- Tafsilot: bildirishnoma kanali ishlab chiqaruvchining doimiy bosimi paytida ortiqcha uyg'onish hodisalarini to'plashi mumkin.
- Dalillar:
  - `crates/iroha_core/src/kura.rs:552`
- Ta'sir: xotira o'sishi xavfi har bir voqea hajmi uchun past, lekin undan qochish mumkin.12. **Quvur liniyasining navbatdagi navbati yozuvchi tugamaguncha xotirada cheklanmagan**
- Komponent: Kura
- Turi: Ishlash/Xotira gigienasi
- Tafsilot: yonbosh navbati `push_back`da aniq qopqoq/orqa bosim yo'q.
- Dalillar:
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- Ta'sir: yozuvchining uzoq muddatli kechikishlari paytida potentsial xotira o'sishi.

## Mavjud test qamrovi va bo'shliqlar

### Kura

- Mavjud qamrov:
  - saqlash byudjeti harakati: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - chiqarishning to'g'riligi va regidratatsiyasi: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Bo'shliqlar:
  - qo'shimchalar/indekslar/fsync nosozliklarini vahimasiz hal qilish uchun nosozliklarni in'ektsiya qoplamasi yo'q
  - katta kutilayotgan navbatlar va byudjetni tekshirish xarajatlari uchun ishlash regressiyasi testi o'tkazilmaydi
  - blokirovka bahsi ostida uzoq vaqtdan beri evakuatsiya kechikish sinovi o'tkazilmagan

### WSV ishlab chiqarish

- Mavjud qamrov:
  - tortishuvlarni qaytarish harakati: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - darajali orqa tomonda qulflash tartibi xavfsizligi: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Bo'shliqlar:
  - og'ir dunyo majburiyatlari ostida maksimal qabul qilinadigan majburiyatlarni ushlab turish muddatini tasdiqlovchi miqdoriy tortishuv testi yo'q
  - agar DA kursorini ilgari surish invariantlari kutilmaganda buzilsa, vahimasiz ishlash uchun regressiya testi yo'q.

### IVM WSV soxta xost- Mavjud qamrov:
  - JSON parser semantikasi va tengdoshlarni tahlil qilish uchun ruxsat (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - TLV dekodlash va JSON dekodlash atrofida tizimli tutun sinovlari (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Bo'shliqlar:
  - ruxsatsiz administrator taxallusni rad etish testlari yo'q
  - `INPUT_PUBLISH_TLV` da katta hajmli TLV konvertini rad etish testlari yo'q
  - nazorat punkti/klon qiymatini tiklash bo'yicha hech qanday benchmark/qo'riqxona sinovlari o'tkazilmaydi

## Ustuvor tuzatish rejasi

### 1-bosqich (Yuqori ta'sirli qattiqlashuv)

1. Kura yozuvchisi `panic!` shoxlarini qayta tiklanadigan xato tarqalishi + yomonlashgan sog'liq signali bilan almashtiring.
- Maqsadli fayllar: `crates/iroha_core/src/kura.rs`
- Qabul qilish:
  - kiritilgan append/index/fsync nosozliklari vahima qo'ymaydi
  - xatolar telemetriya/logging orqali yuzaga keladi va yozuvchi boshqariladi

2. IVM soxta-host TLV nashriyot va JSON konvert yoʻllari uchun chegaralangan konvert cheklarini qoʻshing.
- Maqsadli fayllar: `crates/ivm/src/mock_wsv.rs`
- Qabul qilish:
  - katta hajmdagi foydali yuklarni ajratish og'ir ishlov berishdan oldin rad etiladi
  - yangi testlar katta hajmli TLV va JSON holatlarini qamrab oladi

3. JSON administrator taxalluslari uchun aniq qo‘ng‘iroq qiluvchining ruxsati tekshiruvini o‘tkazing (yoki faqat sinov uchun mo‘ljallangan qattiq funksiya bayroqlari ortidagi darvoza taxalluslari va aniq hujjat).
- Maqsadli fayllar: `crates/ivm/src/mock_wsv.rs`
- Qabul qilish:
  - ruxsatsiz qo'ng'iroq qiluvchi rolni/ruxsatni/teng holatini taxalluslar orqali o'zgartira olmaydi

### 2-bosqich (Hot-path ishlashi)4. Kura byudjeti hisobini bosqichma-bosqich amalga oshirish.
- Navbatdagi toʻliq kutilayotgan navbatni qayta hisoblashni navbat/davom etish/tutishda yangilangan hisoblagichlar bilan almashtiring.
- Qabul qilish:
  - kutilayotgan baytlarni hisoblash uchun O(1) yaqinidagi navbat narxi
  - regressiya benchmarki kutilayotgan chuqurlik o'sishi bilan barqaror kechikishni ko'rsatadi

5. Ko'chirish qulfini ushlab turish vaqtini qisqartiring.
- Variantlar: segmentlangan siqish, qulfni ochish chegaralari bilan bo'laklangan nusxa yoki cheklangan oldingi blokirovka bilan fonga texnik xizmat ko'rsatish rejimi.
- Qabul qilish:
  - katta tarixli evakuatsiya kechikishi kamayadi va oldingi operatsiyalar javob beradi

6. Mumkin bo'lgan hollarda qo'pol `view_lock` muhim qismini qisqartiring.
- Eksklyuziv ushlab turish oynalarini minimallashtirish uchun bo'linish bosqichlarini yoki bosqichli deltalarni suratga olishni baholang.
- Qabul qilish:
  - tortishuv ko'rsatkichlari og'ir blokirovkalar ostida 99p ushlab turish vaqtini qisqartiradi

### 3-bosqich (operatsion to'siqlar)

7. Kura yozuvchisi va yon vagon navbatining orqa bosimi/qopqoqlari uchun chegaralangan/birlashtirilgan uyg'onish signalini kiriting.
8. Quyidagilar uchun telemetriya asboblar panelini kengaytiring:
- `view_lock` kutish/ushlash taqsimoti
- evakuatsiya davomiyligi va har bir ishga qaytarilgan baytlar
- byudjetni tekshirish navbatining kechikishi

## Tavsiya etilgan test qo'shimchalari1. `kura_writer_io_failures_do_not_panic` (birlik, nosozlik inyeksiyasi)
2. `kura_budget_check_scales_with_pending_depth` (ishlash regressiyasi)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (integratsiya/mukammal)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (qarama-qarshilik regressiyasi)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (chidamlilik)
6. `mock_wsv_admin_alias_requires_permissions` (xavfsizlik regressiyasi)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (DoS himoyasi)
8. `mock_wsv_checkpoint_restore_cost_regression` (mukammal mezon)

## Qamrov va ishonch haqida eslatmalar

- `crates/iroha_core/src/kura.rs` va `crates/iroha_core/src/state.rs` uchun topilmalar ishlab chiqarish yo'li topilmalaridir.
- `crates/ivm/src/mock_wsv.rs` uchun topilmalar fayl darajasidagi hujjatlar bo'yicha aniq sinov/dev xost doirasiga kiradi.
- Ushbu auditning o'zi ABI versiyasini o'zgartirishni talab qilmaydi.