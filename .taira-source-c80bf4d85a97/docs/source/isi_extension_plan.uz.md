<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# ISI kengaytirish rejasi (v1)

Ushbu eslatma yangi Iroha maxsus ko'rsatmalari va suratga olish uchun ustuvorlik tartibini tasdiqlaydi.
amalga oshirishdan oldin har bir ko'rsatma uchun muhokama qilinmaydigan invariantlar. Buyurtma mos keladi
birinchi navbatda xavfsizlik va ishlash xavfi, ikkinchidan UX o'tkazish qobiliyati.

## Priority Stack

1. **RotateAccountSignatory** – buzg‘unchi migratsiyalarsiz kalitlarni gigienik aylantirish uchun talab qilinadi.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – Deterministik shartnomani taqdim eting
   o'chirish kalitlari va buzilgan joylashtirishlar uchun saqlashni qayta tiklash.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – Metadata paritetini aniq aktivga kengaytiring
   muvozanatni saqlaydi, shuning uchun kuzatuvchanlik vositalari xoldinglarni belgilashi mumkin.
4. **BatchMintAsset** / **BatchTransferAsset** – yuk hajmini saqlab qolish uchun aniq fan-out yordamchilari
   va VM orqaga qaytish bosimini boshqarish mumkin.

## Ko'rsatmalarning o'zgarmasligi

### SetAssetKeyValue / RemoveAssetKeyValue
- `AssetMetadataKey` nom maydonidan (`state.rs`) qayta foydalaning, shuning uchun kanonik WSV kalitlari barqaror qoladi.
- JSON o'lchami va sxemasi chegaralarini hisob metadata yordamchilari bilan bir xil tarzda qo'llang.
- Emit `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` ta'sirlangan `AssetId` bilan.
- Mavjud aktiv meta-ma'lumotlarini tahrirlash bilan bir xil ruxsat tokenlarini talab qiling (ta'rif egasi OR
  `CanModifyAssetMetadata` uslubidagi grantlar).
- Agar aktiv yozuvi yo'q bo'lsa, to'xtating (so'zsiz yaratilmaydi).### RotateAccount Signatory
- `AccountId` da imzolovchining atom almashinuvi, shu bilan birga hisob meta-ma'lumotlari va bog'langan
  resurslar (aktivlar, triggerlar, rollar, ruxsatlar, kutilayotgan voqealar).
- Joriy imzo qo'ygan shaxs qo'ng'iroq qiluvchiga (yoki aniq token orqali berilgan vakolatga) mos kelishini tekshiring.
- Agar yangi ochiq kalit boshqa kanonik hisobni qo'llab-quvvatlasa, rad eting.
- Qabul qilishdan oldin hisob identifikatorini o'rnatadigan va keshlarni bekor qiladigan barcha kanonik kalitlarni yangilang.
- Audit izlari uchun eski/yangi kalitlarga ega maxsus `AccountEvent::SignatoryRotated` chiqaring.
- Migratsiya iskala: `AccountAlias` + `AccountRekeyRecord` ga tayaning (qarang: `account::rekey`)
  mavjud hisoblar xesh tanaffuslarsiz yangilanish paytida barqaror taxallus bog'lashlarini saqlab qolishi mumkin.

### Shartnoma misolini o'chirish
- `(namespace, contract_id)` bog'lanishini olib tashlang yoki qabr toshini toshga qo'ying, shu bilan birga kelib chiqish ma'lumotlarini saqlang
  (kim, qachon, sabab kodi) muammolarni bartaraf etish uchun.
- Ruxsat bermaslik uchun siyosat ilgaklari bilan faollashtirish bilan bir xil boshqaruv ruxsatini talab qiling
  yuqori ruxsatsiz asosiy tizim nom maydonlarini o'chirish.
