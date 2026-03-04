---
lang: uz
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T19:17:13.236818+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM arxitektura refaktor rejasi

Ushbu reja Iroha virtual mashinasini qayta shakllantirish uchun qisqa muddatli bosqichlarni o'z ichiga oladi.
(IVM) xavfsizlik va ishlash xususiyatlarini saqlab, aniqroq qatlamlarga.
U mas'uliyatni ajratishga, xost integratsiyasini xavfsizroq qilishga va
Kotodama til to'plamini mustaqil qutiga chiqarish uchun tayyorlash.

## Maqsadlar

1. **Qatlamli ish vaqti fasad** – VM uchun aniq ish vaqti interfeysini joriy qiling
   yadro tor belgi orqasida joylashtirilishi mumkin va muqobil front-uchlar rivojlanishi mumkin
   ichki modullarga tegmasdan.
2. **Xost/tizim qo‘ng‘irog‘i chegarasini mustahkamlash** – tizim qo‘ng‘irog‘i orqali marshrutni jo‘natish
   har qanday xostdan oldin ABI siyosati va ko'rsatgichni tekshirishni amalga oshiradigan maxsus adapter
   kod bajariladi.
3. **Til/asboblarni ajratish** – Kotodama maxsus kodini yangi qutiga o'tkazing va
   `ivm` da faqat bayt-kodni bajarish yuzasini saqlang.
4. **Konfiguratsiya uyg‘unligi** – tezlashtirish va funksiya almashuvlarini bir xil bo‘lishi uchun birlashtiradi
   `iroha_config` orqali boshqariladi, ishlab chiqarishda atrof-muhitga asoslangan tugmachalarni olib tashlaydi
   yo'llar.

## Fazalarni ajratish

### 1-bosqich – Fasadning ish vaqti (davom etmoqda)
- Hayotiy tsiklni tavsiflovchi `VmEngine` xususiyatini belgilaydigan `runtime` modulini qo'shing
  operatsiyalar (`load_program`, `execute`, asosiy sanitariya-tesisat).
- `IVM` xususiyatini amalga oshirishga o'rgating.  Bu mavjud tuzilmani saqlaydi, lekin ruxsat beradi
  iste'molchilar (va kelajakdagi testlar) beton o'rniga interfeysga bog'liq
  turlari.
- `lib.rs` dan to'g'ridan-to'g'ri modulni qayta eksport qilishni boshlang, shunda qo'ng'iroq qiluvchilar ushbu orqali import qilinadi.
  iloji bo'lsa, fasad.

**Xavfsizlik / ishlash ta'siri**: Fasad ichki qismga to'g'ridan-to'g'ri kirishni cheklaydi
davlat; faqat xavfsiz kirish nuqtalari ochiladi.  Bu xostni tekshirishni osonlashtiradi
o'zaro ta'sirlar va gaz yoki TLV bilan ishlash sabablari.

### 2-bosqich - Syscall dispetcheri
- `SyscallDispatcher` komponentini kiriting, u `IVMHost`ni o'rab oladi va ABIni qo'llaydi.
  siyosat va ko'rsatgichni bir marta, bir joyda tekshirish.
- Dispetcherdan foydalanish uchun standart xost va soxta xostlarni ko'chiring, olib tashlang
  takrorlangan tekshirish mantig'i.
- Dispetcherni ulanadigan qilib qo'ying, shunda xostlar moslashtirilgan asboblarsiz yetkazib bera oladi
  xavfsizlik tekshiruvlarini chetlab o'tish.
- `SyscallDispatcher::shared(...)` yordamchisini taqdim eting, shunda klonlangan VMlar yo'naltirilishi mumkin
  har bir ishchi binosisiz umumiy `Arc<Mutex<..>>` xost orqali tizim chaqiruvlari
  buyurtma bo'yicha o'ramlar.

**Xavfsizlik / ishlash ta'siri**: Markazlashtirilgan eshiklar xostlardan himoya qiladi
`is_syscall_allowed` ga qo'ng'iroq qilishni unuting va bu kelajakda ko'rsatgichni keshlash imkonini beradi
takroriy tizim chaqiruvlari uchun tekshiruvlar.

### 3-bosqich - Kotodama ekstraktsiyasi
- Kotodama kompilyatori `crates/kotodama_lang` ga chiqarilgan (`crates/ivm/src/kotodama` dan).
- VM iste'mol qiladigan minimal bayt-kod API-ni taqdim eting (`compile_to_ivm_bytecode`).

**Xavfsizlik / ishlash ta'siri**: ajratish VMning hujum yuzasini pasaytiradi
asosiy va tarjimon regressiyalarini xavf ostiga qo'ymasdan til innovatsiyasiga imkon beradi.### 4-bosqich - Konfiguratsiyani birlashtirish
- `iroha_config` oldindan o'rnatilgan (masalan, GPU orqa uchlarini yoqish) orqali ipni tezlashtirish opsiyalari, mavjud muhitni bekor qilish (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) ish vaqtini o'chirish kalitlari sifatida saqlangan.
- `RuntimeConfig` ob'ektini yangi fasad orqali ko'rsating, shunda xostlar tanlashadi
  aniq deterministik tezlashtirish siyosati.

**Xavfsizlik / ishlash ta'siri**: Env-ga asoslangan o'zgartirishlarni yo'q qilish jim turishdan qochadi
konfiguratsiya o'zgarishi va joylashtirishlar bo'ylab deterministik xatti-harakatni ta'minlaydi.

## Darhol keyingi qadamlar

- Fasad xususiyatini qo'shish va yuqori darajadagi qo'ng'iroqlar saytlarini yangilash orqali 1-bosqichni yakunlang
  unga bog'liq.
- Ommaviy reeksportlarni faqat fasad va ataylab ommaviy APIlarni ta'minlash uchun audit qiling
  qutidan oqib chiqish.
- Syscall dispetcher API prototipini alohida modulda yarating va uni ko'chiring
  bir marta tasdiqlangan standart xost.

Har bir bosqich bo'yicha taraqqiyot amalga oshirilgandan so'ng `status.md` da kuzatiladi
davom etmoqda.