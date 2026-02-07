---
lang: uz
direction: ltr
source: AGENTS.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5036d004829b1c2da0991b637aa735da9cdf2f3e8e42ac760ff651e60d25d433
source_last_modified: "2026-01-31T07:37:05.947018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AGENTLAR ko'rsatmalari

Ushbu ko'rsatmalar Yuk ish joyi sifatida tashkil etilgan butun omborga taalluqlidir.

## Tez boshlash
- Ish joyini yaratish: `cargo build --workspace`
- Qurilish taxminan 20 daqiqa davom etishi mumkin; qurish bosqichlari uchun 20 daqiqalik tanaffusdan foydalaning.
- Hammasini sinab ko'ring: `cargo test --workspace` (esda tutingki, bu ishga tushirish odatda bir necha soat davom etadi; shunga muvofiq rejalashtirish)
- Qattiq tuklar: `cargo clippy --workspace --all-targets -- -D warnings`
- Format kodi: `cargo fmt --all` (nashr 2024)
- Bitta qutini sinab ko'ring: `cargo test -p <crate>`
- Bitta testni bajaring: `cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK: Swift paketi testlarini bajarish uchun `IrohaSwift` katalogidan `swift test` ni ishga tushiring.
- Android SDK: `java/iroha_android` dan `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test` bilan ishlaydi.

## Umumiy ko'rinish
- Hyperledger Iroha bu blokcheyn platformasi
- DA/RBC-ni qo'llab-quvvatlash asosiy versiyaga qarab farqlanadi: Iroha 2 ixtiyoriy ravishda DA/RBC-ni yoqishi mumkin; Iroha 3 faqat DA/RBCni yoqishi mumkin.
- IVM bu Iroha virtual mashinasi (IVM), Hyperledger Iroha v2 blokcheyn uchun virtual mashina.
- Kotodama bu IVM uchun yuqori darajadagi aqlli kontrakt tili boʻlib, u xom shartnoma kodi uchun .ko fayl kengaytmasidan foydalanadi va fayl yoki zanjir sifatida saqlanganida .to fayl kengaytmasidan foydalanadigan baytkodga kompilyatsiya qilinadi. Odatda, .to bytecode zanjirda joylashtiriladi.
  - Aniqlik: Kotodama Iroha virtual mashinasini (IVM) maqsad qilib oladi va IVM bayt kodini (`.to`) ishlab chiqaradi. U mustaqil arxitektura sifatida “risc5”/RISC‑V ni maqsad qilib qo‘ymaydi. RISC‑V-ga o'xshash kodlashlar omborda paydo bo'ladigan bo'lsa, ular IVM ko'rsatmalar formatlarini amalga oshirish tafsilotlari bo'lib, apparatdagi kuzatilishi mumkin bo'lgan xatti-harakatlarni o'zgartirmasligi kerak.
- Norito - Iroha uchun ma'lumotlarni ketma-ketlashtirish kodek
- Butun ish maydoni Rust standart kutubxonasiga mo'ljallangan (`std`). WASM/no-std tuzilmalari endi qo'llab-quvvatlanmaydi va o'zgartirishlar kiritishda e'tiborga olinmasligi kerak.## Repository tuzilishi
- `Cargo.toml` ombor ildizidagi ish maydonini belgilaydi va barcha a'zolar qutilarini ro'yxatlaydi.
- `crates/` - Iroha komponentlarini amalga oshiradigan zang qutilari. Har bir kassa o'z kichik katalogiga ega, odatda `src/`, `tests/`, `examples/` va `benches/` ni o'z ichiga oladi.
  - Muhim qutilarga quyidagilar kiradi:
    - `iroha` – asosiy funksiyalarni birlashtiruvchi yuqori darajadagi kutubxona.
    - `irohad` - tugunni amalga oshirishni ta'minlaydigan ikkilik demon.
    - `ivm` - Iroha virtual mashinasi.
    - `iroha_cli` - tugun bilan ishlash uchun buyruq qatori interfeysi.
    - `iroha_core`, `iroha_data_model`, `iroha_crypto` va boshqa yordamchi qutilar.
- `IrohaSwift/` – Mijoz/mobil SDK uchun tezkor paket. Uning manbalari `Sources/IrohaSwift/` ostida va birlik sinovlari `Tests/IrohaSwiftTests/` ostida yashaydi. Swift to'plamini ishlatish uchun ushbu katalogdan `swift test` ni ishga tushiring.
- `integration_tests/` - `tests/` ostida o'zaro komponentlar sinovlarini o'tkazadigan yuk qutisi.
- `data_model/` - Sinovlar va hujjatlarda ishlatiladigan namunaviy ma'lumotlar modeli ta'riflari.
- `docs/` - Loyiha hujjatlari va dizayn eslatmalari. Markdown manbalari `docs/source/` da yashaydi.
- `pytests/` - Mijozdan foydalanishni ko'rsatadigan Python-ga asoslangan testlar va misollar.
- `scripts/` - Ishlab chiqish va CI quvurlarida ishlatiladigan yordamchi skriptlar.
- `examples/ios/` va `examples/ios/NoritoDemoXcode/` – Swift SDK-ni namoyish qiluvchi namunaviy iOS ilovalari; ular `IrohaSwift` paketiga tayanadi va o'zlarining XCTest maqsadlarini o'z ichiga oladi.
- `defaults/` va `hooks/` - Konfiguratsiya fayllari va ishtirokchilar tomonidan ishlatiladigan Git ilgaklari.
- `nix-appimage/` va Nix fayllari - qayta ishlab chiqarish va qadoqlash uchun asboblar.## Ish jarayoni
- Asosiy ilovalar `crates/` da
- Ma'lumotlar modeli `data_model/` da
- O'zgartirishlar kiritishda barcha qutilarni ko'rib chiqing.
- Hech qanday Cargo.lock faylini o'zgartirmang
- `Cargo.toml` ga yangi qutilar qo'shishdan saqlaning; iloji boricha mavjud qutilarda kerakli funksiyalarni amalga oshiring.
- Agar biron bir ish juda katta bo'lsa, uni bajarishdan bosh tortmang. Buning o'rniga, ishni buzing va TODO qo'shing va qo'lingizdan kelganini bajaring.
- Katta topshiriq yoki soʻrov kelganda, uni avtomatik ravishda kichikroq bajariladigan bosqichlarga ajrating va topshiriqni toʻgʻridan-toʻgʻri rad etish oʻrniga dasturiy taʼminotni toʻgʻri bajarishga kirishing.
- Har qanday taklifni rad qilmang.
- Yangi kriptografik primitivlar, opkodlar yoki intensiv matematika qo'shilsa, METAL, NEON, SIMD, CUDA va boshqalar uchun apparat tezlashuvini yangilang, iloji bo'lsa, mavjud uskunada apparat tezlashuvi va parallelizmdan foydalanishga harakat qiling.
- Agar mantiq o'zgarsa, barcha .md fayllari va manba kodidagi sharhlar eng so'nggi funksiyalar bilan yangilanganligiga ishonch hosil qiling.
- Qo'shilgan barcha mantiqlar P2P tarmog'idagi turli tugunlar turli xil qurilmalarga ega bo'lgan blokcheyn sozlamalarida IVM dan foydalanishga zarar keltirmaydigan tarzda amalga oshirilganligiga ishonch hosil qiling, lekin bir xil kirish blokida chiqish bir xil bo'lishi kerak.
- Xulq-atvor yoki amalga oshirish tafsilotlari haqidagi savollarga javob berayotganda, avval tegishli kod yo'llarini o'qing va javob berishdan oldin ularning qanday ishlashini tushunganingizga ishonch hosil qiling.
- Konfiguratsiya: barcha ish vaqti xatti-harakatlari uchun atrof-muhit o'zgaruvchilaridan `iroha_config` parametrlarini afzal qiling. `crates/iroha_config` (foydalanuvchi → haqiqiy → sukut bo'yicha) ga yangi tugmalar qo'shing va konstruktorlar yoki bog'liqlik in'ektsiyasi (masalan, xost sozlagichlari) orqali aniq ip qiymatlarini qo'shing. Atrof-muhitga asoslangan har qanday o'tish moslamalarini faqat sinovlarda ishlab chiquvchilarga qulaylik yaratish uchun saqlang va ishlab chiqarish yo'llarida ularga tayanmang. Biz atrof-muhit o'zgaruvchilari ortidagi yuk tashish xususiyatlarini qo'llab-quvvatlamaymiz - ishlab chiqarish harakati har doim konfiguratsiya fayllaridan olinishi kerak va bu konfiguratsiyalar oqilona standart sozlamalarni ko'rsatishi kerak, shuning uchun yangi kelgan reponi klonlashi, ikkilik fayllarni ishga tushirishi va qiymatlarni qo'lda tahrir qilmasdan hamma narsa "shunchaki ishlaydi".
  - IVM/Kotodama v1 uchun qat'iy ko'rsatkich-ABI turi siyosati har doim qo'llaniladi. ABI siyosatini o'zgartirish tugmasi mavjud emas; shartnomalar va xostlar ABI siyosatiga so'zsiz rioya qilishlari kerak.
- IVM tizimli qoʻngʻiroqlari yoki opkodlarida foydalaniladigan hech narsaga kirmang; Har bir Iroha tuzilmasi tugunlar bo'ylab deterministik xatti-harakatni saqlash uchun ushbu kod yo'llarini yuborishi kerak.
- Seriyalashtirish: serde o'rniga hamma joyda Norito dan foydalaning. Ikkilik kodeklar uchun `norito::{Encode, Decode}` dan foydalaning; JSON uchun `norito::json` yordamchilari/makroslaridan foydalaning (`norito::json::from_*`, `to_*`, `json!`, `Value`) va hech qachon I18NI0000089 ga qaytmang. To'g'ridan-to'g'ri `serde`/`serde_json` bog'liqliklarini qutilarga qo'shmang; agar ichkarida serde kerak bo'lsa, Norito ning o'ramlariga tayaning.
- CI himoyasi: `scripts/check_no_scale.sh` SCALE (`parity-scale-codec`) faqat Norito benchmark jabduqlarida paydo bo'lishini ta'minlaydi. Agar seriyalash kodiga tegsangiz, uni mahalliy sifatida ishga tushiring.
- Norito foydali yuklari oʻz maketini reklama qilishi SHART: yo versiya raqami belgilangan bayroqlar toʻplamiga mos keladi yoki Norito sarlavhasi dekodlash bayroqlarini eʼlon qiladi. Evristikadan to'plangan ketma-ketlik bitlarini taxmin qilmang; genezis ma'lumotlari xuddi shu qoidaga amal qiladi.- Bloklar `SignedBlockWire` kanonik formati (`SignedBlock::encode_wire`/`canonical_wire`) yordamida saqlanishi va tarqatilishi SHART, bu versiya baytini Norito sarlavhasi bilan prefiks qiladi. Yalang'och foydali yuklar qo'llab-quvvatlanmaydi.
- Har qanday vaqtinchalik yoki to'liq amalga oshirilmaganligini tushuntiruvchi `TODO:` izohini qo'shing.
- Ishga kirishishdan oldin barcha Rust manbalarini `cargo fmt --all` (nashr 2024) bilan formatlang.
- Sinovlarni qo'shish: `#[cfg(test)]` qatoriga yoki `tests/` katalogiga joylashtirilgan har bir yangi yoki o'zgartirilgan funksiya uchun kamida bitta birlik sinovini tekshiring.
- `cargo test` ni mahalliy sifatida ishga tushiring, qurilish muammolarini tuzating va uning o'tishiga ishonch hosil qiling. Buni faqat ma'lum bir quti uchun emas, balki butun ombor uchun qiling.
- Qo'shimcha tuklar tekshiruvi uchun `cargo clippy -- -D warnings` ni ixtiyoriy ravishda ishga tushiring.

