---
lang: uz
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-12-29T18:16:35.916241+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Avtomatlashtirish agentini bajarish bo'yicha qo'llanma

Ushbu sahifada har qanday avtomatlashtirish agenti uchun operatsion to'siqlar jamlangan
Hyperledger Iroha ish maydoni ichida ishlash. U kanonikni aks ettiradi
`AGENTS.md` yo'riqnomasi va yo'l xaritasi ma'lumotnomalari shuning uchun tuzing, hujjatlang va
telemetriya o'zgarishlari inson tomonidan ishlab chiqarilganmi yoki yo'qmi bir xil ko'rinadi
avtomatlashtirilgan hissa qo'shuvchi.

Har bir topshiriq deterministik kodni va tegishli hujjatlarni, testlarni,
va operatsion dalillar. Quyidagi bo'limlarni oldindan tayyor havola sifatida ko'rib chiqing
`roadmap.md` elementlariga tegish yoki xatti-harakatlarga oid savollarga javob berish.

## Tez boshlash buyruqlari

| Harakat | Buyruq |
|--------|---------|
| Ish joyini yaratish | `cargo build --workspace` |
| To'liq test to'plamini ishga tushiring | `cargo test --workspace` *(odatda bir necha soat davom etadi)* |
| Sukut bo'yicha rad etish ogohlantirishlari bilan klipni ishga tushiring | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust kodini formatlash | `cargo fmt --all` *(nashr 2024)* |
| Bitta qutini sinab ko'ring | `cargo test -p <crate>` |
| Bitta testni bajaring | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK testlari | `IrohaSwift/` dan, `swift test` |

## Ish jarayoni asoslari

- Savollarga javob berishdan yoki mantiqni o'zgartirishdan oldin tegishli kod yo'llarini o'qing.
- Katta yo'l xaritasi ob'ektlarini qat'iy majburiyatlarga bo'lish; ishni hech qachon rad etmang.
- Mavjud ish maydoni a'zoligi ichida qoling, ichki qutilarni qayta ishlating va bajaring
  **aniq ko'rsatma bo'lmasa** `Cargo.lock` ni o'zgartirmang.
- Xususiyatlar bayroqlari va qobiliyat o'tish-o'tishlarini faqat apparat tomonidan belgilangan hollarda foydalaning
  tezlatgichlar; har bir platformada mavjud bo'lgan deterministik zaxiralarni saqlang.
- Har qanday funktsional o'zgarishlar bilan bir qatorda hujjatlar va Markdown havolalarini yangilang
  shuning uchun hujjatlar har doim hozirgi xatti-harakatni tasvirlaydi.
- Har bir yangi yoki o'zgartirilgan funksiya uchun kamida bitta birlik testini qo'shing. Inlineni afzal ko'ring
  Qo'llanish doirasiga qarab `#[cfg(test)]` modullari yoki sandiqning `tests/` papkasi.
- Ishni tugatgandan so'ng, `status.md`-ni qisqacha xulosa va ma'lumotnoma bilan yangilang
  tegishli fayllar; `roadmap.md`ni hali ham ishlashga muhtoj bo'lgan narsalarga qarating.

## Amalga oshirish to'siqlari

### Seriyalashtirish va ma'lumotlar modellari
- Hamma joyda Norito kodekidan foydalaning (`norito::{Encode, Decode}` orqali ikkilik,
  JSON `norito::json::*` orqali). To'g'ridan-to'g'ri serde/`serde_json` foydalanishni qo'shmang.
- Norito foydali yuklari o'z tartibini reklama qilishi kerak (versiya bayti yoki sarlavha bayroqlari),
  va yangi formatlar tegishli hujjatlar yangilanishini talab qiladi (masalan,
  `norito.md`, `docs/source/da/*.md`).
- Ibtido ma'lumotlari, manifestlar va tarmoq yuklari deterministik bo'lib qolishi kerak
  shuning uchun bir xil kirishga ega bo'lgan ikkita tengdosh bir xil xeshlarni ishlab chiqaradi.

### Konfiguratsiya va ish vaqti harakati
- Yangi muhit o'zgaruvchilaridan `crates/iroha_config` da yashovchi tugmalarni afzal ko'ring.
  Qiymatlarni konstruktorlar yoki qaramlik kiritish orqali aniq o'tkazing.
