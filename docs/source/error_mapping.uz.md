---
lang: uz
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-11T04:52:11.136647+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Xatolarni xaritalash bo'yicha qo'llanma

Oxirgi yangilangan: 2025-08-21

Ushbu qo'llanmada Iroha da umumiy nosozlik rejimlari ma'lumotlar modeli tomonidan yuzaga keladigan barqaror xato toifalari bilan taqqoslanadi. Sinovlarni loyihalash va mijoz xatolarini oldindan aytib bo'ladigan qilish uchun undan foydalaning.

Prinsiplar
- Ko'rsatma va so'rovlar yo'llari tuzilgan raqamlarni chiqaradi. Vahimalardan saqlaning; iloji bo'lsa, ma'lum bir toifa haqida xabar bering.
- Turkumlar barqaror, xabarlar rivojlanishi mumkin. Mijozlar erkin shakldagi satrlarda emas, balki toifalar bo'yicha mos kelishi kerak.

Kategoriyalar
- InstructionExecutionError::Find: ob'ekt etishmayotgan (aktiv, hisob, domen, NFT, rol, trigger, ruxsat, ochiq kalit, blok, tranzaksiya). Misol: mavjud bo'lmagan metama'lumotlar kalitini olib tashlash Find (MetadataKey) ni beradi.
- InstructionExecutionError::Repetition: dublikat ro'yxatga olish yoki ziddiyatli ID. Ko'rsatma turini va takroriy IdBoxni o'z ichiga oladi.
- InstructionExecutionError::Mintability: Mintability invariant buzilgan (`Once` ikki marta charchagan, `Limited(n)` haddan tashqari chizilgan yoki `Infinitely`ni o‘chirishga urinish). Misollar: `Once` sifatida belgilangan aktivni ikki marta zarb qilish `Mintability(MintUnmintable)` daromad keltiradi; `Limited(0)` konfiguratsiyasi `Mintability(InvalidMintabilityTokens)` hosil qiladi.
- InstructionExecutionError::Math: Raqamli domen xatolari (toshib ketish, nolga boʻlish, manfiy qiymat, miqdor yetarli emas). Misol: mavjud miqdordan ko'proq yoqish Math(NotEnoughQuantity) beradi.
- InstructionExecutionError::InvalidParameter: Yo'riqnoma parametri yoki konfiguratsiyasi noto'g'ri (masalan, o'tmishdagi vaqtni ishga tushirish). Noto'g'ri tuzilgan shartnoma yuklamalari uchun foydalaning.
- InstructionExecutionError::Evaluate: ko'rsatma shakli yoki turlari uchun DSL/spec nomuvofiqligi. Misol: aktiv qiymatining noto‘g‘ri raqamli spetsifikatsiyasi Evaluate(Type(AssetNumericSpec(...))).
- InstructionExecutionError::InvariantViolation: Boshqa toifalarda ifodalab bo'lmaydigan tizim invariantining buzilishi. Misol: oxirgi imzo qo'ygan shaxsni olib tashlashga urinish.
- InstructionExecutionError::Query: Ko‘rsatmani bajarish vaqtida so‘rov bajarilmasa, QueryExecutionFailni o‘rash.

QueryExecutionFail
- Topish: so'rov kontekstida etishmayotgan ob'ekt.
- Konvertatsiya: so'rov tomonidan kutilgan noto'g'ri tur.
- Topilmadi: jonli so'rov kursori yo'q.
- CursorMismatch / CursorDone: Kursor protokoli xatolari.
- FetchSizeTooBig: Server tomonidan belgilangan chegaradan oshib ketdi.
- GasBudgetExceeded: so'rovning bajarilishi gaz/materializatsiya byudjetidan oshdi.
- InvalidSingularParameters: Singular so'rovlar uchun qo'llab-quvvatlanmaydigan parametrlar.
- CapacityLimit: jonli so'rovlar saqlash hajmiga erishildi.

Sinov bo'yicha maslahatlar
- Xatoning kelib chiqishiga yaqin birlik testlariga ustunlik bering. Masalan, ob'ektning raqamli spetsifikatsiyasi mos kelmasligi ma'lumotlar modeli testlarida yaratilishi mumkin.
- Integratsiya testlari vakillik holatlari (masalan, dublikat registr, o'chirishda etishmayotgan kalit, egaliksiz o'tkazish) uchun uchdan-uchga xaritalashni qamrab olishi kerak.
- Xabar pastki satrlari o'rniga enum variantlarini moslashtirish orqali tasdiqlarni mustahkam saqlang.