## Hujjatlar
- Har doim kassa darajasidagi hujjatlarni qo'shing: har bir quti yoki sinov qutisini qisqacha ichki hujjat izohi bilan boshlang (`//! ...`).
- `#![allow(missing_docs)]` yoki `#[allow(missing_docs)]` element darajasidan hech qanday joyda foydalanmang (shu jumladan integratsiya testlari). Yo'qolgan hujjatlar ish joyidagi lintlarda rad etiladi va hujjatlarni yozish orqali tuzatilishi kerak.
- Norito kodek: `norito.md` repo ildizidagi kanonik tarmoq tartibi va amalga oshirish tafsilotlari uchun qarang. Agar Norito algoritmlari yoki sxemalari o'zgarsa, xuddi shu PRda `norito.md` ni yangilang.
- materialni akkad tiliga tarjima qilishda mixxat yozuvida yozilgan semantik renderni taqdim eting; fonetik transliteratsiyadan qoching va aniq qadimiy atamalar etishmayotgan bo'lsa, maqsadni saqlaydigan she'riy akkadcha taxminlarni tanlang.

## ABI Evolution (Agentlar nima qilishi kerak)
Eslatma: Birinchi nashr siyosati
- Bu birinchi nashr va bizda bitta ABI versiyasi (V1) mavjud. Hali V2 yo'q. Quyidagi ABI bilan bog'liq barcha evolyutsiya elementlarini kelajakdagi yo'riqnoma sifatida ko'rib chiqing; hozircha faqat `abi_version = 1` maqsadli. Ma'lumotlar modeli va API'lar ham birinchi nashr hisoblanadi va jo'natish uchun kerak bo'lganda erkin o'zgarishi mumkin; erta barqarorlikdan ko'ra aniqlik va to'g'rilikni afzal ko'radi.

