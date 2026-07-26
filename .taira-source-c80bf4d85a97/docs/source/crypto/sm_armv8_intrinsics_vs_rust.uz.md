---
lang: uz
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2025-12-29T18:16:35.940026+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% ARMv8 SM3/SM4 Intrinsics va Pure Rust ilovalari
% Iroha Kripto ishchi guruhi
% 2026-02-12

# Tezkor

> Siz LLM Hyperledger Iroha kripto jamoasining ekspert maslahatchisi sifatida ishlaysiz.  
> Fon:  
> - Hyperledger Iroha - bu Rust-ga asoslangan ruxsat berilgan blokcheyn bo'lib, unda har bir validator deterministik tarzda ishlashi kerak, shuning uchun konsensus ajralib chiqmasligi kerak.  
> - Iroha Xitoyning GM/T kriptografik ibtidoiylaridan SM2 (imzolar), SM3 (xesh) va SM4 (blok shifr) maʼlum tartibga solishni joylashtirish uchun foydalanadi.  
> - Jamoa validator stekiga ikkita SM3/SM4 ilovasini yuboradi:  
> 1. Pure Rust, har qanday protsessorda ishlaydigan, bitli kesilgan, doimiy vaqtli skaler kod.  
> 2. ARMv8 NEON tezlashtirilgan yadrolari ixtiyoriy `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E`, Hyperledger va yangi koʻrsatmalarga tayanadi. Apple M seriyali va Arm server protsessorlari.  
> - `core::arch::aarch64` intrinsics yordamida ish vaqti funksiyasini aniqlashning orqasida tezlashtirilgan kod bor; Tizim iplar big.LITTLE yadrolari bo'ylab ko'chib o'tganda yoki replikalar turli kompilyator bayroqlari bilan qurilganda deterministik bo'lmagan xatti-harakatlardan qochishi kerak.  
> Talab qilingan tahlil:  
> Deterministik blokcheyn tekshiruvi uchun ARMv8 ichki ilovalarini sof Rust zaxiralari bilan solishtiring. Barcha validatorlarni sinxronlashtirish uchun zarur bo'lgan o'tkazuvchanlik/kechikish tezligini, determinizm tuzoqlarini (xususiyatlarni aniqlash, heterojen yadrolar, SIGILL xavfi, hizalama, bajarish yo'llarini aralashtirish), doimiy vaqt xususiyatlari va operatsion xavfsizlik choralarini muhokama qiling, hatto ba'zi apparatlar qo'llab-quvvatlamasa ham, barcha validatorlarni sinxronlashtirish uchun zarur.

# Xulosa

ARMv8-A qurilmalari ixtiyoriy `SM3` (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) va Hyperledger (Hyperledger, Hyperledger, Hyperledger) `SM4EKEY`) ko'rsatmalar to'plamlari GM/T xeshini tezlashtirishi va shifrlash primitivlarini sezilarli darajada bloklashi mumkin. Biroq, deterministik blokcheyn ijrosi xususiyatlarni aniqlash, qayta tiklanish pariteti va doimiy vaqt harakati ustidan qattiq nazoratni talab qiladi. Quyidagi yo'riqnomada ikkita amalga oshirish strategiyasi qanday taqqoslanishi va Iroha stekining bajarilishi kerak bo'lgan narsalarni qamrab oladi.

# Amalga oshirishni taqqoslash| Aspekt | ARMv8 Intrinsics (AAarch64 Inline ASM/`core::arch::aarch64`) | Pure Rust (bit-to'g'ralgan / jadvalsiz) |
|--------|-------------------------------------------------------------|------------------------------------------------|
| O'tkazish qobiliyati | Apple M-seriyali va Neoverse V1 qurilmalarida 3–5 marta tezroq SM3 xeshlash va har bir yadro uchun 8 barobar tezroq SM4 ECB/CTR; xotira bilan bog'langanda torayib boradi. | Skaler ALU bilan bog'langan va aylanadigan asosiy o'tkazuvchanlik; vaqti-vaqti bilan `aarch64` SHA kengaytmalaridan (kompilyatorning avtovektorizatsiyasi orqali) foyda ko'radi, lekin odatda NEONdan 3–8× bo'shliq bilan ortda qoladi. |
| Kechikish | Yagona blokli kechikish ~ 30-40ns M2 da ichki xususiyatlarga ega; qisqa xabarlarni xeshlash va tizim chaqiruvlarida kichik bloklarni shifrlashga mos keladi. | Har bir blok uchun 90-120ns; Raqobatbardosh bo'lib qolish, ko'rsatmalar keshi bosimini oshirish uchun ochishni talab qilishi mumkin. |
| Kod hajmi | Ikkita kodli yo'llarni (intrinsics + skaler) va ish vaqtini ajratishni talab qiladi; `cfg(target_feature)` ishlatilsa, ichki yo'l ixcham. | Yagona yo'l; qo'lda jadval jadvallari tufayli bir oz kattaroq, lekin gating mantig'i yo'q. |
| Determinizm | Deterministik natijaga erishish uchun ish vaqti jo‘natmasini bloklashi, o‘zaro bog‘liq funksiyalarni tekshirish poygalaridan qochish va agar heterojen yadrolar farq qilsa (masalan, big.LITTLE) protsessorga yaqinligini belgilash kerak. | Sukut bo'yicha deterministik; ish vaqti xususiyati aniqlanmaydi. |
| Doimiy holatda turish | Uskuna birligi asosiy turlar uchun doimiy vaqtdir, lekin o'ram orqaga yiqilganda yoki jadvallarni aralashtirishda yashirin tanlovdan qochish kerak. | Rustda to'liq nazorat qilinadi; to'g'ri kodlangan bo'lsa, qurilish (bit-slicing) bilan ta'minlangan doimiy vaqt. |
| Portativlik | `aarch64` + ixtiyoriy xususiyatlarni talab qiladi; x86_64 va RISC-V avtomatik ravishda orqaga tushadi. | Hamma joyda ishlaydi; ishlash kompilyatorni optimallashtirishga bog'liq. |