- Hech qachon IVM tizim qoʻngʻiroqlarini yoki opcode xatti-harakatini oʻtkazmang — ABI v1 hamma joyda yetkazib beriladi.
- Yangi konfiguratsiya opsiyalari qo'shilganda, standart sozlamalarni, hujjatlarni va shunga o'xshashlarni yangilang
  andozalar (`peer.template.toml`, `docs/source/configuration*.md` va boshqalar).### ABI, Syscalls va Pointer turlari
- ABI siyosatiga shartsiz munosabatda bo'ling. Tizimli qo'ng'iroqlar yoki ko'rsatkich turlarini qo'shish/o'chirish
  yangilashni talab qiladi:
  - `ivm::syscalls::abi_syscall_list` va `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` va oltin testlar
  - `crates/ivm/tests/abi_hash_versions.rs` ABI xeshi har doim o'zgarganda
- Noma'lum tizim qo'ng'iroqlari `VMError::UnknownSyscall` ga mos kelishi va manifestlar bo'lishi kerak
  kirish testlarida imzolangan `abi_hash` tenglik tekshiruvlarini saqlab qoling.

### Uskuna tezlashuvi va determinizmi
- Yangi kriptografik primitivlar yoki og'ir matematika apparat tezlashtirilgan holda jo'natilishi kerak
  yo'llar (METAL / NEON / SIMD / CUDA) deterministik zaxiralarni saqlagan holda.
- deterministik bo'lmagan parallel qisqartirishdan saqlaning; ustuvorlik - bir xil chiqishlar
  har bir tengdosh, hatto apparat farqli bo'lsa ham.
- Norito va FASTPQ moslamalarini qayta ishlab chiqarish imkoniyatini saqlang, shunda SRE butun flotni tekshira oladi.
  telemetriya.

### Hujjatlar va dalillar
- Portalda (`docs/portal/...`) har qanday ommaviy hujjat o'zgarishini aks ettiring.
  Hujjatlar sayti Markdown manbalari bilan dolzarb bo'lib qolishi uchun qo'llaniladi.
- Yangi ish oqimlari kiritilganda, ish kitoblari, boshqaruv eslatmalari yoki qo'shing
  dalillarni qayta ishlash, qaytarish va qo'lga kiritish usullarini tushuntiruvchi nazorat ro'yxatlari.
- Kontentni akkad tiliga tarjima qilganda, yozilgan semantik renderlarni taqdim eting
  fonetik transliteratsiyadan ko'ra mixxat yozuvida.

### Sinov va asboblarni kutish
- Mahalliy ravishda tegishli test to'plamlarini ishga tushiring (`cargo test`, `swift test`,
  integratsiya jabduqlar) va PR test bo'limidagi buyruqlarni hujjatlashtiring.
- CI himoyasi skriptlari (`ci/*.sh`) va asboblar panelini yangi telemetriya bilan sinxronlashtiring.
- Proc-makroslar uchun diagnostikani bloklash uchun `trybuild` UI testlari bilan birlik testlarini ulang.

## Yetkazib berishga tayyor nazorat roʻyxati

1. Kod kompilyatsiya qilinadi va `cargo fmt` hech qanday farq yaratmadi.
2. Yangilangan hujjatlar (ish maydoni Markdown plus portal oynalari) yangisini tavsiflaydi
   xatti-harakatlari, yangi CLI bayroqlari yoki konfiguratsiya tugmalari.
3. Sinovlar har bir yangi kod yo'lini qamrab oladi va regressiyalar paytida aniq muvaffaqiyatsizlikka uchraydi
   paydo bo'ladi.
4. Telemetriya, asboblar paneli va ogohlantirish ta'riflari har qanday yangi ko'rsatkichlarga yoki
   xato kodlari.
5. `status.md` tegishli fayllarga havola qilingan qisqacha xulosani o'z ichiga oladi va
   yo'l xaritasi bo'limi.

Ushbu nazorat ro'yxatiga rioya qilish yo'l xaritasi bajarilishini tekshirilishini ta'minlaydi va har bir narsani ta'minlaydi
agent boshqa jamoalar ishonishi mumkin bo'lgan dalillarni taqdim etadi.