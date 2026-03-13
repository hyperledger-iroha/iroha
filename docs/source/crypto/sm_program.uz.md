---
lang: uz
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T23:46:10.134857+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Hyperledger Iroha v2 uchun SM2/SM3/SM4 faollashtirish arxitekturasi qisqartmasi.

# SM dasturi arxitekturasi haqida qisqacha ma'lumot

## Maqsad
Iroha v2 stekiga Xitoy milliy kriptografiyasini (SM2/SM3/SM4) joriy etish uchun texnik reja, ta'minot zanjiri holati va xavf chegaralarini aniqlang, shu bilan birga deterministik bajarilishi va auditorlik qobiliyatini saqlang.

## Qo'llash doirasi
- **Konsensus-tanqidiy yo'llar:** `iroha_crypto`, `iroha`, `irohad`, IVM xost, Kotodama intrinsics.
- **Mijoz SDK’lari va asboblari:** Rust CLI, Kagami, Python/JS/Swift SDK’lar, genezis yordamchi dasturlari.
- **Konfiguratsiya va serializatsiya:** `iroha_config` tugmalari, Norito maʼlumotlar modeli teglari, manifest bilan ishlash, multikodek yangilanishlari.
- **Sinov va muvofiqlik:** Birlik/mulk/operatsion to'plamlar, Wycheproof jabduqlar, ishlash profili, eksport/tartibga solish yo'riqnomasi. *(Holat: RustCrypto tomonidan qoʻllab-quvvatlangan SM stek birlashtirildi; kengaytirilgan CI uchun ixtiyoriy `sm_proptest` fuzz toʻplami va OpenSSL paritet jabduqlari mavjud.)*

Qo'llash doirasi tashqarisida: PQ algoritmlari, konsensus yo'llarida deterministik bo'lmagan xost tezlashishi; wasm/`no_std` tuzilmalari nafaqaga chiqqan.

## Algoritm kiritish va yetkazib berish
| Artefakt | Egasi | Muddati | Eslatmalar |
|----------|-------|-----|-------|
| SM algoritmi xususiyati dizayni (`SM-P0`) | Crypto WG | 2025-02 | Xususiyatlarni aniqlash, qaramlik auditi, xavf registri. |
| Core Rust integratsiyasi (`SM-P1`) | Crypto WG / Ma'lumotlar modeli | 2025-03 | RustCrypto-ga asoslangan verify/hash/AEAD yordamchilari, Norito kengaytmalari, armatura. |
| Imzolash + VM tizimlari (`SM-P2`) | IVM yadro / SDK dasturi | 2025-04 | Deterministik imzolash paketlari, tizimlar, Kotodama qamrovi. |
| Ixtiyoriy provayder va operatsiyalarni yoqish (`SM-P3`) | Platforma operatsiyalari / Ishlash WG | 2025-06 | OpenSSL/Tongsuo backend, ARM intrinsics, telemetriya, hujjatlar. |

## Tanlangan kutubxonalar
- **Birlamchi:** RustCrypto qutilari (`sm2`, `sm3`, `sm4`) `rfc6979` funksiyasi yoqilgan va SM3 deterministik bo'lmagan holatlarga bog'langan.
- **Ixtiyoriy FFI:** OpenSSL 3.x provayder API yoki sertifikatlangan steklar yoki apparat dvigatellarini talab qiladigan joylashtirishlar uchun Tongsuo; konsensus ikkiliklarida sukut bo'yicha xususiyatga ega va o'chirilgan.### Asosiy kutubxona integratsiyasi holati
- `iroha_crypto::sm` birlashtirilgan `sm` funksiyasi ostida SM3 xeshlash, SM2 tekshiruvi va SM4 GCM/CCM yordamchilarini ochib beradi, SDKlar uchun deterministik RFC6979 imzolash yo'llari orqali `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON teglari va multikodek yordamchilari SM2 ochiq kalitlari/imzolari va SM3/SM4 foydali yuklarini qamrab oladi, shuning uchun ko'rsatmalar aniq ketma-ketlikda ketma-ketlashadi. xostlar.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- Ma'lum javoblar to'plamlari RustCrypto integratsiyasini (`sm3_sm4_vectors.rs`, `sm2_negative_vectors.rs`) tasdiqlaydi va CI ning `sm` funksiya ishlarining bir qismi sifatida ishlaydi, tugunlar imzolashda davom etayotganda tekshirishni deterministik saqlaydi. Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- `sm` ixtiyoriy tuzilish tekshiruvi: `cargo check -p iroha_crypto --features sm --locked` (sovuq 7,9s / issiq 0,23s) va `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1,0s) ikkalasi ham muvaffaqiyatli; funksiya yoqilsa, 11 quti qo‘shiladi (`base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`0, `polyval`0, Hyperledger, `sm2`, `sm3`, `sm4`, `sm4-gcm`). Topilmalar `docs/source/crypto/sm_rustcrypto_spike.md` da qayd etilgan.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL manfiy tekshirish moslamalari `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` ostida ishlaydi, bu esa kanonik nosozlik holatlarini (r=0, s=0, ajratuvchi identifikator nomuvofiqligi, o'zgartirilgan ochiq kalit) keng qo'llaniladigan qurilmalar bilan mos kelishini ta'minlaydi. provayderlar.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` endi sotuvchi OpenSSL 3.x asboblar zanjirini (`openssl` sandiq `vendored` xususiyati) kompilyatsiya qiladi, shuning uchun tuzilmalarni oldindan ko'rish va sinovlar har doim LibreSSL/OpenSSL tizimi yo'q bo'lganda ham zamonaviy SM-qobiliyatli provayderga qaratilgan. algoritmlar.【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` endi ish vaqtida AArch64 NEONni aniqlaydi va SM3/SM4 ilgaklarini x86_64/RISC-V jo'natmasi orqali o'tkazadi va `crypto.sm_intrinsics` konfiguratsiya tugmasiga amal qiladi. (`auto`/`force-enable`/`force-disable`). Vektor orqa uchlari bo'lmaganda dispetcher skalyar RustCrypto yo'li bo'ylab yo'nalishni davom ettiradi, shuning uchun dastgohlar va siyosat almashuvlari xostlar bo'ylab barqaror ishlaydi.【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:73】

### Norito Sxema va ma'lumotlar modeli sirtlari| Norito turi / iste'molchi | Vakillik | Cheklovlar va eslatmalar |
|---------------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | Yalang'och: 32-bayt blob · JSON: katta hex string (`"4F4D..."`) | Kanonik Norito kortejli o'rash `[u8; 32]`. JSON/Yalang'och dekodlash ≠32 uzunliklarni rad etadi. `sm_norito_roundtrip::sm3_digest_norito_roundtrip` tomonidan ikki tomonlama sayohatlar qamrab olinadi. |
| `Sm2PublicKey` / `Sm2Signature` | Multicodec prefiksli bloblar (`0x1306` vaqtinchalik) | Ochiq kalitlar siqilmagan SEC1 nuqtalarini kodlaydi; imzolar `(r∥s)` (har biri 32 bayt), DER tahlil qiluvchi himoyasi bilan. |
| `Sm4Key` | Yalang'och: 16 baytlik blob | Kotodama/CLI taʼsiri ostida oʻramni nolga solish. JSON serializatsiyasi ataylab o'tkazib yuborilgan; operatorlar bloklar (kontraktlar) yoki CLI hex (`--key-hex`) orqali kalitlarni o'tkazishlari kerak. |
| `sm4_gcm_seal/open` operandlari | 4 ta bloblar majmuasi: `(key, nonce, aad, payload)` | Kalit = 16 bayt; nonce = 12 bayt; teg uzunligi 16 baytda belgilangan. `(ciphertext, tag)` qaytaradi; Kotodama/CLI ham hex, ham base64 yordamchilarini chiqaradi.【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` operandlari | `r14` da 4 ta blok (kalit, bo'lmagan, aad, foydali yuk) + teg uzunligi | 7–13 bayt emas; teg uzunligi ∈ {4,6,8,10,12,14,16}. `sm` xususiyati `sm-ccm` bayrog'i orqasida CCMni ko'rsatadi. |
| Kotodama ichki (`sm::hash`, `sm::seal_gcm`, `sm::open_gcm`, …) | Yuqoridagi SCALLs xaritasi | Kirish tekshiruvi xost qoidalarini aks ettiradi; noto'g'ri shakllangan o'lchamlar `ExecutionError::Type` ni oshiradi. |

Foydalanish uchun tezkor ma'lumotnoma:
- **Shartnomalar/sinovlarda SM3 xeshing:** `Sm3Digest::hash(b"...")` (Rust) yoki Kotodama `sm::hash(input_blob)`. JSON 64 hex belgilarni kutadi.
- **CLI orqali SM4 AEAD:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` hex/base64 shifrlangan matn+teg juftlarini beradi. Mos `gcm-open` bilan shifrni hal qiling.
- **Multicodec strings:** SM2 ochiq kalitlari/imzolari `PublicKey::from_str`/`Signature::from_bytes` tomonidan qabul qilingan ko‘p bazali qatordan/ko‘p bazaga ajraladi, bu esa Norito manifestlari va hisob identifikatorlarini SM imzolovchilarini olib yurish imkonini beradi.

