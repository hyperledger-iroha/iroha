---
lang: uz
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ Maʼlumotlar modeli ⇄ Kotodama — Hizalanishni koʻrib chiqish

Ushbu hujjat Iroha Virtual Mashina (IVM) ko'rsatmalar to'plami va Iroha Maxsus Yo'riqnomalari (ISI) va `iroha_data_model` bilan tizimli qo'ng'iroq yuzasi xaritasi va Kotodama bilan qanday tuzilganligini tekshiradi. U mavjud bo'shliqlarni aniqlaydi va aniq yaxshilanishlarni taklif qiladi, shuning uchun to'rtta qatlam deterministik va ergonomik jihatdan bir-biriga mos keladi.

Bayt-kod maqsadi haqida eslatma: Kotodama aqlli kontraktlari Iroha Virtual Mashina (IVM) baytekodiga (`.to`) kompilyatsiya qilinadi. Ular “risc5”/RISC‑V-ni mustaqil arxitektura sifatida maqsad qilib qo‘ymaydi. Bu yerda havola qilingan har qanday RISC‑V-ga o‘xshash kodlashlar IVM aralash ko‘rsatmalar formatining bir qismi bo‘lib, amalga oshirish detali bo‘lib qoladi.

## Qo'llanish doirasi va manbalari
- IVM: `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` va `crates/ivm/docs/*`.
- ISI/Ma'lumotlar modeli: `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*` va `docs/source/data_model_and_isi_spec.md` hujjatlari.
- Kotodama: `crates/kotodama_lang/src/*`, `crates/ivm/docs/*` da hujjatlar.
- Asosiy integratsiya: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminologiya
- “ISI” ijrochi orqali dunyo holatini o‘zgartiruvchi o‘rnatilgan ko‘rsatmalar turlarini bildiradi (masalan, RegisterAccount, Mint, Transfer).
- “Syscall” 8-bitli raqamga ega IVM `SCALL` ga tegishli boʻlib, buxgalteriya hisobi operatsiyalari uchun xostga topshiradi.

---

## Joriy xaritalash (amalga oshirilganidek)

### IVM Ko'rsatmalar
- Arifmetik, xotira, boshqaruv oqimi, kripto, vektor va ZK yordamchilari `instruction.rs` da aniqlangan va `ivm.rs` da amalga oshirilgan. Bular mustaqil va deterministikdir; tezlashtirish yo'llari (SIMD/Metal/CUDA) protsessorning zaxiralariga ega.
- Tizim/xost chegarasi `SCALL` (opcode 0x60) orqali. Raqamlar `syscalls.rs` ro'yxatida keltirilgan va dunyo operatsiyalarini (ro'yxatdan o'tkazish/ro'yxatdan o'chirish domen/hisob/aktiv, zarb/yozib/o'tkazish, rol/ruxsat operatsiyalari, triggerlar) va yordamchilarni (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, I106080, va hokazo) o'z ichiga oladi.

### Xost qatlami
- `IVMHost::syscall(number, &mut IVM)` xususiyati `host.rs` da yashaydi.
- DefaultHost faqat hisobdan tashqari yordamchilarni qo'llaydi (ajratish, yig'ish o'sishi, kirishlar/chiqishlar, ZK proof yordamchilari, xususiyatlarni aniqlash) - u dunyo holati mutatsiyalarini amalga oshirmaydi.
- `WsvHost` demosi `mock_wsv.rs` da mavjud boʻlib, u `AccountId`/Kotodama in map orqali aktiv operatsiyalari (Transfer/Mint/Burn) kichik xotiradagi WSV ni xaritada koʻrsatadi. x10..x13.

