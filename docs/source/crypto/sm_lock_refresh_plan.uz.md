---
lang: uz
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2025-12-29T18:16:35.943236+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM spike talab qiladigan Cargo.lock yangilanishini rejalashtirish tartibi.

# SM xususiyati `Cargo.lock` yangilash rejasi

`--locked` kuchga kirgan paytda `iroha_crypto` uchun `sm` funksiyasi dastlab `cargo check`ni yakunlay olmadi. Ushbu eslatma ruxsat etilgan `Cargo.lock` yangilanishi uchun muvofiqlashtirish bosqichlarini qayd etadi va ushbu ehtiyojning joriy holatini kuzatib boradi.

> **2026-02-12 yangilanishi:** Oxirgi tekshirish ixtiyoriy `sm` funksiyasini ko‘rsatadi, endi mavjud blokfayl bilan tuziladi (`cargo check -p iroha_crypto --features sm --locked` 7,9 soniya sovuq/0,23 soniya issiqda muvaffaqiyatli ishlaydi). Bog'liqlik to'plamida allaqachon `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `Cargo.lock`, `Cargo.lock`, `sm2`, `sm3`, `sm4` va `sm4-gcm`, shuning uchun darhol qulfni yangilash talab qilinmaydi. Kelajakdagi qaramlik zarbalari yoki yangi ixtiyoriy qutilar uchun quyidagi protsedurani kutish rejimida saqlang.

## Nima uchun yangilash kerak
- Spikening oldingi iteratsiyalari qulflangan faylda etishmayotgan ixtiyoriy qutilarni qo'shishni talab qildi. Joriy blokirovka suratlari allaqachon RustCrypto stekini o'z ichiga oladi (`sm2`, `sm3`, `sm4`, qo'llab-quvvatlovchi kodeklar va AES yordamchilari).
- Repository siyosati hali ham opportunistik qulflangan fayl tahrirlarini bloklaydi; agar kelajakda bog'liqlikni yangilash zarur bo'lsa, quyidagi tartib amalda qoladi.
- Ushbu rejani saqlab qoling, shunda jamoa SM bilan bog'liq yangi bog'liqliklar kiritilganda yoki mavjudlari versiya o'zgarishlarini talab qilganda nazorat ostida yangilashni amalga oshirishi mumkin.

## Tavsiya etilgan muvofiqlashtirish bosqichlari
1. **Crypto WG + Release Eng sinxronizatsiyasida so‘rovni ko‘tarish (egasi: @crypto-wg lead).**
   - `docs/source/crypto/sm_program.md` havolasi va xususiyatning ixtiyoriy xususiyatiga e'tibor bering.
   - Bir vaqtning o'zida qulflangan faylni o'zgartirish oynalari yo'qligini tasdiqlang (masalan, bog'liqlikni muzlatish).
2. **Loklash farqi bilan yamoq tayyorlang (egasi: @release-eng).**
   - Faqat kerakli qutilarni yangilash uchun `scripts/sm_lock_refresh.sh` (tasdiqlangandan keyin) ni bajaring.
   - `cargo tree -p iroha_crypto --features sm` chiqishini yozib oling (skript `target/sm_dep_tree.txt` chiqaradi).
3. **Xavfsizlik tekshiruvi (egasi: @security-reviews).**
   - Yangi qutilar/versiyalar audit reestriga va litsenziya talablariga mos kelishini tekshiring.
   - Ta'minot zanjiri kuzatuvchisida xeshlarni yozib oling.
4. **Birlashtirish oynasi bajarilishi.**
   - Faqat qulflangan fayl deltasi, bog'liqlik daraxti snapshoti (artifakt sifatida biriktirilgan) va yangilangan audit eslatmalarini o'z ichiga olgan PRni yuboring.
   - Birlashtirishdan oldin CI `cargo check -p iroha_crypto --features sm` bilan ishlashiga ishonch hosil qiling.
5. **Kuzatuv vazifalari.**
   - `docs/source/crypto/sm_program.md` harakat elementi nazorat roʻyxatini yangilang.
   - SDK jamoasini `--features sm` bilan xususiyatni mahalliy ravishda kompilyatsiya qilish mumkinligi haqida xabar bering.## Xronologiya va egalari
| Qadam | Maqsad | Egasi | Holati |
|------|--------|-------|--------|
| Keyingi Crypto WG chaqiruvida kun tartibini so'rang | 22.01.2025 | Crypto WG etakchi | ✅ Tugallandi (ko'rib chiqish tugallangan spike yangilanmasdan davom etishi mumkin) |
| Qoralama selektiv `cargo update` buyrug'i + aql-idrok farqi | 24.01.2025 | Release Engineering | ⚪ Kutish rejimida (yangi qutilar paydo bo'lsa, qayta faollashtiring) |
| Yangi qutilarning xavfsizlik tekshiruvi | 27.01.2025 | Xavfsizlik sharhlari | ⚪ Kutish rejimida (yangilash davom etganda tekshirish roʻyxatidan qayta foydalaning) |
| Bloklash faylini yangilash PR | 29.01.2025 | Release Engineering | ⚪ Kutish rejimida |
| SM dasturining hujjatlar roʻyxatini yangilash | Birlashgandan keyin | Crypto WG etakchi | ✅ `docs/source/crypto/sm_program.md` yozuvi orqali murojaat qilingan (2026-02-12) |

## Eslatmalar
- Kelajakdagi yangilanishni yuqorida sanab o'tilgan SM bilan bog'liq qutilar (va `rfc6979` kabi qo'llab-quvvatlovchi yordamchilar) bilan cheklab qo'ying, bunda ish maydoni bo'ylab `cargo update` dan qoching.
- Agar biron-bir o'tish davriga bog'liqlik MSRV driftini keltirib chiqarsa, uni birlashtirishdan oldin yuzaga keltiring.
- Birlashtirilgandan so'ng, `sm` xususiyati uchun qurish vaqtlarini kuzatish uchun vaqtinchalik CI ishini yoqing.