Ma'lumotlar modeli iste'molchilari SM4 kalitlari va teglarini vaqtinchalik bloklar sifatida ko'rishlari kerak; hech qachon zanjirda xom kalitlarni saqlamang. Shartnomalar faqat shifrlangan matn/teg chiqishi yoki olingan dayjestlarni (masalan, kalitning SM3) tekshirish zarur bo'lganda saqlashi kerak.

### Ta'minot zanjiri va litsenziyalash
| Komponent | Litsenziya | Yumshatish |
|-----------|---------|-----------|
| `sm2`, `sm3`, `sm4` | Apache-2.0 / MIT | Yuqori oqimlarni kuzatib boring, agar blokirovkali relizlar kerak bo'lsa, sotuvchi, validator GA imzolashdan oldin uchinchi tomon auditini rejalashtiring. |
| `rfc6979` | Apache-2.0 / MIT | Boshqa algoritmlarda allaqachon ishlatilgan; SM3 digest bilan `k` deterministik bog'lanishini tasdiqlang. |
| Ixtiyoriy OpenSSL/Tongsuo | Apache-2.0 / BSD uslubi | `sm-ffi-openssl` xususiyatidan orqada qoling, operatorning aniq ro'yxatini va qadoqlash nazorat ro'yxatini talab qiling. |### Bayroqlar va egalik xususiyati
| Yuzaki | Standart | Ta'minotchi | Eslatmalar |
|---------|---------|------------|-------|
| `iroha_crypto/sm-core`, `sm-ccm`, `sm` | O'chirilgan | Crypto WG | RustCrypto SM primitivlarini yoqadi; `sm` autentifikatsiya qilingan shifrlashni talab qiladigan mijozlar uchun CCM yordamchilarini to'playdi. |
| `ivm/sm` | O'chirilgan | IVM Asosiy jamoa | SM tizim qoʻngʻiroqlarini quradi (`sm3_hash`, `sm2_verify`, `sm4_gcm_*`, `sm4_ccm_*`). Xost darvozasi `crypto.allowed_signing` (`sm2` mavjudligi) dan kelib chiqadi. |
| `iroha_crypto/sm_proptest` | O'chirilgan | QA / Crypto WG | Noto'g'ri shakllangan imzo/teglarni qamrab oluvchi xususiyat-test jabduqlar. Faqat kengaytirilgan CIda yoqilgan. |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | Config WG / Operators WG | `sm2` va `sm3-256` xesh mavjudligi SM tizimi qoʻngʻiroqlari/imzolarini beradi; `sm2`ni olib tashlash faqat tekshirish rejimiga qaytadi. |
| Ixtiyoriy `sm-ffi-openssl` (oldindan ko'rish) | O'chirilgan | Platforma operatsiyalari | OpenSSL/Tongsuo provayderi integratsiyasi uchun joy ushlagich xususiyati; sertifikatlash va qadoqlash SOPlari qo'nguncha o'chirilgan bo'lib qoladi. |

Tarmoq siyosati endi `network.require_sm_handshake_match` va
`network.require_sm_openssl_preview_match` (ikkalasi ham standart `true`). Ikkala bayroqni tozalashga ruxsat beriladi
faqat Ed25519 kuzatuvchilari SM yoqilgan validatorlarga ulanadigan aralash joylashtirishlar; nomuvofiqliklar mavjud
`WARN` da tizimga kirgan, ammo konsensus tugunlari tasodifiy holatlarning oldini olish uchun standart sozlamalarni yoqilgan holda saqlashi kerak.
SM-dan xabardor va SM-nogiron tengdoshlari o'rtasidagi tafovut.
CLI bu almashishlarni “iroha_cli” ilovasi orqali qo‘l siqish yangilanishi orqali amalga oshiradi
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-oldindan ko'rish-mismatch`, or the matching `--talab qilish-*`
qat'iy ijroni tiklash uchun bayroqlar.#### OpenSSL/Tongsuo oldindan ko'rish (`sm-ffi-openssl`)
- **Scope.** OpenSSL ish vaqti mavjudligini tasdiqlovchi va OpenSSL tomonidan qo‘llab-quvvatlangan SM3 xeshing, SM2 tekshiruvi va SM4-GCM shifrlash/shifrini yechish imkonini beruvchi faqat oldindan ko‘rish uchun mo‘ljallangan provayder (`OpenSslProvider`) tuzadi. Konsensus ikkiliklari RustCrypto yo'lidan foydalanishni davom ettirishi kerak; FFI backend chekka tekshirish/imzolovchi uchuvchilar uchun qat'iy ravishda tanlanadi.
- **Shartlarni yarating.** `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` bilan kompilyatsiya qiling va OpenSSL/Tongsuo 3.0+ (SM2/SM3/SM4 qo'llab-quvvatlangan `libcrypto`) ga qarshi asboblar zanjiri havolalarini ta'minlang. Statik ulanish tavsiya etilmaydi; operator tomonidan boshqariladigan dinamik kutubxonalarni afzal ko'ring.
- **Ishlab chiquvchi tutun testi.** `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` va keyin `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture` ni bajarish uchun `scripts/sm_openssl_smoke.sh` ni ishga tushiring; OpenSSL ≥3 ishlab chiqish sarlavhalari mavjud bo'lmaganda (yoki `pkg-config` yo'q bo'lsa) yordamchi avtomatik ravishda o'tkazib yuboradi va ishlab chiquvchilar SM2 tekshiruvi Rust ilovasiga qaytganmi yoki yo'qligini ko'rishlari uchun tutun chiqadi.
- **Rust iskala.** `openssl_sm` moduli endi SM3 xeshlash, SM2 tekshiruvi (ZA prehash + SM2 ECDSA) va SM4 GCM shifrlash/shifrini ochishni OpenSSL orqali oldindan ko'rish tugmalari va noto'g'ri kalit/nonce/teg uzunligini qamrab olgan tizimli xatolar bilan yo'naltiradi; SM4 CCM qo'shimcha FFI shimlari qo'nguncha faqat sof Russt bo'lib qoladi.
- **O‘tkazib yuborish xatti-harakati.** OpenSSL ≥3.0 sarlavhalari yoki kutubxonalari bo‘lmasa, tutun testi o‘tkazib yuborish bannerini chop etadi (`-- --nocapture` orqali), lekin baribir muvaffaqiyatli chiqadi, shuning uchun CI atrof-muhit bo‘shliqlarini haqiqiy regressiyalardan ajrata oladi.
- **Runtime to'siqlari.** OpenSSL oldindan ko'rish sukut bo'yicha o'chirilgan; FFI yo'lidan foydalanishga urinishdan oldin uni konfiguratsiya (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) orqali yoqing. Provayder tugaguniga qadar ishlab chiqarish klasterlarini faqat tekshirish rejimida saqlang (`sm2` ni `allowed_signing` dan chiqarib tashlang), RustCrypto deterministik zaxirasiga tayaning va imzo chekuvchi uchuvchilarni alohida muhitlarga cheklab qo'ying.
- **Qadoqlash tekshiruvi roʻyxati.** Oʻrnatish manifestlarida provayder versiyasi, oʻrnatish yoʻli va yaxlitlik xeshlarini hujjatlashtiring. Operatorlar tasdiqlangan OpenSSL/Tongsuo tuzilmasini o'rnatadigan o'rnatish skriptlarini taqdim etishi, uni OS ishonchli do'konida ro'yxatdan o'tkazishi (agar kerak bo'lsa) va yangilanishlarni texnik xizmat ko'rsatish oynalari orqasiga o'rnatishi kerak.
- **Keyingi qadamlar.** Kelajakdagi bosqichlarga deterministik SM4 CCM FFI ulanishlari, CI tutun ishlari (qarang: `ci/check_sm_openssl_stub.sh`) va telemetriya. `roadmap.md` da SM-P3.1.x ostida taraqqiyotni kuzatib boring.

#### Kodga egalik surati
- **Kripto WG:** `iroha_crypto`, SM moslamalari, muvofiqlik hujjatlari.
- **IVM Yadro:** tizimli qo'llanmalar, Kotodama intrinsics, xost darvozasi.
- **Konfiguratsiya WG:** Kunpgururi `crypto.allowed_signing`/`default_hash`, lidj manipps, bluv.
- **SDK dasturi:** CLI/Kagami/SDK-larda SM-dan xabardor asboblar, umumiy jihozlar.
- **Platforma Ops & Performance WG:** tezlashtirish ilgaklari, telemetriya, operatorni yoqish.

## Konfiguratsiya migratsiya o'yin kitobiOperatorlar faqat Ed25519 tarmoqlaridan SM-ni yoqadigan joylashtirishga o'tishlari kerak
bosqichli jarayonni kuzatib boring
[`sm_config_migration.md`](sm_config_migration.md). Qo'llanma qurilishni o'z ichiga oladi
tasdiqlash, `iroha_config` qatlamlash (`defaults` → `user` → `actual`), genezis
`kagami` bekor qilish (masalan, `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`) orqali regeneratsiya, parvozdan oldin tekshirish va orqaga qaytarish
rejalashtirish, shuning uchun konfiguratsiya suratlari va manifestlari butun dunyo bo'ylab izchil bo'lib qoladi
flot.

## Deterministik siyosat
- SDK-lardagi barcha SM2 imzolash yo'llari va ixtiyoriy xost imzolash uchun RFC6979-dan kelib chiqadigan noncesni qo'llash; tekshirgichlar faqat kanonik r∥s kodlashni qabul qiladi.
- Boshqaruv-tekislik aloqasi (oqim) Ed25519 qoladi; Agar boshqaruv kengayishni ma'qullamasa, SM2 ma'lumotlar tekisligi imzolari bilan cheklangan.
- Intrinsics (ARM SM3/SM4) deterministik tekshirish/xesh operatsiyalari bilan chegaralangan, ish vaqti xususiyatlarini aniqlash va dasturiy ta'minotni qayta tiklash.

## Norito va kodlash rejasi
1. `iroha_data_model` da algoritm raqamlarini `Sm2PublicKey`, `Sm2Signature`, `Sm3Digest`, `Sm4Key` bilan kengaytiring.
2. DER noaniqliklariga yo'l qo'ymaslik uchun SM2 imzolarini katta-endian qattiq kenglikdagi `r∥s` massivlari (32+32 bayt) sifatida seriyalashtiring; konvertatsiyalar adapterlarda amalga oshiriladi. *(Bajarildi: `Sm2Signature` yordamchilarida amalga oshirildi; Norito/JSON boʻylab sayohatlar joyida.)*
3. Agar multiformatlardan foydalansangiz, multikodek identifikatorlarini (`sm3-256`, `sm2-pub`, `sm4-key`) ro'yxatdan o'tkazing, moslamalar va hujjatlarni yangilang. *(Ravj: `sm2-pub` vaqtinchalik kodi `0x1306` endi olingan kalitlar bilan tasdiqlangan; SM3/SM4 kodlari yakuniy tayinlanishni kutmoqda, `sm_known_answers.toml` orqali kuzatiladi.)*
4. Yangilangan Norito oltin testlari aylanma sayohatlar va noto'g'ri shakllangan kodlashlarni rad etish (qisqa/uzun r yoki s, noto'g'ri egri parametrlari).## Xost va VM integratsiya rejasi (SM-2)
1. Mavjud GOST xesh shimini aks ettiruvchi xost tomoni `sm3_hash` tizim chaqiruvini amalga oshirish; `Sm3Digest::hash` dan qayta foydalaning va deterministik xato yo'llarini oching. *(Qo'ndi: xost Blob TLVni qaytaradi; `DefaultHost` amalga oshirish va `sm_syscalls.rs` regressiyasiga qarang.)*
2. Kanonik r∥ imzolarini qabul qiluvchi, farqlovchi identifikatorlarni tasdiqlovchi va deterministik qaytish kodlaridagi nosozliklarni xaritalashtirgan `sm2_verify` bilan VM tizimi jadvalini kengaytiring. *(Bajarildi: xost + Kotodama ichki maʼlumotlarini qaytarish `1/0`; regressiya toʻplami endi kesilgan imzolar, notoʻgʻri shakllangan ochiq kalitlar, bloklanmagan TLVlar va UTF-8/boʻsh/mos kelmaydigan Hyperledger foydali yuklarni qamrab oladi*)
3. `sm4_gcm_seal`/`sm4_gcm_open` (va ixtiyoriy ravishda CCM) tizimli qoʻngʻiroqlarini aniq boʻlmagan/teg oʻlchami (RFC 8998) bilan taʼminlang. *(Bajarildi: GCM qattiq 12 baytlik nonslar + 16 bayt teglardan foydalanadi; CCM `r14` orqali boshqariladigan teg uzunligi {4,6,8,10,12,14,16} boʻlgan 7–13 baytlik nonslarni qoʻllab-quvvatlaydi; Kotodama va Kotodama va I08018 kabilar ochiq `sm::seal_ccm/open_ccm`.) Qayta ishlatmaslik siyosatini ishlab chiquvchi qoʻllanmasida hujjatlashtiring.*
4. Kotodama simli tutun kontraktlari va ijobiy va salbiy holatlarni (o'zgartirilgan teglar, noto'g'ri tuzilgan imzolar, qo'llab-quvvatlanmaydigan algoritmlar) qamrab oluvchi IVM integratsiya testlari. *(SM3/SM2/SM4 uchun xost regressiyalarini aks ettirish `crates/ivm/tests/kotodama_sm_syscalls.rs` orqali amalga oshirildi.)*
5. Syscall ruxsat etilgan roʻyxatlar, siyosatlar va ABI hujjatlarini (`crates/ivm/docs/syscalls.md`) yangilang va yangi yozuvlarni qoʻshgandan soʻng xeshlangan manifestlarni yangilang.