### ISI va ma'lumotlar modeli
- O'rnatilgan ISI turlari va semantikasi `iroha_core::smartcontracts::isi::*` da amalga oshirilgan va `docs/source/data_model_and_isi_spec.md` da hujjatlashtirilgan.
- `InstructionBox` barqaror “sim identifikatorlari” va Norito kodli registrdan foydalanadi; mahalliy ijro jo'natmasi yadrodagi joriy kod yo'lidir.### IVM asosiy integratsiyasi
- `State::execute_trigger(..)` keshlangan `IVM` ni klonlaydi, `CoreHost::with_accounts_and_args` ni biriktiradi va keyin `load_program` + `run` ni chaqiradi.
- `CoreHost` `IVMHost` ni qo'llaydi: statistik tizimlar ko'rsatgich-ABI TLV tartibi orqali dekodlanadi, o'rnatilgan ISI (`InstructionBox`) bilan taqqoslanadi va navbatga qo'yiladi. VM qaytgandan so'ng, xost o'sha ISI ni oddiy ijrochiga topshiradi, shuning uchun ruxsatlar, invariantlar, hodisalar va telemetriya mahalliy ijro bilan bir xil bo'lib qoladi. WSV ga tegmaydigan yordamchi tizim qo'ng'iroqlari hali ham `DefaultHost` ga vakolat beradi.
- `executor.rs` o'rnatilgan ISIni asl holatda ishlatishda davom etmoqda; validator ijrochining o'zini IVM ga ko'chirish kelajakdagi ish bo'lib qoladi.

### Kotodama → IVM
- Frontend qismlari mavjud (lekser/parser/minimal semantika/IR/regalloc).
- Codegen (`kotodama::compiler`) IVM operatsiyalari toʻplamini chiqaradi va aktiv operatsiyalari uchun `SCALL` dan foydalanadi:
  - `MintAsset` → o'rnatish x10=hisob, x11=aktiv, x12=&NoritoBytes(Raqamli); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` shunga o'xshash (summa NoritoBytes(Raqamli) ko'rsatgich sifatida berilgan).
- `koto_*_demo.rs` demolari `WsvHost` yordamida tez sinovdan oʻtkazish uchun identifikatorlarga koʻrsatilgan butun son indekslari bilan namoyish etiladi.

---

## Bo'shliqlar va nomuvofiqliklar

1) Asosiy xost qamrovi va pariteti
- Holat: `CoreHost` endi yadroda mavjud va standart yo'l orqali bajariladigan ko'plab daftar tizimi qo'ng'iroqlarini ISIga tarjima qiladi. Qoplash hali ham to'liq emas (masalan, ba'zi rollar/ruxsat/tetik tizimli qo'ng'iroqlar stublar) va navbatga qo'yilgan ISI ning mahalliy ijro bilan bir xil holat/hodisalar hosil qilishini kafolatlash uchun paritet testlari talab qilinadi.

2) Syscall yuzasi va ISI/Data Model nomlanishi va qamrovi
- NFT'lar: tizim qo'ng'iroqlari endi `iroha_data_model::nft` bilan moslangan kanonik `SYSCALL_NFT_*` nomlarini ko'rsatadi.
- Rollar/ruxsatlar/Triggerlar: tizim qo'ng'iroqlari ro'yxati mavjud, ammo har bir qo'ng'iroqni yadrodagi aniq ISIga bog'laydigan mos yozuvlar ilovasi yoki xaritalash jadvali mavjud emas.
- Parametrlar/semantika: ba'zi tizimli qo'ng'iroqlar parametr kodlashni (yozilgan identifikatorlar va ko'rsatkichlar) yoki gaz semantikasini aniqlamaydi; ISI semantikasi yaxshi aniqlangan.

3) VM/xost chegarasi bo'ylab yozilgan ma'lumotlarni uzatish uchun ABI
- Pointer-ABI TLV’lari endi `CoreHost` (`decode_tlv_typed`) da dekodlanadi, bu identifikatorlar, metama’lumotlar va JSON foydali yuklari uchun deterministik yo‘l beradi. Har bir tizim qoʻngʻirogʻi kutilgan koʻrsatkich turlarini hujjatlashini va Kotodama toʻgʻri TLVlarni chiqarishini (shu jumladan siyosat turini rad etganda xatoliklarni bartaraf etish) taʼminlash ustida ish olib borilmoqda.

