---
lang: uz
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2025-12-29T18:16:35.940439+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Tashqi audit bo'yicha qisqacha ma'lumot
% Iroha Kripto ishchi guruhi
% 2026-01-30

# Umumiy ko'rinish

Ushbu qisqacha ma'lumot uchun zarur bo'lgan muhandislik va muvofiqlik konteksti to'plangan
Iroha ning SM2/SM3/SM4 faolligini mustaqil ko'rib chiqish. U auditorlik guruhlariga mo'ljallangan
Rust kriptografiyasi tajribasi va Xitoy milliy kompaniyasi bilan tanish
Kriptografiya standartlari. Kutilgan natija yozma hisobotdir
amalga oshirish xatarlari, muvofiqlik kamchiliklari va ustuvor tuzatish bo'yicha ko'rsatmalar
oldindan ko'rishdan ishlab chiqarishga o'tadigan SM chiqarilishidan oldin.

# Dastur surati

- **Chiqarish doirasi:** Iroha 2/3 umumiy kod bazasi, deterministik tekshirish
  tugunlar va SDK'lar bo'ylab yo'llar, konfiguratsiya himoyasi orqasida imzo qo'yish mumkin.
- **Hozirgi bosqich:** Rust bilan SM-P3.2 (OpenSSL/Tongsuo backend integratsiyasi)
  tekshirish uchun yuborilgan ilovalar va nosimmetrik foydalanish holatlari.
- **Maqsadli qaror qabul qilish sanasi:** 2026-04-30 (audit natijalari go/no-go uchun ma'lumot beradi
  validator tuzilmalarida SM-ga kirishni yoqish).
- **Kuzatiladigan asosiy xavflar:** uchinchi tomonga qaramlik nasl-nasabi, deterministik
  aralash apparat ostida xatti-harakatlar, operator muvofiqligi tayyorligi.

# Kod va moslamalar uchun havolalar

- `crates/iroha_crypto/src/sm.rs` - Rust ilovalari va ixtiyoriy OpenSSL
  bog'lashlar (`sm-ffi-openssl` xususiyati).