### Xost va VM integratsiyasi holati
- DefaultHost, CoreHost va WsvHost SM3/SM2/SM4 tizim qo'ng'iroqlarini ochib beradi va ularni `sm_enabled` da o'rnatadi, ish vaqti bayrog'i bo'lsa, `PermissionDenied` ni qaytaradi. noto'g'ri.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` shlyuzlari quvur liniyasi/ijrochi/davlat orqali o'tkaziladi, shuning uchun ishlab chiqarish tugunlari konfiguratsiya orqali aniq tanlanadi; `sm2` qoʻshilishi SM yordamchisi mavjudligini oʻchirib qoʻyadi.`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iro3】【crates/iro.
- SM3 xeshing, SM2 tekshiruvi va SM4 GCM/CCM muhri/ochiq uchun yoqilgan va o'chirilgan yo'llarni (DefaultHost/CoreHost/WsvHost) regressiya qamrovi mashqlari. oqimlari.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## Konfiguratsiya mavzulari
- `crypto.allowed_signing`, `crypto.default_hash`, `crypto.sm2_distid_default` va ixtiyoriy `crypto.enable_sm_openssl_preview` ni `iroha_config` ga qo'shing. Santexnika ma'lumotlar modeli xususiyati kripto qutisini aks ettirayotganiga ishonch hosil qiling (`iroha_data_model` `sm` → `iroha_crypto/sm` ni ko'rsatadi).
- Manifest/genesis fayllari ruxsat etilgan algoritmlarni aniqlashi uchun qabul qilish qoidalariga sim konfiguratsiyasi; boshqaruv tekisligi sukut bo'yicha Ed25519 bo'lib qoladi.### CLI va SDK ishi (SM-3)
1. **Torii CLI** (`crates/iroha_cli`): SM2 keygen/import/eksport (ajralmas xabardor), SM3 xeshing yordamchilari va SM4 AEAD shifrlash/shifrini yechish buyruqlarini qo‘shing. Interaktiv takliflar va hujjatlarni yangilang.
2. **Genesis asboblari** (`xtask`, `scripts/`): manifestlarga ruxsat etilgan imzolash algoritmlari va standart xeshlarni e'lon qilishga ruxsat bering, agar SM mos keladigan konfiguratsiya tugmalarisiz yoqilgan bo'lsa, tezda ishlamay qoladi. *(Bajarildi: `RawGenesisTransaction` endi `crypto` blokiga ega, `default_hash`/`allowed_signing`/`sm2_distid_default`; `ManifestCrypto::validate` va I10222X va I18NIX va reject sozlamalari defaults/genesis manifest suratni reklama qiladi.)*
3. **SDK sirtlari**:
   - Rust (`iroha_client`): SM2 imzolash/tasdiqlash yordamchilari, SM3 xeshlash, deterministik sukut bo'yicha SM4 AEAD o'ramlarini ochish.
   - Python/JS/Swift: Rust API-ni aks ettiradi; tillararo testlar uchun `sm_known_answers.toml` da bosqichli moslamalarni qayta ishlating.
4. CLI/SDK tezkor ishga tushirishda SMni yoqish uchun operator ish jarayonini hujjatlashtiring va JSON/YAML konfiguratsiyasi yangi algoritm teglarini qabul qilishiga ishonch hosil qiling.

#### CLI jarayoni
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` endi `client.toml` parchasi (`public_key_config`, `private_key_hex`, Hyperledger) bilan birga SM2 kalit juftligini tavsiflovchi JSON foydali yukini chiqaradi. Buyruq deterministik yaratish uchun `--seed-hex` ni qabul qiladi va xostlar tomonidan ishlatiladigan RFC 6979 hosilasini aks ettiradi.
- `cargo xtask sm-operator-snippet --distid CN12345678901234` keygen/eksport oqimini o'rab, bir xil `sm2-key.json`/`client-sm2.toml` chiqishlarini bir qadamda yozadi. `--json-out <path|->` / `--snippet-out <path|->` dan fayllarni qayta yo'naltirish yoki ularni stdout-ga oqimlash, avtomatlashtirish uchun `jq` qaramligini olib tashlash uchun foydalaning.
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` mavjud materialdan bir xil metamaʼlumotlarni oladi, shuning uchun operatorlar kirishdan oldin farqlovchi identifikatorlarni tasdiqlashlari mumkin.
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` konfiguratsiya snippetini (jumladan, `allowed_signing`/`sm2_distid_default` yoʻriqnomasini) chop etadi va ixtiyoriy ravishda skript yaratish uchun JSON kalit inventarini qayta chiqaradi.
- `iroha_cli tools crypto sm3 hash --data <string>` ixtiyoriy foydali yuklarni xeshlaydi; `--data-hex` / `--file` ikkilik kirishlarni qamrab oladi va buyruq manifest asboblari uchun hex va base64 dayjestlari haqida xabar beradi.
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (va `gcm-open`) xost SM4-GCM yordamchilarini va sirt `ciphertext_hex`/`tag_hex` yoki ochiq matnli foydali yuklarni o'rash. `sm4 ccm-seal` / `sm4 ccm-open` CCM uchun bir xil uzunlikdagi (7–13 bayt) va teg uzunligi (4,6,8,10,12,14,16) tasdiqlanishi bilan bir xil UXni taʼminlaydi; ikkala buyruq ham ixtiyoriy ravishda diskga xom baytlarni chiqaradi.## Sinov strategiyasi
### Birlik/Ma'lum javob testlari
- SM3 uchun GM/T 0004 & GB/T 32905 vektorlari (masalan, `"abc"`).
- SM4 uchun GM/T 0002 & RFC 8998 vektorlari (blok + GCM/CCM).
- SM2 uchun GM/T 0003/GB/T 32918 misollari (Z-qiymati, imzo tekshiruvi), shu jumladan `ALICE123@YAHOO.COM` identifikatorli 1-ilova-misol.
- Vaqtinchalik o'rnatish fayli: `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- Wycheproof-dan olingan SM2 regressiya to'plami (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) endi deterministik moslamalarni (D-ilova, SDK urug'lari) bit-flip, xabar-buzg'unchi va qisqartirilgan imzo negativlari bilan qatlamlaydigan 52-xodisali korpusga ega. Sanitatsiya qilingan JSON `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` da yashaydi va `sm2_fuzz.rs` uni to'g'ridan-to'g'ri iste'mol qiladi, shuning uchun ham baxtli yo'l, ham o'zgartirish stsenariylari noaniq/xususiyatlar bo'ylab mos keladi. dawamiee gae-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-ilova dae-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-i-ii.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (yoki `--input-url <https://…>`) har qanday yuqori oqim tushishini (generator yorlig'i ixtiyoriy) aniqlab beradi va `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`ni qayta yozadi. C2SP rasmiy korpusni nashr etgunga qadar, vilkalarni qo'lda yuklab oling va ularni yordamchi orqali boqing; u kalitlarni, hisoblarni va bayroqlarni normallashtiradi, shuning uchun sharhlovchilar farqlar ustida fikr yuritishi mumkin.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` da tasdiqlangan SM2/SM3 Norito bo'ylab sayohatlar.
- `crates/ivm/tests/sm_syscalls.rs` da SM3 xost tizimi chaqiruvi regressiyasi (SM xususiyati).
- SM2 `crates/ivm/tests/sm_syscalls.rs` tizimidagi regressiyani tasdiqlaydi (muvaffaqiyat + muvaffaqiyatsizlik holatlari).

### Mulk va regressiya testlari
- Yaroqsiz egri chiziqlarni, kanonik bo'lmagan r/sni va bo'lmaganlarni qayta ishlatishni rad etuvchi SM2 taklifi. *(`crates/iroha_crypto/tests/sm2_fuzz.rs` da mavjud, `sm_proptest` orqasida joylashgan; `cargo test -p iroha_crypto --features "sm sm_proptest"` orqali yoqing.)*
- Turli rejimlarga moslashtirilgan Wycheproof SM4 vektorlari (blok/AES-rejimi); SM2 qo'shimchalari uchun yuqori oqimni kuzatib boring. `sm3_sm4_vectors.rs` endi GCM va CCM uchun teglar bitlarini aylantirish, kesilgan teglar va shifrlangan matnni buzishni mashq qiladi.

### O'zaro ishlash va ishlash
- RustCrypto ↔ SM2 belgisi/verify, SM3 dayjestlari va SM4 ECB/GCM uchun OpenSSL/Tongsuo paritet to'plami `crates/iroha_crypto/tests/sm_cli_matrix.rs` da yashaydi; uni `scripts/sm_interop_matrix.sh` bilan chaqiring. CCM paritet vektorlari endi `sm3_sm4_vectors.rs` da ishlaydi; Yuqori oqimdagi CLIlar CCM yordamchilarini fosh qilgandan so'ng, CLI matritsasi qo'llab-quvvatlanadi.
- SM3 NEON yordamchisi endi Armv8 siqish/to‘ldirish yo‘lini `sm_accel::is_sm3_enabled` orqali ish vaqti chegarasi bilan oxirigacha boshqaradi (SM3/SM4 bo‘ylab aks ettirilgan xususiyat + env bekor qilish). Oltin dayjestlar (nol/`"abc"`/uzun-blok + tasodifiy uzunliklar) va majburiy o‘chirish testlari RustCrypto skalyar orqa qismi bilan tenglikni saqlaydi va Criterion mikro dastgohi (`crates/sm3_neon/benches/digest.rs`) A.Arch xostlarida skalyar va NEON6 o‘tkazish qobiliyatini qayd etadi.
- Ed25519/SHA-2 va SM2/SM3/SM4 ni solishtirish va bardoshlik chegaralarini tekshirish uchun `scripts/gost_bench.sh` aks ettiruvchi mukammal jabduqlar.#### Arm64 Baseline (mahalliy Apple Silicon; `sm_perf` mezoni, 2025-12-05 yangilangan)
- `scripts/sm_perf.sh` endi Criterion dastgohini boshqaradi va `crates/iroha_crypto/benches/sm_perf_baseline.json` ga nisbatan medianlarni qo'llaydi (aarch64 macOS da qayd etilgan; tolerantlik sukut bo'yicha 25%, asosiy metama'lumotlar xost uch barobarini oladi). Yangi `--mode` bayrog'i muhandislarga skaler va NEON va `sm-neon-force` ma'lumotlar nuqtalarini skriptni tahrir qilmasdan olish imkonini beradi; joriy suratga olish paketi (xom JSON + jamlangan xulosa) `artifacts/sm_perf/2026-03-lab/m3pro_native/` ostida ishlaydi va har bir foydali yukni `cpu_label="m3-pro-native"` bilan muhrlaydi.
- Tezlashtirish rejimlari endi taqqoslash maqsadi sifatida skalyar bazani avtomatik ravishda tanlaydi. `scripts/sm_perf.sh` iplari `--compare-baseline/--compare-tolerance/--compare-label` dan `sm_perf_check` gacha, skaler moslamaga nisbatan har bir benchmark deltalarini chiqaradi va sekinlashuv sozlangan chegaradan oshib ketganda ishlamay qoladi. Taqqoslash qo'riqchisini bazaviy ko'rsatkichlar bo'yicha toleranslar boshqaradi (SM3 Apple skalyar bazasida 12% bilan chegaralangan, SM3 taqqoslash deltasi esa endi chayqalishning oldini olish uchun skaler moslamaga nisbatan 70% gacha ruxsat beradi); Linux bazalari bir xil taqqoslash xaritasini qayta ishlatadi, chunki ular `neoverse-proxy-macos` suratga olishdan eksport qilinadi va agar medianalar farq qilsa, biz ularni yalang'och metall Neoverse ishga tushirgandan so'ng kuchaytiramiz. Qattiqroq chegaralarni (masalan, `--compare-tolerance 0.20`) qo'lga kiritishda `--compare-tolerance` dan aniq o'ting va muqobil mos yozuvlar xostlariga izoh qo'shish uchun `--compare-label` dan foydalaning.
- CI ma'lumot mashinasida qayd etilgan asosiy chiziqlar endi `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` va `sm_perf_baseline_aarch64_macos_neon_force.json` da mavjud. Ularni `scripts/sm_perf.sh --mode scalar --write-baseline`, `--mode auto --write-baseline` yoki `--mode neon-force --write-baseline` bilan yangilang (qo'lga olishdan oldin `SM_PERF_CPU_LABEL` o'rnating) va yaratilgan JSONni ishga tushirish jurnallari bilan birga arxivlang. Ko'rib chiquvchilar har bir namunani tekshirishlari uchun yig'ilgan yordamchi chiqishni (`artifacts/.../aggregated.json`) PR bilan saqlang. Linux/Neoverse bazalari endi `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json` da yuboriladi, `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` (CPU yorlig'i `neoverse-proxy-macos`, SM3 aarch64 macOS/Linux uchun tolerantlikni 0,70 bilan solishtiradi); toleranslarni kuchaytirish uchun mavjud bo'lganda yalang'och metall Neoverse xostlarida qayta ishga tushiring.
- Asosiy JSON fayllari endi har bir mezon uchun himoya panjaralarini mustahkamlash uchun ixtiyoriy `tolerances` ob'ektiga ega bo'lishi mumkin. Misol:
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` ushbu kasr chegaralarini (misolda 8% va 12%) qo'llaydi, shu bilan birga ro'yxatga kiritilmagan har qanday ko'rsatkichlar uchun global CLI tolerantligidan foydalanadi.
- Taqqoslash soqchilari `compare_tolerances` ni taqqoslashning asosiy chizig'ida ham hurmat qilishlari mumkin. To'g'ridan-to'g'ri bazaviy tekshiruvlar uchun birlamchi `tolerances` qat'iyligini saqlab, skaler moslamaga nisbatan (masalan, `\"sm3_vs_sha256_hash/sm3_hash\": 0.70`) nisbatan bo'shashgan deltaga ruxsat berish uchun bundan foydalaning.- Tekshirilgan Apple Silicon bazalari endi beton himoya panjaralari bilan jo'natiladi: SM2/SM4 operatsiyalari farqlarga qarab 12–20% driftga imkon beradi, SM3/ChaCha taqqoslashlari esa 8–12% ni tashkil qiladi. Skayar bazaning `sm3` tolerantligi endi 0,12 ga kuchaytirildi; `unknown_linux_gnu` fayllari `neoverse-proxy-macos` eksportini bir xil bardoshlik xaritasi bilan aks ettiradi (SM3 0,70 bilan solishtiring) va metama'lumotlar qaydlari yalang'och metall Neoverse qayta ishlanmaguncha Linux darvozasi uchun yuborilganligini ko'rsatadi.
- SM2 imzolash: har bir operatsiya uchun 298µs (Ed25519: 32µs) ⇒ ~9,2× sekinroq; tekshirish: 267µs (Ed25519: 41µs) ⇒ ~6,5× sekinroq.
- SM3 xeshing (4KiB foydali yuk): 11,2µs, SHA-256 bilan 11,3µs (≈356MiB/s va 353MiB/s) bilan samarali paritet.
- SM4-GCM muhr/ochiq (1KiB foydali yuk, 12 bayt bo'lmagan): 15,5µs va ChaCha20-Poly1305 1,78µs (≈64MiB/s va 525MiB/s).
- Qayta ishlab chiqarish uchun olingan benchmark artefaktlari (`target/criterion/sm_perf*`); Linux bazasi `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` dan olingan (CPU yorlig'i `neoverse-proxy-macos`, SM3 tolerantlikni solishtirish 0,70) va tolerantlikni kuchaytirish uchun laboratoriya vaqti ochilgandan so'ng yalang'och metall Neoverse xostlarida (`SM-4c.1`) yangilanishi mumkin.

#### O'zaro arxitekturani suratga olish nazorat ro'yxati
- `scripts/sm_perf_capture_helper.sh` **ni maqsadli mashinada** ishga tushiring (x86_64 ish stantsiyasi, Neoverse ARM server va boshqalar). Tasvirga muhrlash uchun `--cpu-label <host>` dan o'ting va (matritsa rejimida ishlayotganda) laboratoriya rejalashtirish uchun yaratilgan reja/buyruqlarni oldindan to'ldirish uchun. Yordamchi rejimga xos buyruqlarni chop etadi:
  1. Criterion to'plamini to'g'ri xususiyatlar to'plami bilan bajaring va
  2. medianalarni `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json` ga yozing.
- Avval skalyar asosiy chiziqni oling, so'ngra `auto` (va AArch64 platformalarida `neon-force`) uchun yordamchini qayta ishga tushiring. Sharhlovchilar JSON metamaʼlumotlarida xost tafsilotlarini kuzatishi uchun mazmunli `SM_PERF_CPU_LABEL` dan foydalaning.
- Har bir ishga tushirilgandan so'ng, xom `target/criterion/sm_perf*` katalogini arxivlang va uni yaratilgan asosiy ko'rsatkichlar bilan birga PRga qo'shing. Ketma-ket ikkita yugurish barqarorlashishi bilanoq har bir benchmark toleranslarini torting (mos yozuvlar formatlash uchun `sm_perf_baseline_aarch64_macos_*.json` ga qarang).
- Ushbu bo'limda medianlar + tolerantliklarni yozib oling va yangi arxitektura qamrab olinganda `status.md`/`roadmap.md`-ni yangilang. Linux bazalari endi `neoverse-proxy-macos` suratga olishdan tekshiriladi (metadata aarch64-noma'lum-linux-gnu darvozasiga eksport qilinganligini qayd etadi); Yalang'och metall Neoverse/x86_64 xostlarida ushbu laboratoriya uylari mavjud bo'lganda, keyingi nazorat sifatida qayta ishga tushiring.

#### ARMv8 SM3/SM4 ichki va skaler yo'llar
`sm_accel` (qarang: `crates/iroha_crypto/src/sm.rs:739`) NEON tomonidan qo'llab-quvvatlanadigan SM3/SM4 yordamchilari uchun ish vaqti jo'natish qatlamini taqdim etadi. Xususiyat uchta darajada himoyalangan:| Qatlam | Nazorat | Eslatmalar |
|-------|---------|-------|
| Kompilyatsiya vaqti | `--features sm` (endi `sm-neon` `aarch64` da avtomatik ravishda tortiladi) yoki `sm-neon-force` (testlar/benchmarklar) | NEON modullarini quradi va `sm3-neon`/`sm4-neon` havolalarini yaratadi. |
| Ish vaqti avtomatik aniqlash | `sm4_neon::is_supported()` | Faqat AES/PMULL ekvivalentlarini (masalan, Apple M-seriyasi, Neoverse V1/N2) ochib beradigan protsessorlar uchun amal qiladi. NEON yoki FEAT_SM4 ni niqoblaydigan VMlar skaler kodga qaytadi. |
| Operatorni bekor qilish | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | Konfiguratsiyaga asoslangan jo'natish ishga tushirilganda qo'llaniladi; `force-enable` dan faqat ishonchli muhitda profil yaratish uchun foydalaning va skalyar zaxiralarni tekshirishda `force-disable` ni afzal qiling. |

**Umumiylik konverti (Apple M3 Pro; medianlar `sm_perf_baseline_aarch64_macos_{mode}.json` da yozilgan):**

| Rejim | SM3 dayjesti (4KiB) | SM4-GCM muhri (1KiB) | Eslatmalar |
|------|-------------------|----------------------|-------|
| Skalar | 11,6µs | 15,9µs | Deterministik RustCrypto yo'li; hamma joyda ishlatiladi `sm` xususiyati kompilyatsiya qilingan, lekin NEON mavjud emas. |
| NEON avto | ~2,7x skaler | dan tezroq ~2,3x skaler | dan tezroq Joriy NEON yadrolari (SM-5a.2c) bir vaqtning o'zida to'rtta so'z jadvalini kengaytiradi va ikkita navbatdagi fan chiqishidan foydalanadi; aniq medianlar xostga qarab farq qiladi, shuning uchun asosiy JSON metamaʼlumotlariga murojaat qiling. |
| NEON kuchi | NEONni avtomatik aks ettiradi, lekin qayta tiklashni butunlay o'chiradi | NEON auto | bilan bir xil `scripts/sm_perf.sh --mode neon-force` orqali mashq qilingan; CI ni hatto skaler rejimga sukut bo'lgan xostlarda ham halol saqlaydi. |

**Determinizm va joylashtirish bo'yicha ko'rsatmalar**
- Intrinsics hech qachon kuzatilishi mumkin bo'lgan natijalarni o'zgartirmaydi - `sm_accel` tezlashtirilgan yo'l mavjud bo'lmaganda `None` ni qaytaradi, shuning uchun skaler yordamchi ishlaydi. Shuning uchun konsensus kodining yo'llari skalyar amalga oshirish to'g'ri bo'lsa, deterministik bo'lib qoladi.
- NEON yo'li ishlatilganmi yoki yo'qmi, biznes mantig'ini **bermang**. Tezlashtirishni faqat yaxshi maslahat sifatida ko'ring va holatni faqat telemetriya orqali oching (masalan, `sm_intrinsics_enabled` o'lchagich).
- SM kodini bosgandan so'ng har doim `ci/check_sm_perf.sh` (yoki `make check-sm-perf`) ni ishga tushiring, shunda Mezon jabduqlari har bir asosiy JSONga o'rnatilgan toleranslar yordamida ham skaler, ham tezlashtirilgan yo'llarni tasdiqlaydi.
- Benchmarking yoki disk raskadrovka paytida kompilyatsiya vaqti belgilaridan `crypto.sm_intrinsics` konfiguratsiya tugmasiga ustunlik bering; `sm-neon-force` bilan qayta kompilyatsiya qilish skalyar qaytarilishni butunlay o'chirib qo'yadi, `force-enable` esa ish vaqtini aniqlashni shunchaki o'zgartiradi.
- Tanlangan siyosatni nashr qaydlarida hujjatlashtiring: ishlab chiqarish tuzilmalari siyosatni `Auto` da tark etishi kerak, bu esa har bir validatorga apparat imkoniyatlarini mustaqil ravishda kashf qilish va bir xil ikkilik artefaktlarni baham ko‘rish imkonini beradi.
- Statik ravishda bog'langan sotuvchining ichki ma'lumotlarini (masalan, uchinchi tomon SM4 kutubxonalari) aralashtiruvchi ikkilik fayllarni jo'natishdan saqlaning, agar ular bir xil jo'natish va sinov oqimiga rioya qilmaguncha, aks holda bizning asosiy vositalarimiz mukammal regressiyalarni ushlab turmaydi.#### x86_64 Rosetta bazaviy chiziq (Apple M3 Pro; olingan 2025-12-01)
- Asosiy chiziqlar `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) da, `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` ostida xom + jamlangan suratlar bilan yashaydi.
- X86_64 uchun har bir benchmark toleranslari SM2 uchun 20%, Ed25519/SHA-256 uchun 15% va SM4/ChaCha uchun 12% ga o'rnatiladi. `scripts/sm_perf.sh` endi AAarch64 bo'lmagan xostlarda tezlashtirishni taqqoslash tolerantligini sukut bo'yicha 25% ga qo'yadi, shuning uchun skaler-avtomatik o'zaro bog'liq bo'lib qoladi va Neoverse qayta ishga tushirilgunga qadar umumiy `m3-pro-native` bazaviy chiziq uchun AArch64 da 5,25 bo'sh qoladi.

| Benchmark | Skalar | Avto | Neon-Force | Avto va Skalar | Neon vs Skalar | Neon vs Avto |
|----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57.43 |  57.12 |      55.77 |          -0,53% |         -2,88% |        -2,36% |
| sm2_vs_ed25519_sign/sm2_sign |   572,76 | 568,71 |     557,83 |          -0,71% |         -2,61% |        -1,91% |
| sm2_vs_ed25519_verify/verify/ed25519 |    69.03 |  68.42 |      66.28 |          -0,88% |         -3,97% |        -3,12% |
| sm2_vs_ed25519_verify/verify/sm2 |   521.73 | 514.50 |     502.17 |          -1,38% |         -3,75% |        -2,40% |
| sm3_vs_sha256_hash/sha256_hash |    16.78 |  16.58 |      16.16 |          -1,19% |         -3,69% |        -2,52% |
| sm3_vs_sha256_hash/sm3_hash |    15.78 |  15.51 |      15.04 |          -1,71% |         -4,69% |        -3,03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1.96 |   1.97 |       1.97 |           0,39% |          0,16% |        -0,23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0,72% |         -0,01% |        -0,72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.96 |   2.00 |       1.93 |           2,23% |         -1,14% |        -3,30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16.58 |      16.15 |          -0,10% |         -2,66% |        -2,57% |

#### x86_64 / aarch64 bo'lmagan boshqa maqsadlar
- Hozirgi tuzilmalar hali ham x86_64 da faqat deterministik RustCrypto skalyar yo'lini jo'natadi; `sm`-ni yoqilgan holda saqlang, lekin SM-4c.1b qo'nguncha tashqi AVX2/VAES yadrolarini **bermang**. Ish vaqti siyosati ARMni aks ettiradi: sukut bo'yicha `Auto`, `crypto.sm_intrinsics` ni hurmat qiling va bir xil telemetriya o'lchagichlarini ishlating.
- Linux/x86_64 yozuvlari yozilishi kerak; ushbu uskunada yordamchini qayta ishlating va medianlarni yuqoridagi Rosetta asosiy chiziqlari va bardoshlik xaritasi bilan birga `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` ga tushiring.**Umumiy tuzoqlar**
1. **Virtuallashtirilgan ARM misollari:** Ko‘pgina bulutlar NEONni ko‘rsatadi, lekin `sm4_neon::is_supported()` tekshiradigan SM4/AES kengaytmalarini yashiradi. O'sha muhitlarda skalyar yo'lni kuting va shunga mos ravishda mukammal asosiy chiziqlarni oling.
2. **Qisman bekor qilish:** Ishlashlar o‘rtasida doimiy `crypto.sm_intrinsics` qiymatlarini aralashtirish mos kelmasligi perf ko‘rsatkichlariga olib keladi. Tajriba chiptasida mo'ljallangan bekor qilishni hujjatlang va yangi asosiy chiziqlarni olishdan oldin konfiguratsiyani qayta o'rnating.
3. **CI pariteti:** NEON faol bo‘lsa, ba’zi macOS dasturlari teskari asosidagi perf namunalarini olishga ruxsat bermaydi. `scripts/sm_perf_capture_helper.sh` chiqishlarini PRga biriktirib qo'ying, shunda ko'rib chiquvchilar yuguruvchi o'sha hisoblagichlarni yashirgan taqdirda ham tezlashtirilgan yo'l bajarilganligini tasdiqlashlari mumkin.
4. **Kelajakdagi ISA variantlari (SVE/SVE2):** Joriy yadrolar NEON chiziqli shakllarini qabul qiladi. SVE/SVE2-ga o'tishdan oldin `sm_accel::NeonPolicy`-ni maxsus variant bilan kengaytiring, shunda biz CI, telemetriya va operator tugmachalarini bir xilda ushlab turishimiz mumkin.

SM-5a/SM-4c.1 ostida kuzatilgan amallar CI har bir yangi arxitektura uchun paritet dalillarini olishini taʼminlaydi va yoʻl xaritasi Neoverse/x86 bazaviy chiziqlari va NEON-skalar tolerantliklari yaqinlashguncha 🈺 da qoladi.

## Muvofiqlik va me'yoriy eslatmalar

### Standartlar va me'yoriy havolalar
- **GM/T 0002-2012** (SM4), **GM/T 0003-2012** + **GB/T 32918 seriyali** (SM2), **GM/T 0004-2012** + **GB/T 32905/32907** (SM39th) va qurilmalarimiz iste'mol qiladigan ta'riflar, test vektorlari va KDF ulanishlari.【docs/source/crypto/sm_vectors.md#L79】
- `docs/source/crypto/sm_compliance_brief.md` da muvofiqlik bayonnomasi muhandislik, SRE va yuridik guruhlar uchun hujjatlarni topshirish/eksport majburiyatlari bilan bir qatorda ushbu standartlarni o'zaro bog'laydi; GM/T katalogi qayta ko'rib chiqilsa, bu qisqacha ma'lumotni yangilab turing.

### Materik Xitoy tartibga soluvchi ish oqimi
1. **Mahsulotni topshirish (xàngàngìní):** Materik Xitoydan SM yoqilgan binarlarni joʻnatishdan oldin artefakt manifestini, deterministik yaratish bosqichlarini va bogʻliqlik roʻyxatini viloyat kriptografiyasi maʼmuriyatiga yuboring. To'ldirish shablonlari va muvofiqlikni tekshirish ro'yxati `docs/source/crypto/sm_compliance_brief.md` va qo'shimchalar katalogida (`sm_product_filing_template.md`, `sm_sales_usage_filing_template.md`, `sm_export_statement_template.md`) mavjud.
2. **Sotish/foydalanish uchun ariza berish (kànhàng/yànjàngíní):** Quruqlikda SM-ni yoqadigan tugunlarni boshqarayotgan operatorlar oʻzlarining joylashtirish doirasi, kalitlarni boshqarish holati va telemetriya rejasini roʻyxatdan oʻtkazishlari kerak. Hujjat topshirishda imzolangan manifestlar va `iroha_sm_*` metrik snapshotlarini ilova qiling.
3. **Akkreditatsiyalangan test:** Muhim infratuzilma operatorlari sertifikatlangan laboratoriya hisobotlarini talab qilishi mumkin. Qayta tiklanadigan qurilish skriptlarini, SBOM eksportlarini va Wycheproof/interop artefaktlarini (pastga qarang) taqdim eting, shunda quyi oqim auditorlari kodni o'zgartirmasdan vektorlarni qayta ishlab chiqarishlari mumkin.
4. **Holatni kuzatish:** Tugallangan arizalarni chiqarish chiptasi va `status.md` da yozib oling; etishmayotgan arizalar faqat tekshirishdan imzo chekuvchi uchuvchilarga ko'tarilishni bloklaydi.### Eksport va tarqatish holati
- SM-qo‘llab-quvvatlaydigan ikkilik fayllarni **US EAR toifasi 5                **     **        **       **       ** va **                 (5D002) reglamentining                                                                              boshqariladigan elementlar sifatida SM  SM - SM - SM -qobiliyatiga ega bo'lgan  qorama qoidalari           **020202020 qoidalari (5D002             **    ** **    (5D0002020202020202020 . Manbani nashr etish ochiq manbali/ENC bo'limlari uchun javob berishda davom etmoqda, ammo embargo qilingan manzillarga qayta taqsimlash hali ham qonuniy ko'rib chiqishni talab qiladi.
- Relizlar manifestlari ENC/TSU asosiga havola qiluvchi eksport bayonotini to'plashi va agar FFI oldindan ko'rish paketlangan bo'lsa, OpenSSL/Tongsuo qurish identifikatorlari ro'yxatini ko'rsatishi kerak.
- Transchegaraviy uzatish bilan bog'liq muammolarni oldini olish uchun operatorlar quruqlikda tarqatish kerak bo'lganda, mintaqaviy mahalliy qadoqlash (masalan, materik oynalari) ni afzal ko'ring.

### Operator hujjatlari va dalillari
- Ushbu arxitektura qisqachasini `docs/source/crypto/sm_operator_rollout.md` va `docs/source/crypto/sm_compliance_brief.md` da muvofiqlikni topshirish bo'yicha qo'llanma bilan bog'lang.
- `docs/genesis.md`, `docs/genesis.he.md` va `docs/genesis.ja.md` bo'ylab genezis/operator tezkor ishga tushirishni sinxronlashtiring; SM2/SM3 CLI ish oqimi `crypto` manifestlarini ekish uchun operatorga qaragan haqiqat manbai mavjud.
- OpenSSL/Tongsuo kelib chiqishi, `scripts/sm_openssl_smoke.sh` chiqishi va `scripts/sm_interop_matrix.sh` paritet jurnallarini har bir nashr toʻplami bilan arxivlang, shuning uchun muvofiqlik va audit hamkorlari deterministik artefaktlarga ega.
- Muvofiqlik ko'lami o'zgarganda (yangi yurisdiktsiyalar, hujjatlarni to'ldirish yoki eksport qarorlari) dastur holatini ochiq tutish uchun `status.md` ni yangilang.
- `docs/source/release_dual_track_runbook.md` da olingan bosqichli tayyorgarlik ko'rib chiqishlariga (`SM-RR1`–`SM-RR3`) rioya qiling; Faqat tekshirish, uchuvchi va GA imzolash bosqichlari o'rtasida targ'ib qilish u erda sanab o'tilgan artefaktlarni talab qiladi.

## Interop retseptlari

### RustCrypto ↔ OpenSSL/Tongsuo matritsasi
1. OpenSSL/Tongsuo CLIs mavjudligiga ishonch hosil qiling (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` aniq asboblarni tanlash imkonini beradi).
2. `scripts/sm_interop_matrix.sh` ni ishga tushiring; u `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` ni chaqiradi va SM2 imzolash/verify, SM3 dayjestlari va SM4 ECB/GCM oqimlarini har bir provayderga qarshi mashq qiladi va mavjud bo'lmagan har qanday CLI-ni o'tkazib yuboradi.【scripts/sm_interop_matrix.sh#L1】
3. Olingan `target/debug/deps/sm_cli_matrix*.log` fayllarini reliz artefaktlari bilan arxivlang.

### OpenSSL oldindan ko'rish tutuni (qadoqlash darvozasi)
1. OpenSSL ≥3.0 ishlab chiqish sarlavhalarini oʻrnating va `pkg-config` ularni topa olishiga ishonch hosil qiling.
2. `scripts/sm_openssl_smoke.sh` ni bajaring; yordamchi `cargo check`/`cargo test --test sm_openssl_smoke` ni ishga tushiradi, SM3 xeshing, SM2 tekshiruvi va FFI backend orqali SM4-GCM aylanma safarlarini amalga oshiradi (sinov jabduqlari oldindan ko'rishni aniq ko'rish imkonini beradi).【scripts/sm_openssl#smo
3. O'tkazib yuborilmaydigan har qanday nosozlikni bo'shatish blokatori sifatida ko'ring; audit dalillari uchun konsol chiqishini yozib oling.

### Deterministik moslamani yangilash
- Har bir muvofiqlikni topshirishdan oldin SM moslamalarini (`sm_vectors.md`, `fixtures/sm/…`) yangilang, so'ngra paritet matritsasi va tutun simini qayta ishga tushiring, shunda auditorlar arizalar bilan birga yangi deterministik transkriptlarni oladilar.## Tashqi auditga tayyorgarlik
- `docs/source/crypto/sm_audit_brief.md` tashqi ko'rib chiqish uchun kontekst, qamrov, jadval va kontaktlarni to'playdi.
- Audit artefaktlari `docs/source/crypto/attachments/` (OpenSSL tutun jurnali, yuk daraxti surati, yuk metamaʼlumotlarini eksport qilish, asboblar toʻplamining kelib chiqishi) va `fuzz/sm_corpus_manifest.json` (mavjud regressiya vektorlaridan olingan deterministik SM fuzz urugʻlari) ostida yashaydi. MacOS tizimida tutun jurnali hozirda oʻtkazib yuborilgan ishga tushirishni qayd etadi, chunki ish maydoniga bogʻliqlik davri `cargo check` ni oldini oladi; Linux tsiklsiz tuzilmalari oldindan ko'rish backendini to'liq ishlatadi.
- Crypto WG, Platform Ops, Security va Docs/DevRel yetakchilariga 2026-01-30 da RFQ jo‘natmasidan oldin moslashtirish uchun yuborilgan.

### Audit ishtiroki holati

- **Bitlar izi (CN kriptografiya amaliyoti)** — **2026-02-21** da bajarilgan ishlar bayonnomasi, boshlanish sanasi **2026-02-24**, dala ishi oynasi **2026-02-24-2026-03-22**, yakuniy hisobot **2016-02-0. Haftalik holatni tekshirish punkti har chorshanba kuni soat 09:00UTC Crypto WG rahbari va Xavfsizlik muhandisligi aloqasi bilan. Kontaktlar, materiallar va dalillar qoʻshimchalari uchun [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) ga qarang.
- **NCC Group APAC (ehtimoliy holatlar uchun slot)** — 2026 yil may oyidagi oyna qo'shimcha topilmalar yoki tartibga soluvchi so'rovlar ikkinchi fikrni talab qilsa, keyingi/parallel ko'rib chiqish sifatida saqlangan. Ishlash tafsilotlari va eskalatsiya ilgaklari `sm_audit_brief.md` da Trail of Bits yozuvi bilan birga qayd etilgan.

## Xatarlar va kamaytirish choralari

To'liq ro'yxatga olish: batafsil ma'lumot uchun [`sm_risk_register.md`](sm_risk_register.md) ga qarang
ehtimollik/ta'sir bahosi, triggerlarni kuzatish va ro'yxatdan o'tish tarixi. The
Quyidagi xulosa muhandislikni chiqarish uchun paydo bo'lgan sarlavhalarni kuzatib boradi.
| Xavf | Jiddiylik | Egasi | Yumshatish |
|------|----------|-------|------------|
| RustCrypto SM qutilari uchun tashqi audit yo'qligi | Yuqori | Crypto WG | Shartnoma Trail of Bits/NCC Group, audit hisoboti qabul qilinmaguncha faqat tekshirishni davom eting. |
| SDKlar bo'yicha deterministik bo'lmagan regressiyalar | Yuqori | SDK dasturi yetakchilari | SDK CI bo'ylab moslamalarni baham ko'ring; kanonik r∥s kodlashni amalga oshirish; o'zaro SDK integratsiya testlarini qo'shing (SM-3c da kuzatilgan). |
| Intrinsicsdagi ISAga xos xatolar | O'rta | Ishlash WG | Xususiyatlar darvozasining ichki xususiyatlari, ARMda CI qamrovini talab qiladi, dasturiy ta'minotni qayta tiklashni ta'minlaydi. Uskunani tekshirish matritsasi `sm_perf.md` da saqlanadi. |
| Muvofiqlik noaniqligi qabul qilishni kechiktirish | O'rta | Hujjatlar va yuridik aloqa | Muvofiqlik haqida qisqacha ma'lumot va operator nazorat ro'yxatini (SM-6a/SM-6b) GA dan oldin e'lon qiling; huquqiy ma'lumotlarni to'plash. `sm_compliance_brief.md` da yuborilgan hujjatlarni tekshirish roʻyxati. |
| Provayder yangilanishlari bilan FFI backend drift | O'rta | Platforma operatsiyalari | Provayder versiyalarini mahkamlang, paritet testlarini qo'shing, qadoqlash barqarorlashguncha FFI backend ulanishini davom ettiring (SM-P3). |## Ochiq savollar / kuzatuvlar
1. Rustda SM algoritmlari bilan tajribaga ega mustaqil audit hamkorlarini tanlang.
   - **Javob (2026-02-24):** Bits’ning CN kriptografiyasi amaliyoti birlamchi audit SOWni imzoladi (boshlanish sanasi 2026-02-24, yetkazib berish 2026-04-15) va NCC Group APAC may oyida favqulodda vaziyatlar uchun slotga ega, shuning uchun regulyatorlar qayta ko‘rib chiqmasdan ikkinchi tekshiruvni talab qilishlari mumkin. Ishtirok etish doirasi, kontaktlar va nazorat roʻyxatlari [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) ichida joylashgan va `sm_audit_vendor_landscape.md` da aks ettirilgan.
2. Rasmiy Wycheproof SM2 ma'lumotlar to'plami uchun yuqori oqimni kuzatishni davom eting; ish maydoni hozirda 52-tasvirli komplektni (deterministik moslamalar + sintezlangan o'zgartirish qutilari) jo'natadi va uni `sm2_wycheproof.rs`/`sm2_fuzz.rs` ichiga yuboradi. Yuqori oqim JSON tushganidan keyin korpusni `cargo xtask sm-wycheproof-sync` orqali yangilang.
   - Track Bouncy Castle va GmSSL salbiy vektor to'plamlari; Mavjud korpusni to'ldirish uchun litsenziyalashdan so'ng `sm2_fuzz.rs` ga import qiling.
3. SMni qabul qilish monitoringi uchun asosiy telemetriyani (metrikalar, jurnallar) aniqlang.
4. Kotodama/VM taʼsir qilish uchun SM4 AEAD sukut boʻyicha GCM yoki CCM ekanligini aniqlang.
5. 1-ilova misoli uchun RustCrypto/OpenSSL paritetini kuzatib boring (ID `ALICE123@YAHOO.COM`): nashr etilgan ochiq kalit va `(r, s)` uchun kutubxona qo‘llab-quvvatlanishini tasdiqlang, shunda armatura regressiya testlariga ko‘tarilishi mumkin.

## Harakat elementlari
- [x] Xavfsizlik kuzatuvchisida qaramlik tekshiruvi va yozib olishni yakunlang.
- [x] RustCrypto SM qutilari uchun audit hamkorining kelishuvini tasdiqlang (SM-P0 kuzatuvi). Trail of Bits (CN kriptografiya amaliyoti) `sm_audit_brief.md` da qayd etilgan boshlanish/etkazib berish sanalari bilan birlamchi ko'rib chiqishga ega va NCC Group APAC regulyator yoki boshqaruv kuzatuvlarini qondirish uchun 2026 yil may oyida favqulodda vaziyatlarni saqlab qoldi.
- [x] SM4 CCM o'zgartirish holatlari (SM-4a) uchun Wycheproof qamrovini kengaytiring.
- [x] SDK va sim orqali CI (SM-3c/SM-1b.1) bo'ylab kanonik SM2 imzolash moslamalari; `scripts/check_sm2_sdk_fixtures.py` tomonidan qo'riqlanadi (qarang: `ci/check_sm2_sdk_fixtures.sh`).

## Muvofiqlik ilovasi (davlat tijorat kriptografiyasi)

- **Tasnifi: ** SM2/SM3/SM4 Xitoyning *davlat tijorat kriptografiyasi* rejimi ostida yuboriladi (XXR Kriptografiya qonuni, 3-modda). Ushbu algoritmlarni Iroha dasturiy ta'minotida jo'natish loyihani asosiy/umumiy (davlat siri) darajalariga **qo'ymaydi**, lekin XXRni joylashtirishda ulardan foydalanadigan operatorlar tijorat-kripto topshirish va MLPS majburiyatlariga rioya qilishlari kerak.
- **Standartlar qatori:** Ommaviy hujjatlarni GM/T xususiyatlarining rasmiy GB/T konvertatsiyalari bilan moslang:

| Algoritm | GB/T ma'lumotnomasi | GM/T kelib chiqishi | Eslatmalar |
|----------|----------------|-------------|-------|
| SM2 | GB/T32918 (barcha qismlar) | GM/T0003 | ECC raqamli imzo + kalit almashinuvi; Iroha yadro tugunlarida tekshirishni va SDK-larga deterministik imzolashni ochib beradi. |
| SM3 | GB/T32905 | GM/T0004 | 256 bitli xesh; skaler va ARMv8 tezlashtirilgan yo'llari bo'ylab deterministik xeshlash. |
| SM4 | GB/T32907 | GM/T0002 | 128 bitli blokli shifr; Iroha GCM/CCM yordamchilarini taqdim etadi va ilovalar bo'yicha katta-endian paritetini ta'minlaydi. |- **Qobiliyat manifesti:** Torii `/v2/node/capabilities` so‘nggi nuqtasi quyidagi JSON shaklini reklama qiladi, shuning uchun operatorlar va asboblar SM manifestini dasturiy ravishda iste’mol qilishi mumkin:

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

`iroha runtime capabilities` CLI kichik buyrug'i bir xil foydali yukni mahalliy darajada ko'rsatadi va muvofiqlik dalillarini to'plash uchun JSON reklamasi bilan bir qatorda bir qatorli xulosani chop etadi.

- **Tasdiqlangan hujjatlar:** yuqoridagi algoritmlar/standartlarni aniqlaydigan relizlar haqida eslatmalar va SBOMlarni nashr eting va operatorlar uni viloyat boʻlimlariga biriktirishlari uchun reliz artefaktlari bilan birga toʻliq muvofiqlik qisqachasini (`sm_chinese_crypto_law_brief.md`) saqlang. hujjatlar.【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **Operatorni topshirish:** tarqatuvchilarga MLPS2.0/GB/T39786-2021 kripto-ilovalarni baholash, SM kalitlarni boshqarish SOPlari va ≥6 yil dalillarni saqlashni talab qilishini eslatib turing; ularni muvofiqlik briefidagi operator nazorat roʻyxatiga yoʻnaltiring.【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## Aloqa rejasi
- **Tomoshabinlar:** Crypto WG asosiy aʼzolari, Release Engineering, Security Review Board, SDK dasturi yetakchilari.
- **Artifaktlar:** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, yoʻl xaritasi koʻchirmasi (SM-0 .. SM-7a).
- **Kanal:** Haftalik Crypto WG sinxronlash kun tartibi + amallarni umumlashtiradigan va qulfni yangilash va qaramlikni qabul qilish uchun tasdiqlashni talab qiluvchi keyingi elektron pochta (loyiha 2025-01-19 tarqatilgan).
- **Egasi:** Crypto WG yetakchisi (delegat qabul qilinadi).