4) Gaz va xato xaritalashning mustahkamligi
- IVM operatsion kodlari har bir gaz uchun to'lov; CoreHost endi mahalliy gaz jadvali (jumladan, paketli uzatishlar va sotuvchi ISI ko'prigi) yordamida ISI tizimlari uchun qo'shimcha gazni qaytaradi va ZK tekshiruv tizimlari maxfiy gaz jadvalini qayta ishlatadi. DefaultHost hali ham sinovni qoplash uchun minimal xarajatlarni saqlab qoladi.
- Xato yuzalari farqlanadi: IVM qaytaradi `VMError::{OutOfGas,PermissionDenied,...}`; ISI `InstructionExecutionError` toifalarini qaytaradi (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, Kotodama, Kotodama).5) Tezlanish yo'llari bo'ylab determinizm
- IVM vektor/CUDA/Metal protsessorning zaxiralariga ega, ammo ba'zi operatsiyalar saqlanib qolmoqda (`SETVL`, PARBEGIN/PAREND) va hali deterministik yadroning bir qismi emas.
- Merkle daraxtlari IVM va tugun (`ivm::merkle_tree` va `iroha_crypto::MerkleTree`) o'rtasida farqlanadi - birlashtirish elementi allaqachon `roadmap.md` da paydo bo'lgan.

6) Kotodama til yuzasi va mo'ljallangan daftar semantikasi
- kompilyator kichik kichik to'plamni chiqaradi; til xususiyatlarining aksariyati (holat/strukturalar, triggerlar, ruxsatlar, kiritilgan parametrlar/qaytarmalar) hali xost/ISI modeliga ulanmagan.
- Tizimli qo'ng'iroqlar hokimiyat uchun qonuniy ekanligiga ishonch hosil qilish uchun yozish qobiliyati/effekti yo'q.

---

## Tavsiyalar (aniq qadamlar)

### A. Yadroda ishlab chiqarish IVM xostini amalga oshirish
- `ivm::host::IVMHost`ni amalga oshiruvchi `iroha_core::smartcontracts::ivm::host` modulini qo'shing.
- `ivm::syscalls` da har bir tizim qo'ng'irog'i uchun:
  - Kanonik ABI orqali argumentlarni dekodlash (B.ga qarang), mos keladigan o'rnatilgan ISI ni yarating yoki to'g'ridan-to'g'ri bir xil asosiy mantiqqa qo'ng'iroq qiling, uni `StateTransaction` ga qarshi bajaring va xatolarni aniq tarzda IVM qaytish kodiga qaytaring.
  - Yadroda belgilangan har bir tizimli qo'ng'iroq jadvali yordamida gazni aniq zaryadlang (va kelajakda kerak bo'lsa, `SYSCALL_GET_PARAMETER` orqali IVM ta'sir qiladi). Dastlab, har bir qo'ng'iroq uchun uy egasidan belgilangan qo'shimcha gazni qaytaring.
- `authority: &AccountId` va `&mut StateTransaction` ni xostga o'tkazing, shuning uchun ruxsatlarni tekshirish va hodisalar mahalliy ISI bilan bir xil bo'ladi.
- Ushbu xostni `vm.run()` dan oldin biriktirish uchun `State::execute_trigger(ExecutableRef::Ivm)` ni yangilang va ISI bilan bir xil `ExecutionStep` semantikasini qaytaring (hodisalar allaqachon yadroda chiqarilgan; izchil xatti-harakatlar tasdiqlanishi kerak).

### B. Terilgan qiymatlar uchun deterministik VM/host ABI ni aniqlang
- Strukturaviy argumentlar uchun VM tomonida Norito dan foydalaning:
  - `AccountId`, `AssetDefinitionId`, `Numeric`, I101270.
  - Xost `IVM` xotira yordamchilari orqali baytlarni o'qiydi va Norito bilan dekodlaydi (`iroha_data_model` allaqachon `Encode/Decode` hosil qiladi).