- `crates/ivm/tests/sm_syscalls.rs` - IVM xeshlash uchun tizim qamrovi,
  tekshirish va simmetrik rejimlar.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` - Norito foydali yuk
  SM artefaktlari uchun aylanma sayohatlar.
- `docs/source/crypto/sm_program.md` - dastur tarixi, qaramlik auditi va
  aylanma to'siqlar.
- `docs/source/crypto/sm_operator_rollout.md` — operatorga qarashli yoqish va
  orqaga qaytarish protseduralari.
- `docs/source/crypto/sm_compliance_brief.md` - normativ xulosa va eksport
  mulohazalar.
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — OpenSSL tomonidan qo'llab-quvvatlanadigan oqimlar uchun deterministik tutun jabduqlari.
- `fuzz/sm_*` korpusi — SM3/SM4 ibtidoiylarini qamrab olgan RustCrypto asosidagi noaniq urug'lar.

# Talab qilingan audit doirasi1. **Spesifikasiyaga muvofiqligi**
   - SM2 imzo tekshiruvini, ZA hisobini va kanonikni tasdiqlang
     kodlash harakati.
   - GM/T 0002-2012 va GM/T 0007-2012 ga muvofiq SM3/SM4 primitivlarini tasdiqlang,
     qarshi rejimi o'zgarmaslari va IV ishlov berish, shu jumladan.
2. **Determinizm va doimiy vaqt kafolatlari**
   - Tugun bajarilishi uchun filiallarni, jadvallarni qidirishni va apparat jo'natishini ko'rib chiqing
     CPU oilalarida deterministik bo'lib qoladi.
   - Maxfiy kalit operatsiyalari uchun doimiy vaqt talablarini baholang va tasdiqlang
     OpenSSL/Tongsuo yo'llari doimiy vaqt semantikasini saqlaydi.
3. **Yon kanal va nosozlik tahlili**
   - Rust va ikkalasida ham vaqt, kesh va quvvat yon kanali xavflarini tekshiring
     FFI tomonidan qo'llab-quvvatlanadigan kod yo'llari.
   - Imzoni tekshirish uchun xatoliklarni bartaraf etish va xatolar tarqalishini baholash va
     autentifikatsiya qilingan shifrlash xatolari.
4. **Yaratish, bog'liqlik va ta'minot zanjirini ko'rib chiqish**
   - OpenSSL/Tongsuo artefaktlarining qayta tiklanadigan tuzilishi va kelib chiqishini tasdiqlang.
   - Tobelik daraxtini litsenziyalash va auditni qamrab olishni ko'rib chiqing.
5. **Sinov va tekshirish uskunalari tanqidi**
   - Deterministik tutun sinovlari, noaniq jabduqlar va Norito moslamalarini baholang.
   - Qo'shimcha qamrovni tavsiya eting (masalan, differentsial test, mulkka asoslangan
     dalillar) agar bo'shliqlar qolsa.
6. **Muvofiqlik va operator ko'rsatmalarini tekshirish**
   - Yuborilgan hujjatlarni qonuniy talablarga va kutilganiga muvofiq tekshirish
     operator boshqaruvlari.

# Yetkazib berish va logistika

- **Boshlanish vaqti:** 2026-02-24 (virtual, 90 daqiqa).
- **Intervyular:** Crypto WG, IVM texnik xizmat ko'rsatuvchilar, platforma operatsiyalari (kerak bo'lganda).
- **Artefaktga kirish:** faqat o‘qish uchun mo‘ljallangan ombor oynasi, CI quvuri jurnallari, armatura
  chiqishlar va qaramlik SBOMlari (CycloneDX).
- **Oraliq yangilanishlar:** haftalik yozma holat + xavf haqida eslatmalar.
- **Yakuniy natijalar (muddati 2026-04-15):**
  - Tavakkalchilik darajasi bilan ijro etuvchi xulosa.
  - Batafsil topilmalar (har bir masala: ta'sir, ehtimollik, kod havolalari,
    tuzatish bo'yicha ko'rsatmalar).
  - Qayta sinov/tasdiqlash rejasi.
  - Determinizm, doimiy vaqt holati va muvofiqlikni moslashtirish bo'yicha bayonot.

## Ishtirok etish holati

| Sotuvchi | Holati | Boshlanish | Maydon oynasi | Eslatmalar |
|--------|--------|----------|--------------|-------|
| Bitlar izi (CN amaliyoti) | Bajarilgan ishlar hisoboti 2026-02-21 | 2026-02-24 | 2026-02-24-2026-03-22 | Yetkazib berish muddati 2026-04-15; Hui Chjan muhandislik hamkasbi sifatida Aleksey M. bilan hamkorlikda etakchi. Haftalik holat chaqiruvi chorshanba kunlari 09:00UTC. |
| NCC Group (APAC) | Favqulodda vaziyatlar uchun joy ajratilgan | Yo'q (kutishda) | Vaqtinchalik 2026-05-06-2026-05-31 | Faqat yuqori xavfli topilmalar ikkinchi o'tishni talab qilsa, faollashtirish; tayyorlik Priya N. (Xavfsizlik) va NCC Group ish stoli tomonidan tasdiqlangan 2026-02-22. |

# Qo'shimchalar Autreach to'plamiga kiritilgan- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
- `docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"` surati.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — `iroha_crypto` qutisi uchun `cargo metadata` eksporti (qulflangan qaramlik grafigi).
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — soʻnggi `scripts/sm_openssl_smoke.sh` ishga tushirildi (provayder yordami yoʻq boʻlganda SM2/SM4 yoʻllarini oʻtkazib yuboradi).
- `docs/source/crypto/attachments/sm_openssl_provenance.md` - mahalliy asboblar to'plamining kelib chiqishi (pkg-config/OpenSSL versiya eslatmalari).
- Fuzz korpus manifest (`fuzz/sm_corpus_manifest.json`).

> **Atrof-muhitga oid ogohlantirish:** Joriy ishlanma surati ishlab chiqarilgan OpenSSL 3.x asboblar zanjiridan foydalanadi (`openssl` sandiq `vendored` xususiyati), lekin macOS’da SM3/SM4 protsessorining o‘ziga xos xususiyatlari yo‘q va standart provayder SMPS4-ning o‘zini tuta olmayotganligini ko‘rsatmaydi. qamrovi va ilova misoli SM2 tahlili. Ish maydoniga bog'liqlik davri (`sorafs_manifest ↔ sorafs_car`) ham `cargo check` xatosini chiqargandan so'ng yordamchi skriptni ishga tushirishni o'tkazib yuborishga majbur qiladi. Tashqi auditdan oldin toʻliq paritetni olish uchun toʻplamni Linux relizlar yaratish muhitida (SM4 yoqilgan OpenSSL/Tongsuo va siklsiz) qayta ishga tushiring.

# Nomzod audit hamkorlari va qamrovi