# Runtime Dispatch tuzoqlari

1. **Deterministik bo'lmagan xususiyatlarni tekshirish**
   - Muammo: `is_aarch64_feature_detected!("sm4")` ni heterojen katta. LITTLE SoC-larda tekshirish har bir yadroga turlicha javob berishi mumkin va o'zaro ish zarralarini o'g'irlash bir blok ichidagi yo'llarni aralashtirishi mumkin.
   - Yumshatish: tugunni ishga tushirish vaqtida apparat qobiliyatini aynan bir marta qo'lga kiriting, `OnceLock` orqali translyatsiya qiling va VM yoki kripto qutilari ichida tezlashtirilgan yadrolarni bajarayotganda CPU yaqinligi bilan bog'lang. Konsensus-tanqidiy ish boshlangandan keyin hech qachon xususiyat bayroqlari bo'yicha tarmoqlanmang.

2. **Replikalar bo‘yicha aralash aniqlik**
   - Muammo: turli kompilyatorlar bilan tuzilgan tugunlar ichki mavjudligi (`target_feature=+sm4` kompilyatsiya vaqtini yoqish va ish vaqtini aniqlash) bo'yicha kelishmasligi mumkin. Agar ijro turli xil kod yo'llari orqali o'tsa, mikro-arxitektura vaqtlari pow-ga asoslangan orqaga qaytish yoki tezlikni cheklovchilarga oqib chiqishi mumkin.
   - Yumshatish: aniq `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE` bilan kanonik tuzilish profillarini tarqating, deterministik qayta tartiblashni talab qiling (masalan, konfiguratsiya apparatni yoqmasa, skalerni afzal ko'ring) va sertifikatlash uchun manifestlarga konfiguratsiya xeshini qo'shing.3. ** Apple va Linuxda ko'rsatmalar mavjudligi**
   - Muammo: Apple SM4 ko'rsatmalarini faqat eng yangi kremniy va OS versiyalarida taqdim etadi; Linux distroslari eksport tasdiqlanishi kutilgunga qadar yadrolarni niqoblashi mumkin. Soqchisiz ichki narsalarga tayanish SIGILLsni keltirib chiqaradi.
   - Yumshatish: `std::arch::is_aarch64_feature_detected!` orqali eshik, tutun sinovlarida `SIGILL` ni ushlang va etishmayotgan ichki narsalarni kutilgan qaytarilish sifatida ko'rib chiqing (hali deterministik).

4. **Parallel qismlarga ajratish va xotirani tartibga solish**
   - Muammo: tezlashtirilgan yadrolar ko'pincha iteratsiyada bir nechta bloklarni qayta ishlaydi; NEON yuklari/do'konlari tekislanmagan kirishlar bilan ishlatilsa, Norito-seriyadan ajratilgan buferlar bilan oziqlanganda xatolik paydo bo'lishi yoki aniq tekislashni tuzatishni talab qilishi mumkin.
   - Yumshatish: bloklarga moslashtirilgan taqsimotlarni saqlang (masalan, `SM4_BLOCK_SIZE` `aligned_alloc` oʻramlari orqali koʻpaytiring), disk raskadrovka tuzilmalarida tekislashni tasdiqlang va notoʻgʻri moslashtirilganda skalyar holatga qayting.

5. **Ko'rsatmalar keshini zaharlash hujumlari**
   - Muammo: konsensus raqiblari zaif yadrolarda kichik I-kesh chiziqlarini buzadigan ish yuklarini yaratishi mumkin, bu tezlashtirilgan va skaler yo'llar o'rtasidagi kechikish farqini kengaytiradi.
   - Yumshatish: oldindan aytib bo'lmaydigan dallanishning oldini olish uchun deterministik bo'lak o'lchamlariga, pad halqalariga rejalashtirishni to'g'rilang va jitterning bardoshlik oynalarida qolishini ta'minlash uchun mikrobench regressiya testlarini o'z ichiga oladi.

# Deterministik joylashtirish bo'yicha tavsiyalar

- **Tuzish vaqti siyosati:** tezlashtirilgan kodni versiya tuzilmalarida sukut bo‘yicha yoqilgan xususiyat bayrog‘i (masalan, `sm_accel_neon`) orqasida saqlang, ammo paritet qamrovi etuk bo‘lgunga qadar test tarmoqlari uchun konfiguratsiyalarga aniq kirishni talab qiling.
- **Qayta paritet testlari:** tezlashtirilgan va skaler yo‘llarni orqaga qarab boshqaradigan oltin vektorlarni saqlab turish (joriy `sm_neon_check` ish jarayoni); provayder qo‘llab-quvvatlagandan so‘ng SM3/SM4 GCM rejimlarini qamrab olish uchun kengaytiring.
- **Manifest attestatsiyasi:** tengdoshlarni qabul qilishda farqlanishni aniqlash uchun tugunning Norito manifestiga tezlashtirish siyosatini (`hardware=sm-neon|scalar`) kiriting.
- **Telemetriya:** har ikki yo‘lda qo‘ng‘iroqning kechikish vaqtini taqqoslaydigan ko‘rsatkichlarni chiqaradi; Agar ajralish oldindan belgilangan chegaralardan oshib ketgan bo'lsa (masalan, >5% jitter) ogohlantirish, apparatning mumkin bo'lgan siljishi haqida signal.
- **Hujjatlar:** operator yo'riqnomasini (`sm_operator_rollout.md`) ichki ma'lumotlarni yoqish/o'chirish bo'yicha ko'rsatmalar bilan yangilab turing va yo'ldan qat'iy nazar deterministik xatti-harakatlar saqlanib qolishiga e'tibor bering.

# Ma'lumotnomalar

- `crates/iroha_crypto/src/sm.rs` - NEON va skalyar amalga oshirish ilgaklari.
- `.github/workflows/sm-neon-check.yml` - majburiy NEON CI chizig'i paritetni ta'minlaydi.
- `docs/source/crypto/sm_program.md` - to'siqlar va ishlash eshiklarini bo'shatish.
- Arm Architecture Reference Manual, Armv8-A, D13 bo'limi (SM3/SM4 ko'rsatmalari).
- GM/T 0002-2012, GM/T 0003-2012 - taqqoslash sinovlari uchun rasmiy SM3/SM4 spetsifikatsiyalari.