- Kotodama kodidagi minimal yordamchilarni qo'shing, literal identifikatorlarni kod/doimiy hovuzlarga seriyalashtirish yoki xotirada chaqiruv ramkalarini tayyorlash.
- Miqdorlar `Numeric` va NoritoBytes ko'rsatkichlari sifatida uzatiladi; boshqa murakkab turlar ham ko‘rsatgich orqali o‘tadi.
- Buni `crates/ivm/docs/calling_convention.md` da hujjatlang va misollar qo'shing.### C. Tizim chaqiruvi nomini va qamrovini ISI/Ma'lumotlar modeli bilan moslashtiring
- Aniqlik uchun NFT bilan bog'liq tizimli qo'ng'iroqlar nomini o'zgartiring: kanonik nomlar endi `SYSCALL_NFT_*` naqshiga amal qiladi (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA` va boshqalar).
- Har bir tizim chaqiruvidan asosiy ISI semantikasiga xaritalash jadvalini (hujjat + kod sharhlari) nashr eting, jumladan:
  - Parametrlar (registrlar va ko'rsatkichlar), kutilgan old shartlar, hodisalar va xato xaritalari.
  - Gaz to'lovlari.
- Har bir o'rnatilgan ISI uchun Kotodama (domenlar, hisoblar, aktivlar, rollar/ruxsatlar, triggerlar, parametrlar) dan chaqirilmasligi kerak bo'lgan tizim chaqiruvi mavjudligiga ishonch hosil qiling. Agar ISI imtiyozli bo'lib qolishi kerak bo'lsa, uni hujjatlashtiring va xostdagi ruxsatlarni tekshirish orqali amalga oshiring.

### D. Xatolar va gazlarni birlashtirish
- Xostda tarjima qatlamini qo'shing: `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` xaritasini maxsus `VMError` kodlari yoki kengaytirilgan natija konventsiyasi (masalan, `x10=0/1` ni o'rnating va aniq belgilangan `VMError::HostRejected { code }` dan foydalaning).
- Syscalls uchun yadroda gaz jadvalini joriy qilish; uni IVM hujjatlarida aks ettirish; xarajatlarni kiritish hajmi prognoz qilinadigan va platformadan mustaqil bo'lishini ta'minlash.

### E. Determinizm va umumiy primitivlar
- Merkle daraxtini birlashtirishni yakunlang (yo'l xaritasiga qarang) va bir xil barglar va dalillar bilan `ivm::merkle_tree` - `iroha_crypto` ni olib tashlang/taxallus.
- `SETVL`/PARBEGIN/PAREND` ni oxirigacha determinizm tekshiruvlari va deterministik rejalashtirish strategiyasi mavjud bo'lgunga qadar zahirada saqlang; IVM bugungi kunda ushbu maslahatlarni e'tiborsiz qoldiradigan hujjat.
- Tezlashtirish yo'llari bayt-bayt bir xil natijalar berishiga ishonch hosil qiling; Agar imkoni bo'lmasa, protsessorning qayta tiklanish ekvivalentligini ta'minlash uchun sinov orqali xususiyatlarni saqlang.

### F. Kotodama kompilyator simlari
- ID va murakkab parametrlar uchun kodekni kanonik ABI (B.) ga kengaytiring; integer→ID demo xaritalaridan foydalanishni to‘xtating.
- Aniq nomlar bilan aktivlardan tashqari (domenlar/hisoblar/rollar/ruxsatlar/triggerlar) to'g'ridan-to'g'ri ISI tizimi qo'ng'iroqlariga o'rnatilgan xaritalarni qo'shing.
- Kompilyatsiya vaqtini tekshirish va ixtiyoriy `permission(...)` izohlarini qo'shing; statik isbot mumkin bo'lmaganda, ish vaqti xostidagi xatolarga qaytish.
- Norito argumentlarini dekodlaydigan va vaqtinchalik WSVni mutatsiyaga soluvchi test xostidan foydalanib, kichik shartnomalarni toʻplaydigan va boshqaradigan `crates/ivm/tests/kotodama.rs` da birlik testlarini qoʻshing.

### G. Hujjatlar va ishlab chiquvchilar ergonomikasi
- `docs/source/data_model_and_isi_spec.md` ni tizimli xaritalash jadvali va ABI qaydlari bilan yangilang.
- `IVMHost` ni haqiqiy `StateTransaction` orqali qanday amalga oshirishni tavsiflovchi `crates/ivm/docs/` da yangi “IVM Xost integratsiyasi qoʻllanmasi” hujjatini qoʻshing.
- `README.md` va sandiq hujjatlarida Kotodama IVM `.to` bayt kodini nishonga olishini va tizimli chaqiruvlar dunyo holatiga ko'prik ekanligini aniqlang.

---

## Tavsiya etilgan xaritalash jadvali (dastlabki qoralama)

Vakil kichik to'plami - xostni amalga oshirish jarayonida yakunlang va kengaytiring.- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → ISI Register
- SYSCALL_REGISTER_ACCOUNT (id: ptr AccountId) → ISI Register
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → ISI Register
- SYSCALL_MINT_ASSET(hisob: ptr AccountId, aktiv: ptr AssetDefinitionId, summa: ptr NoritoBytes(Raqamli)) → ISI Mint
- SYSCALL_BURN_ASSET(hisob: ptr AccountId, aktiv: ptr AssetDefinitionId, summa: ptr NoritoBytes(Numeric)) → ISI Burn
- SYSCALL_TRANSFER_ASSET(dan: ptr AccountId, to: ptr AccountId, aktiv: ptr AssetDefinitionId, summa: ptr NoritoBytes(Raqamli)) → ISI Transfer
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (ko‘lamni ochish/yopish; individual yozuvlar `transfer_asset` orqali tushiriladi)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → Shartnomalar zanjirdan tashqaridagi yozuvlarni seriyalashtirganda, oldindan kodlangan partiyani yuboring
- SYSCALL_NFT_MINT_ASSET (id: ptr NftId, egasi: ptr AccountId) → ISI Register
- SYSCALL_NFT_TRANSFER_ASSET(: ptr AccountId, dan: ptr AccountId, identifikator: ptr NftId) → ISI Transfer
- SYSCALL_NFT_SET_METADATA (id: ptr NftId, kontent: ptr metadata) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET (id: ptr NftId) → ISI registrini bekor qilish
- SYSCALL_CREATE_ROLE(id: ptr RoleId, rol: ptr Role) → ISI Register
- SYSCALL_GRANT_ROLE(hisob: ptr AccountId, rol: ptr RoleId) → ISI Grant
- SYSCALL_REVOKE_ROLE(hisob: ptr AccountId, rol: ptr RoleId) → ISI bekor qilish
- SYSCALL_SET_PARAMETER(param: ptr Parameter) → ISI SetParameter

Eslatmalar
- “ptr T” VM xotirasida saqlangan T uchun Norito-kodlangan baytlarga registrdagi ko'rsatgichni anglatadi; xost uni mos keladigan `iroha_data_model` turiga dekodlaydi.
- Qaytish konventsiyasi: muvaffaqiyat to'plamlari `x10=1`; nosozlik `x10=0` to'plamlari va halokatli xatolar uchun `VMError::HostRejected` ni oshirishi mumkin.

---

## Xatarlar va tarqatish rejasi
- Tor majmui (Aktivlar + Hisoblar) uchun xostni ulashdan boshlang va yo'naltirilgan testlarni qo'shing.
- Xost semantikasi etuk bo'lganda, mahalliy ISI ijrosini ishonchli yo'l sifatida saqlang; bir xil yakuniy effektlar va hodisalarni tasdiqlash uchun testlarda ikkala yo'lni ham "soya rejimi" ostida boshqaring.
- Paritet tasdiqlangach, ishlab chiqarishda IVM triggerlari uchun IVM xostini yoqing; keyinchalik IVM orqali muntazam tranzaktsiyalarni yo'naltirishni ham ko'rib chiqing.

---

## Ajoyib ish
- Norito-kodlangan ko'rsatkichlarni (`crates/ivm/src/kotodama_std.rs`) o'tadigan Kotodama yordamchilarini yakunlang va ularni CLI kompilyatori orqali yuzaga chiqaring.
- Syscall gaz jadvalini (shu jumladan yordamchi tizim qo'ng'iroqlarini) nashr eting va CoreHost-ni qo'llash/testlarini unga mos holda saqlang.
- ✅ ABI ko'rsatkich-argumentini qamrab oluvchi Norito bo'ylab sayohat moslamalari qo'shildi; CIda saqlanadigan manifest va NFT ko'rsatgich qamrovi uchun `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` ga qarang.