| Firma | Tegishli tajriba | Odatda ko'lami va yetkazib berish | Eslatmalar |
|------|---------------------|------------------------------|-------|
| Trail of Bits (CN kriptografiya amaliyoti) | Rust kodini ko'rib chiqish (`ring`, zkVMs), mobil to'lovlar to'plami uchun oldingi GM/T baholashlari. | Xususiyatlarga muvofiqlik farqi (GM/T 0002/3/4), Rust + OpenSSL yo'llarining doimiy ko'rib chiqilishi, differentsial noaniqlik, ta'minot zanjiri ko'rib chiqish, tuzatish yo'l xaritasi. | Allaqachon shug'ullangan; Kelajakdagi yangilanish davrlarini rejalashtirishda jadval to'liqligi uchun saqlanadi. |
| NCC Group APAC | Uskuna/SOC + Rust kriptografiyasi qizil jamoalari, RustCrypto ibtidoiylari va to'lov HSM ko'priklari haqida nashr etilgan sharhlar. | Rust + JNI/FFI ulanishlarining yaxlit bahosi, deterministik siyosatni tekshirish, perf/telemetriya eshigini ko'rib chiqish, operator o'yin kitobini o'rganish. | Favqulodda vaziyat sifatida saqlangan; Xitoy regulyatorlari uchun ikki tilli hisobotlarni ham taqdim etishi mumkin. |
| Kudelski Xavfsizlik (Blockchain va kripto jamoasi) | Rust-da amalga oshirilgan Halo2, Mina, zkSync, maxsus imzo sxemalarini tekshirish. | Elliptik egri chiziqning to'g'riligiga, transkript yaxlitligiga, apparat tezlashuvi uchun tahdid modellashtirishga va CI/rollout dalillariga e'tibor qarating. | Uskuna tezlashuvi (SM-5a) va FASTPQ-dan SM-ga o'zaro ta'sirlar bo'yicha ikkinchi fikrlar uchun foydalidir. |
| Eng kam vakolat | Rust-ga asoslangan blokcheynlar (Filecoin, Polkadot) uchun kriptografik protokol tekshiruvlari, takrorlanadigan qurilish konsaltinglari. | Deterministik qurilish tekshiruvi, Norito kodek tekshiruvi, muvofiqlik dalillarini o'zaro tekshirish, operator aloqasini tekshirish. | Regulyatorlar kodni tekshirishdan tashqari mustaqil tekshirishni talab qilganda shaffoflik/audit hisoboti natijalari uchun juda mos keladi. |

Barcha majburiyatlar yuqorida sanab o'tilgan bir xil artefakt to'plamini va firmaga qarab quyidagi ixtiyoriy qo'shimchalarni talab qiladi:- **Maxsus muvofiqlik va deterministik xatti-harakat: ** SM2 ZA hosilasi, SM3 to'ldirish, SM4 davra funksiyalari va `sm_accel` ish vaqti jo'natish shlyuzining satr bo'yicha tekshiruvi tezlashuv hech qachon semantikani o'zgartirmasligini ta'minlash uchun.
- **Yon kanal va FFI tekshiruvi:** Doimiy vaqtli da'volar, xavfli kod bloklari va OpenSSL/Tongsuo ko'prigi qatlamlarini tekshirish, shu jumladan Rust yo'liga qarshi farq sinovlari.
- **CI / ta'minot zanjiri tekshiruvi:** `sm_interop_matrix`, `sm_openssl_smoke` va `sm_perf` jabduqlarini SBOM/SLSA attestatsiyalari bilan ko'paytirish, shuning uchun audit natijalari bevosita dalillarni e'lon qilish uchun bog'lanishi mumkin.
- **Operator garovi:** `sm_operator_rollout.md`, muvofiqlik topshirish shablonlari va telemetriya asboblar panelini o'zaro tekshirish, hujjatlarda va'da qilingan yumshatishlar texnik jihatdan bajarilishi mumkinligini tasdiqlash.

Kelajakdagi auditlarni aniqlashda ushbu jadvaldan sotuvchining kuchli tomonlarini yoʻl xaritasining aniq bosqichiga moslashtirish uchun qaytadan foydalaning (masalan, apparat/mukammal relizlar uchun Kudelski, til/ish vaqtining toʻgʻriligi uchun Trail of Bits va takrorlanishi mumkin boʻlgan tuzilish kafolatlari uchun eng kichik vakolat).

# Aloqa nuqtalari

- **Texnik egasi:** Crypto WG yetakchisi (Aleksey M., `alexey@iroha.tech`)
- **Dastur menejeri:** Platforma operatsiyalari koordinatori (Sara K.,
  `sarah@iroha.tech`)
- **Xavfsizlik aloqasi:** Xavfsizlik muhandisligi (Priya N., `security@iroha.tech`)
- **Hujjatlar bilan bog‘lanish:** Docs/DevRel yetakchisi (Jamila R.,
  `docs@iroha.tech`)