- Hodisa jurnallarini deterministik saqlash uchun misol allaqachon faol bo'lmaganda rad eting.
- Quyi oqim kuzatuvchilari iste'mol qilishi mumkin bo'lgan `ContractInstanceEvent::Deactivated` chiqaring.### SmartContractBytesni olib tashlang
- `code_hash` tomonidan saqlangan bayt kodini faqat manifest yoki faol misollar bo'lmaganda kesishga ruxsat bering
  artefaktga murojaat qilish; aks holda tavsiflovchi xato bilan muvaffaqiyatsizlikka uchraydi.
- Ruxsat eshigini ro'yxatdan o'tkazish (`CanRegisterSmartContractCode`) va operator darajasida
  qo'riqchi (masalan, `CanManageSmartContractStorage`).
- Taqdim etilgan `code_hash` o'chirishdan oldin saqlangan tana hazm qilishiga mos kelishini tekshiring.
  eskirgan tutqichlar.
- `ContractCodeEvent::Removed`-ni xesh va qo'ng'iroq qiluvchi metama'lumotlari bilan chiqaring.

### BatchMintAsset / BatchTransferAsset
- Hammasi yoki hech narsa semantikasi: yo har bir kortej muvaffaqiyatli bo'ladi yoki ko'rsatma yon tomonlarsiz bekor qilinadi
  effektlar.
- Kirish vektorlari aniq tartiblangan bo'lishi kerak (to'liq tartiblash yo'q) va konfiguratsiya bilan chegaralangan bo'lishi kerak
  (`max_batch_isi_items`).
- Har bir ob'ekt bo'yicha aktiv hodisalarini emissiya qilish, shuning uchun quyi oqim buxgalteriya hisobi izchil bo'lib qoladi; to'plam konteksti qo'shimcha,
  almashtirish emas.
- Ruxsat tekshiruvlari har bir maqsad uchun mavjud bitta elementli mantiqni qayta ishlatadi (aktiv egasi, ta'rif egasi,
  yoki berilgan qobiliyat) holat mutatsiyasidan oldin.
- Maslahat kirish to'plamlari optimistik parallellikni to'g'ri saqlash uchun barcha o'qish/yozish kalitlarini birlashtirishi kerak.

## Amalga oshirish iskala- Ma'lumotlar modeli endi balans metama'lumotlari uchun `SetAssetKeyValue` / `RemoveAssetKeyValue` skafoldlarini olib yuradi
  tahrirlar (`transparent.rs`).
- Ijrochi tashrif buyuruvchilar simli erlarni mezbonlik qilgandan so'ng ruxsatnomalarni o'tkazadigan joy egalarini ko'rsatadi
  (`default/mod.rs`).
- Rekey prototip turlari (`account::rekey`) aylanma migratsiya uchun qo'nish zonasini ta'minlaydi.
- Dunyo holati `AccountAlias` tomonidan kalitlangan `account_rekey_records` ni o'z ichiga oladi, shuning uchun biz taxallusni sahnalashtira olamiz →
  tarixiy `AccountId` kodlashiga tegmasdan imzolangan migratsiya.

## IVM Syscall loyihasini tuzish

- `DeactivateContractInstance` / `RemoveSmartContractBytes` uchun xost shimlari
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) va
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), ikkalasi ham Norito TLV larni aks ettiradi
  kanonik ISI tuzilmalari.
- `abi_syscall_list()` ni faqat xost ishlovchilari `iroha_core` ijro yoʻllarini aks ettirgandan soʻng kengaytiring.
  Rivojlanish jarayonida ABI xeshlari barqaror.
- Kotodama ni yangilang, tizim qo'ng'irog'i raqamlari barqarorlashgandan so'ng pasaytirish; kengaytirilgan uchun oltin qoplama qo'shing
  bir vaqtning o'zida sirt.

## Holat

Yuqoridagi tartib va invariantlar amalga oshirishga tayyor. Kuzatuv filiallari murojaat qilishi kerak
ijro yo'llari va tizim ta'sirini ulashda ushbu hujjat.