## Mustaqil soʻrov (nusxalash/qoʻyish)> Siz LLM Hyperledger Iroha kripto jamoasining ekspert maslahatchisi sifatida ishlaysiz.  
> Ma'lumot: Hyperledger Iroha - bu Rust-ga asoslangan ruxsat berilgan blokcheyn bo'lib, validatorlarda deterministik bajarilishini talab qiladi. Platforma Xitoyning GM/T SM2/SM3/SM4 kriptografiya to'plamini qo'llab-quvvatlaydi. SM3 va SM4 uchun kodlar bazasi ikkita dasturni jo'natadi: (a) hamma joyda ishlaydigan sof Rust bitli doimiy vaqt skalyar kodi va (b) `SM3PARTW1`, `SM3PARTW1`, `SM3PARTW1`, `aligned_alloc`, Hyperledger, Hyperledger, Hyperledger, Ixtiyoriy ko'rsatmalarga bog'liq bo'lgan ARMv8 NEON tezlashtirilgan yadrolari. `SM3SS2`, `SM4E` va `SM4EKEY`. Tezlashtirilgan yo'llar `core::arch::aarch64` yordamida ish vaqti xususiyatlarini aniqlash orqali yoqiladi; iplar heterojen big.LITTLE yadrolari bo'ylab ko'chib o'tganda yoki replikalar turli `target_feature` bayroqlari bilan qurilganda ular determinizmni kiritmasligi kerak.  
> Vazifa: deterministik blokcheyn tekshiruvi uchun ichki quvvatga ega ilovalarni skalyar zaxiralar bilan solishtiring. Tafsilotli o'tkazuvchanlik va kechikish farqlari, determinizm xavfini sanab o'ting (xususiyatlarni aniqlash, heterojen yadrolar, SIGILL xatti-harakati, hizalama, aralash ijro yo'llari), doimiy vaqt holatiga sharh bering va barcha sinashtiruvchi dasturlarning qattiqligini ta'minlaydigan himoya choralarini tavsiya eting (sinov strategiyasi, manifest/attestatsiya maydonlari, telemetriya, operator hujjatlari).