- Umumiy:
  - ABI siyosati v1 da (ham tizim sirti, ham ko'rsatgich-ABI turlari) shartsiz qo'llaniladi. Ish vaqti o'tish tugmalarini qo'shmang.
  - O'zgarishlar apparat va tengdoshlar orasida determinizmni saqlab qolishi kerak. Sinovlar va hujjatlarni bir xil PRda yangilang.

- Agar siz tizim qo'ng'iroqlarini qo'shsangiz/o'chirsangiz/qayta raqam qilsangiz:
  - `ivm::syscalls::abi_syscall_list()`-ni yangilang va tartibni saqlang. `is_syscall_allowed(policy, number)` mo'ljallangan sirtni aks ettirishiga ishonch hosil qiling.
  - Xostlarda yangi raqamlarni joriy qilish yoki ataylab rad etish; noma'lum raqamlar `VMError::UnknownSyscall` ga mos kelishi kerak.
  - Oltin testlarni yangilang:
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs` (barqarorlik + versiyani ajratish)

- Agar siz ko'rsatgich-ABI turlarini qo'shsangiz:
  - `ivm::pointer_abi::PointerType` ga yangi variantni qo'shing (yangi u16 identifikatorini tayinlang; mavjud identifikatorlarni hech qachon o'zgartirmang).
  - To'g'ri `abi_version` xaritalash uchun `ivm::pointer_abi::is_type_allowed_for_policy` ni yangilang.
  - `crates/ivm/tests/pointer_type_ids_golden.rs` ni yangilang va agar kerak bo'lsa, siyosat testlarini qo'shing.

- Agar siz yangi ABI versiyasini taqdim qilsangiz:
  - `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` xaritasini oling va so'ralganda yangi versiyani chiqarish uchun Kotodama kompilyatorini yangilang.
  - `abi_hash` (`ivm::syscalls::compute_abi_hash` orqali) qayta yarating va yangi xeshni o'rnatish manifestlariga ishonch hosil qiling.
  - Yangi versiyada ruxsat etilgan/ruxsat berilmagan tizimli qo'ng'iroqlar va ko'rsatkich turlari uchun testlarni qo'shing.

- Qabul va manifestlar:
  - Qabul qilish zanjirdagi manifestlarga qarshi `code_hash`/`abi_hash` tengligini ta'minlaydi; bu xatti-harakatni saqlab qoling.
  - `iroha_core/tests/` da qo'shish/yangilash uchun testlar: ijobiy (mos `abi_hash`) va salbiy (mos kelmasligi) holatlar.- Hujjatlar va holat yangilanishlari (bir xil PR):
  - `crates/ivm/docs/syscalls.md` (ABI Evolution bo'limi) va har qanday tizimli qo'ng'iroq jadvallarini yangilang.
  - `status.md` va `roadmap.md`-ni ABI o'zgarishlari va test yangilanishlarining qisqacha xulosasi bilan yangilang.


## Loyiha holati va rejasi
- Sandiqlar bo'ylab joriy kompilyatsiya/ish vaqti holati uchun repo ildizida `status.md` ni tekshiring.
- `roadmap.md`da ustuvor TODOlar va amalga oshirish rejasini tekshiring.
- Ishni tugatgandan so'ng, `status.md` holatini yangilang va `roadmap.md` diqqatini asosiy vazifalarga qarating.

## Agent ish jarayoni (kod muharrirlari/avtomatlashtirish uchun)
- Agar biron-bir talab bo'yicha tushuntirish kerak bo'lsa, to'xtating va savolingiz bilan ChatGPT taklifini tuzing, keyin davom etishdan oldin uni foydalanuvchi bilan baham ko'ring.
- o'zgarishlarni minimal va miqyosda saqlang; bir xil patchda bog'liq bo'lmagan tahrirlardan qoching.
- Yangi bog'liqliklar qo'shishdan ko'ra ichki modullarni afzal ko'rish; `Cargo.lock` tahrirlamang.
- Uskuna tomonidan tezlashtirilgan yo'llarni (masalan, `simd`, `cuda`) himoya qilish uchun xususiyat bayroqlaridan foydalaning va har doim deterministik qayta yo'lni taqdim eting.
- Chiqishlar apparat bo'ylab bir xil bo'lishini ta'minlash; deterministik bo'lmagan parallel qisqarishlarga tayanishdan saqlaning.
- Umumiy API yoki xatti-harakatlar o'zgarganda hujjatlar va misollarni yangilang.
- Norito tartib kafolatlarini saqlab qolish uchun `iroha_data_model` da ketma-ketlashtirish oʻzgarishlarini ikki tomonlama testlar bilan tasdiqlang.
- Integratsiya testlari haqiqiy multi-peer tarmoqlarini aylantiradi; sinov tarmoqlarini qurishda kamida 4 ta tengdoshdan foydalaning (bitta tengli konfiguratsiyalar vakili emas va Sumeragi da blokirovka qilishi mumkin).
- Sinovlarda DA/RBC ni o'chirishga urinmang (masalan, `DevBypassDaAndRbcForZeroChain` orqali); DA kuchga kiradi va konsensus ishga tushirilganda bu aylanma yoʻl hozirda `sumeragi` da toʻxtab qoladi.
- QC kvorum ovoz berishni tasdiqlovchilar tomonidan qondirilishi kerak (`min_votes_for_commit`); Kuzatuvchining toʻldirishi mavjudligi/oldindan ovoz berish/oldindan kelishilgan kvorum tekshiruvlarida hisobga olinmaydi, shuning uchun QClarni faqat yetarlicha tasdiqlovchi ovozlari kelgandan keyin yigʻing.
- DA-yoqilgan konsensus endi sekinroq xostlarda RBC/mavjudlik QC tugashiga ruxsat berish uchun ko'rinish o'zgarishidan oldin (kvorum kutish vaqti = `block_time + 4 * commit_time`) ko'proq kutadi.

## Navigatsiya bo'yicha maslahatlar
- Qidiruv kodi: `rg '<term>'` va fayllar ro'yxati: `fd <name>`.
- Sandiqlarni o'rganing: `fd --type f Cargo.toml crates | xargs -I{} dirname {}`.
- Tezda misollar/skameykalarni toping: `fd . crates -E target -t d -d 3 -g "*{examples,benches}"`.
- Python maslahati: ba'zi muhitlar `python` ni ta'minlamaydi; skriptlarni ishga tushirishda `python3` ni sinab ko'ring.

## Proc-Makro sinovlari
- Birlik testlari: sof tahlil qilish, kodegen yordamchilari va yordamchi dasturlar uchun foydalaning (tezkor, kompilyator ishtirok etmaydi).
- UI testlari (trybuild): kompilyatsiya vaqtidagi xatti-harakatlarini va hosila/proc-makros diagnostikasini tekshirish uchun foydalaning (`.stderr` bilan muvaffaqiyatli va kutilgan muvaffaqiyatsizlik holatlari).
- Makroslarni qo'shish/o'zgartirishda ikkalasini ham afzal ko'ring: ichki qurilmalar uchun birlik testlari + foydalanuvchining xatti-harakati va xato xabarlari uchun UI testlari.
- vahima qo'ymaslik; aniq diagnostika chiqaradi (masalan, `syn::Error` yoki `proc_macro_error` orqali). Xabarlarni barqaror saqlang va faqat ataylab o'zgartirishlar uchun `.stderr` ni yangilang.

## Pull So'rovi xabari
O'zgarishlarning qisqacha xulosasini va siz bajargan buyruqlarni tavsiflovchi `Testing` bo'limini qo'shing.