---
lang: uz
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 ma'lumotlar modeli va ISI — Amalga oshirishdan olingan spetsifikatsiya

Ushbu spetsifikatsiya dizaynni ko'rib chiqishga yordam berish uchun `iroha_data_model` va `iroha_core` bo'ylab joriy joriy qilishdan teskari ishlab chiqilgan. Backticklardagi yo'llar vakolatli kodga ishora qiladi.

## Qo'llash doirasi
- Kanonik ob'ektlarni (domenlar, hisoblar, aktivlar, NFTlar, rollar, ruxsatlar, tengdoshlar, triggerlar) va ularning identifikatorlarini belgilaydi.
- Holatni o'zgartiruvchi ko'rsatmalarni (ISI) tavsiflaydi: turlar, parametrlar, old shartlar, holat o'tishlari, chiqarilgan hodisalar va xatolik shartlari.
- Parametrlarni boshqarish, tranzaktsiyalar va ko'rsatmalarni ketma-ketlashtirishni umumlashtiradi.

Determinizm: Barcha ko'rsatmalar semantikasi apparatga bog'liq bo'lmagan sof holatga o'tishdir. Serializatsiya Norito dan foydalanadi; VM baytkodi IVM dan foydalanadi va zanjirda bajarilishidan oldin xost tomonidan tekshiriladi.

---

## Ob'ektlar va identifikatorlar
Identifikatorlar `Display`/`FromStr` boʻylab sayohatga ega barqaror string shakllariga ega. Nom qoidalari bo'sh joy va ajratilgan `@ # $` belgilarni taqiqlaydi.- `Name` - tasdiqlangan matn identifikatori. Qoidalar: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domen: `{ id, logo, metadata, owned_by }`. Quruvchilar: `NewDomain`. Kod: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` - kanonik manzillar `AccountAddress` (I105 / hex) orqali ishlab chiqariladi va Torii `AccountAddress::parse_encoded` orqali kirishlarni normallashtiradi. I105 - afzal qilingan hisob formati; I105 shakli faqat Sora UX uchun. Tanish `alias` (rad etilgan eski shakl) qatori faqat marshrutlash taxallus sifatida saqlanadi. Hisob: `{ id, metadata }`. Kod: `crates/iroha_data_model/src/account.rs`.- Hisobni qabul qilish siyosati — domenlar `iroha:account_admission_policy` metamaʼlumotlar kaliti ostida Norito-JSON `AccountAdmissionPolicy` ni saqlash orqali yashirin hisob yaratishni nazorat qiladi. Kalit yo'q bo'lganda, zanjir darajasidagi maxsus parametr `iroha:default_account_admission_policy` standartni ta'minlaydi; u ham bo'lmasa, qattiq standart `ImplicitReceive` (birinchi versiya). Siyosat teglari `mode` (`ExplicitOnly` yoki `ImplicitReceive`) hamda ixtiyoriy har bir tranzaksiya (standart `16`) va har bir blok yaratish cheklovlari, ixtiyoriy `implicit_creation_fee` hisobi (yoki sink), Har bir aktiv taʼrifi uchun `min_initial_amounts` va ixtiyoriy `default_role_on_create` (`AccountCreated` dan keyin beriladi, agar yoʻq boʻlsa, `DefaultRoleError` bilan rad etiladi). Ibtido ishtirok eta olmaydi; o'chirilgan/yaroqsiz siyosatlar `InstructionExecutionError::AccountAdmission` bilan noma'lum hisoblar uchun kvitansiya uslubidagi ko'rsatmalarni rad etadi. `AccountCreated` dan oldin `iroha:created_via="implicit"` metamaʼlumotlarining yashirin hisob shtampi; standart rollar keyingi `AccountRoleGranted` ni chiqaradi va ijrochi egasining asosiy qoidalari yangi hisobning o'z aktivlarini/NFTlarini qo'shimcha rollarsiz sarflashga imkon beradi. Kod: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — kanonik `aid:<32-lower-hex-no-dash>` (UUID-v4 bayt). Ta'rif: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. `alias` harflari `<name>#<domain>@<dataspace>` yoki `<name>#<dataspace>`, `<name>` obyekt taʼrifi nomiga teng boʻlishi kerak. Kod: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: kanonik kodlangan literal `norito:<hex>` (eski matn shakllari birinchi versiyada qo'llab-quvvatlanmaydi).- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Kod: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Rol: `{ id, permissions: BTreeSet<Permission> }` quruvchi `NewRole { inner: Role, grant_to }` bilan. Kod: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Kod: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — tengdosh identifikatori (ochiq kalit) va manzil. Kod: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Trigger: `{ id, action }`. Harakat: `{ executable, repeats, authority, filter, metadata }`. Kod: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` qoʻshish/olib tashlash belgisi bilan. Kod: `crates/iroha_data_model/src/metadata.rs`.
- Obuna namunasi (ilova qatlami): rejalar `subscription_plan` metama'lumotlariga ega `AssetDefinition` yozuvlari; obunalar `subscription` metama'lumotlariga ega `Nft` yozuvlari; hisob-kitob obuna NFTlariga havola qiluvchi vaqt triggerlari orqali amalga oshiriladi. `docs/source/subscriptions_api.md` va `crates/iroha_data_model/src/subscription.rs` ga qarang.
- **Kriptografik primitivlar** (`sm` xususiyati):
  - `Sm2PublicKey` / `Sm2Signature` kanonik SEC1 nuqtasini + SM2 uchun qattiq kenglikdagi `r∥s` kodlashni aks ettiradi. Konstruktorlar egri chiziqli a'zolikni va farqlovchi ID semantikasini (`DEFAULT_DISTID`) ta'minlaydi, tekshirish esa noto'g'ri shakllangan yoki yuqori diapazonli skalerlarni rad etadi. Kod: `crates/iroha_crypto/src/sm.rs` va `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` GM/T 0004 dayjestini Norito seriyali `[u8; 32]` yangi turi sifatida manifestlarda yoki telemetriyada xeshlar paydo bo'lganda ishlatiladi. Kod: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` 128-bitli SM4 kalitlarini ifodalaydi va xost tizimlari va maʼlumotlar modeli qurilmalari oʻrtasida taqsimlanadi. Kod: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Bu turlar mavjud Ed25519/BLS/ML-DSA primitivlari bilan birga joylashadi va `sm` funksiyasi yoqilgandan keyin maʼlumotlar modeli isteʼmolchilari (Torii, SDK, genezis asboblari) foydalanishi mumkin.
- Ma'lumotlar maydonidan olingan aloqalar do'konlari (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, tarmoqli releyli favqulodda vaziyatlarni bekor qilish registrlari) va ma'lumotlar maydoni uchun maqsadli ruxsatlar (permissions of dataspace-target permissions permissions of dataspace in 8110101010101010) do'konlar) ma'lumotlar bo'shliqlari faol `dataspace_catalog` dan yo'qolganda, `State::set_nexus(...)` da kesiladi, bu esa ish vaqti katalogi yangilanishidan keyin eskirgan ma'lumotlar maydoni havolalarini oldini oladi. Yoʻlak koʻlamli DA/rele keshlari (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) shuningdek, chiziq oʻchirilganda yoki boshqa maʼlumotlar maydoniga qayta tayinlanganda kesiladi. Space Directory ISI'lar (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) shuningdek, `dataspace` ni faol katalogga nisbatan tasdiqlaydi va `InvalidParameter` bilan noma'lum identifikatorlarni rad etadi.

Muhim xususiyatlar: `Identifiable`, `Registered`/`Registrable` (quruvchi namunasi), `HasMetadata`, `IntoKeyValue`. Kod: `crates/iroha_data_model/src/lib.rs`.

Voqealar: Har bir ob'ektda mutatsiyalar (yaratish/o'chirish/egasini o'zgartirish/meta-ma'lumotlarini o'zgartirish va h.k.) sodir bo'lgan hodisalar mavjud. Kod: `crates/iroha_data_model/src/events/`.

---## Parametrlar (zanjir konfiguratsiyasi)
- Oilalar: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, plus `custom: BTreeMap`.
- Farqlar uchun yagona raqamlar: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Agregator: `Parameters`. Kod: `crates/iroha_data_model/src/parameter/system.rs`.

Parametrlarni sozlash (ISI): `SetParameter(Parameter)` mos keladigan maydonni yangilaydi va `ConfigurationEvent::Changed` chiqaradi. Kod: `crates/iroha_data_model/src/isi/transparent.rs`, `crates/iroha_core/src/smartcontracts/isi/world.rs` da ijrochi.

---

## Yo'riqnomani seriyalashtirish va ro'yxatga olish
- Asosiy xususiyat: `Instruction: Send + Sync + 'static`, `dyn_encode()`, `as_any()`, barqaror `id()` (standart beton turi nomi uchun).
- `InstructionBox`: `Box<dyn Instruction>` o'rami. Clone/Eq/Ord `(type_id, encoded_bytes)` da ishlaydi, shuning uchun tenglik qiymat bo'yicha bo'ladi.
- `InstructionBox` uchun Norito serde `(String wire_id, Vec<u8> payload)` sifatida seriyalanadi (agar sim identifikatori bo'lmasa, `type_name` ga qaytadi). Deserializatsiya konstruktorlarga global `InstructionRegistry` xaritalash identifikatorlaridan foydalanadi. Standart registr barcha o'rnatilgan ISIni o'z ichiga oladi. Kod: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: turlari, semantikasi, xatolari
Bajarish `iroha_core::smartcontracts::isi` da `Execute for <Instruction>` orqali amalga oshiriladi. Quyida ommaviy effektlar, old shartlar, chiqarilgan hodisalar va xatolar ro'yxati keltirilgan.

### Ro'yxatdan o'tish / Ro'yxatdan o'tish
Turlari: `Register<T: Registered>` va `Unregister<T: Identifiable>`, `RegisterBox`/`UnregisterBox` yig'indisi aniq maqsadlarni qamrab oladi.- Register Peer: dunyodagi tengdoshlar to'plamiga qo'shimchalar.
  - Old shartlar: mavjud bo'lmasligi kerak.
  - Voqealar: `PeerEvent::Added`.
  - Xatolar: agar dublikat bo'lsa, `Repetition(Register, PeerId)`; Qidiruvlarda `FindError`. Kod: `core/.../isi/world.rs`.

- Domenni ro'yxatdan o'tkazish: `NewDomain` dan `owned_by = authority` bilan tuziladi. Ruxsat berilmagan: `genesis` domeni.
  - Old shartlar: domenning mavjud emasligi; emas `genesis`.
  - Voqealar: `DomainEvent::Created`.
  - Xatolar: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Kod: `core/.../isi/world.rs`.

- Hisob qaydnomasini ro'yxatdan o'tkazish: `genesis` domenida taqiqlangan `NewAccount` dan tuziladi; `genesis` hisobini ro‘yxatdan o‘tkazib bo‘lmaydi.
  - Old shartlar: domen mavjud bo'lishi kerak; hisobning yo'qligi; genezis sohasida emas.
  - Voqealar: `DomainEvent::Account(AccountEvent::Created)`.
  - Xatolar: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Kod: `core/.../isi/domain.rs`.

- Register AssetDefinition: quruvchidan tuziladi; `owned_by = authority` to'plamlari.
  - Old shartlar: mavjud emaslik ta'rifi; domen mavjud; `name` talab qilinadi, kesishdan keyin boʻsh boʻlmasligi va `#`/`@` boʻlmasligi kerak.
  - Voqealar: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Xatolar: `Repetition(Register, AssetDefinitionId)`. Kod: `core/.../isi/domain.rs`.

- NFTni ro'yxatdan o'tkazish: quruvchidan tuzilmalar; `owned_by = authority` to'plamlari.
  - Old shartlar: NFT mavjud emasligi; domen mavjud.
  - Voqealar: `DomainEvent::Nft(NftEvent::Created)`.
  - Xatolar: `Repetition(Register, NftId)`. Kod: `core/.../isi/nft.rs`.- Ro'yxatdan o'tish: `NewRole { inner, grant_to }` dan tuziladi (birinchi egasi hisob rolini xaritalash orqali qayd etilgan), `inner: Role` saqlaydi.
  - Old shartlar: rolning yo'qligi.
  - Voqealar: `RoleEvent::Created`.
  - Xatolar: `Repetition(Register, RoleId)`. Kod: `core/.../isi/world.rs`.

- Register Trigger: triggerni filtr turi bo'yicha o'rnatilgan tegishli triggerda saqlaydi.
  - Old shartlar: Agar filtr zarb qilinmasa, `action.repeats` `Exactly(1)` bo'lishi kerak (aks holda `MathError::Overflow`). Ikki nusxadagi identifikatorlar taqiqlangan.
  - Voqealar: `TriggerEvent::Created(TriggerId)`.
  - Xatolar: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` konvertatsiya/tasdiqlashda xatoliklar. Kod: `core/.../isi/triggers/mod.rs`.- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger registrini bekor qilish: maqsadni olib tashlaydi; oʻchirish hodisalarini chiqaradi. Qo'shimcha kaskadli olib tashlashlar:- Domenni ro'yxatdan o'chirish: domen ob'ektini va uning selektor/tasdiqlash siyosati holatini olib tashlaydi; domendagi aktiv taʼriflarini (va shu taʼriflar bilan kalitlangan `zk_assets` maxfiy holatini), ushbu taʼriflar aktivlarini (va har bir aktiv metamaʼlumotlarini), domendagi NFTʼlarni va domenga oid hisob yorligʻi/taxallus prognozlarini oʻchirib tashlaydi. Shuningdek, u oʻchirilgan domendan omon qolgan hisoblarni uzib qoʻyadi va olib tashlangan domenga yoki u bilan birga oʻchirilgan resurslarga (domen ruxsatnomalari, olib tashlangan taʼriflar uchun aktiv taʼrifi/aktiv ruxsatnomalari va olib tashlangan NFT identifikatorlari uchun NFT ruxsatnomalari) havola qiluvchi hisob/rol boʻyicha ruxsat yozuvlarini oʻchiradi. Domenni olib tashlash global `AccountId`, uning tx ketma-ketligi/UAID holati, xorijiy aktiv yoki NFT egaligi, ishga tushirish vakolati yoki saqlanib qolgan hisobga ishora qiluvchi boshqa tashqi audit/konfiguratsiya maʼlumotnomalarini oʻchirib tashlamaydi. Qo'riqlash panjaralari: domendagi har qanday aktiv ta'rifi hali ham repo-kelishuvi, hisob-kitob kitobi, jamoat yo'li mukofoti/da'vosi, oflayn to'lov/o'tkazish, hisob-kitob repo defoltlari (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), boshqaruv-konfiguratsiyalangan/viralizatsiya-vositaviylik/vositaviylik-repo-shartnomasi bilan havola qilinganda rad etadi. aktiv-ta'rifi ma'lumotnomalari, oracle-economics sozlangan mukofot/slash/nizo-obligatsiya aktiv-ta'rifi ma'lumotnomalari yoki Nexus to'lovi/staking aktivi ta'rifi ma'lumotnomalari (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Voqealar: `DomainEvent::Deleted`, shuningdek, har bir elementni o'chirisholib tashlangan domenga tegishli resurslar uchun voqealar haqida. Xatolar: agar etishmayotgan bo'lsa, `FindError::Domain`; `InvariantViolation` saqlangan obyekt taʼrifi mos yozuvlar ziddiyatlari. Kod: `core/.../isi/world.rs`.- Hisob qaydnomasini ro‘yxatdan o‘chirish: hisob ruxsatnomalari, rollari, tx-ketma-ket hisoblagichi, hisob yorlig‘i xaritasi va UAID bog‘lanishlarini olib tashlaydi; hisobga tegishli aktivlarni (va har bir aktiv metamaʼlumotlarini) oʻchirib tashlaydi; hisobga tegishli NFTlarni o'chiradi; vakolati shu hisob bo'lgan triggerlarni olib tashlaydi; oʻchirilgan hisob qaydnomasiga havola qiluvchi hisob/rol boʻyicha ruxsat yozuvlarini, olib tashlangan tegishli NFT identifikatorlari uchun hisob/rol boʻyicha NFT-maqsad ruxsatlarini va olib tashlangan triggerlar uchun hisob/rol doirasidagi trigger-maqsadli ruxsatlarni oʻchirib tashlaydi. Qo'riqlash bloklari: agar hisob hali ham domenga ega bo'lsa, ob'ekt ta'rifi, SoraFS provayder bilan bog'lanishi, faol fuqarolik rekordi, umumiy staking/mukofot holati (jumladan, hisob da'vogar yoki mukofot-aktiv egasi sifatida ko'rinadigan mukofot-da'vo kalitlari), faol oracle holati (jumladan, twitter-hisob-kitoblar, oracle-provayderlar-provayderlari)ni rad etadi yoki oracle-economics sozlangan mukofot/slash hisobi havolalari), faol Nexus toʻlovi/staking hisob qaydnomalari (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; kanonik sifatida tahlil qilingan), faol domensiz hisob qaydnomasi va faol domensiz hisob qaydnomasi muvaffaqiyatsiz tugadi. repo-kelishuv holati, faol hisob-kitoblar kitobi holati, faol oflayn imtiyoz/o‘tkazish yoki oflayn hukm-bekor qilish holati, faol aktiv ta’riflari uchun faol oflayn eskrow-hisob qaydnomasi konfiguratsiyasi havolalari (`settlement.offline.escrow_accounts`), faol boshqaruv holati (taklif/bosqichni tasdiqlashals/locks/slashes/kengash/parlament roʻyxatlari, taklif parlamenti suratlari, ish vaqtini yangilash taklifchisi yozuvlari, boshqaruv tomonidan sozlangan escrow/slash-receiver/virusli-pul hisobi maʼlumotnomalari, boshqaruv SoraFS telemetriya maʼlumotnomalari / I10XNI/I1010NI orqali `gov.sorafs_telemetry.per_provider_submitters` yoki `gov.sorafs_provider_owners` orqali boshqaruv tomonidan sozlangan SoraFS provayder-egasi maʼlumotnomalari, sozlangan kontentni nashr qilish ruxsatnomalari roʻyxati hisobi havolalari (`content.publish_allow_accounts`), faol ijtimoiy escrow joʻnatuvchisi holati, faol kontentni yuboruvchi holati, faol kontentni yuboruvchi holati faol chiziqli reley favqulodda tekshiruvi bekor qilish holati yoki faol SoraFS pin-registr emitenti/bog'lovchi yozuvlari (pin manifestlari, manifest taxalluslari, replikatsiya buyurtmalari). Voqealar: `AccountEvent::Deleted`, shuningdek, olib tashlangan NFT uchun `NftEvent::Deleted`. Xatolar: agar yo'q bo'lsa, `FindError::Account`; `InvariantViolation` egalik etimlari. Kod: `core/.../isi/domain.rs`.- AssetDefinition registrini bekor qilish: ushbu taʼrifning barcha aktivlarini va ularning har bir aktiv metamaʼlumotlarini oʻchirib tashlaydi va shu taʼrif bilan kalitlangan maxfiy `zk_assets` yon holatini oʻchiradi; shuningdek, olib tashlangan aktiv taʼrifiga yoki uning aktiv namunalariga havola qiluvchi mos keladigan `settlement.offline.escrow_accounts` yozuvi va hisob/rol boʻyicha ruxsat yozuvlarini kesadi. Qo'riqchi relslar: ta'rif hali ham repo-kelishuvi, hisob-kitob kitobi, ommaviy yo'lak mukofoti/da'vosi, oflayn to'lov/o'tkazma holati, hisob-kitob repo defoltlari (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), boshqaruv tomonidan sozlangan ovoz berish/fuqarolik huquqiga ega bo'lganida rad etadi aktiv-ta'rifi ma'lumotnomalari, oracle-economics sozlangan mukofot/slash/nizo-obligatsiya aktiv-ta'rifi ma'lumotnomalari yoki Nexus to'lovi/staking aktivi ta'rifi ma'lumotnomalari (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Voqealar: har bir aktiv uchun `AssetDefinitionEvent::Deleted` va `AssetEvent::Deleted`. Xatolar: `FindError::AssetDefinition`, `InvariantViolation` mos yozuvlar ziddiyatlari. Kod: `core/.../isi/domain.rs`.
  - NFTni ro'yxatdan o'chirish: NFTni o'chiradi va o'chirilgan NFTga havola qiluvchi hisob/rol bo'yicha ruxsat yozuvlarini o'chiradi. Voqealar: `NftEvent::Deleted`. Xatolar: `FindError::Nft`. Kod: `core/.../isi/nft.rs`.
  - Rolni ro'yxatdan o'chirish: birinchi navbatda barcha hisoblardan rolni bekor qiladi; keyin rolni olib tashlaydi. Voqealar: `RoleEvent::Deleted`. Xatolar: `FindError::Role`. Kod: `core/.../isi/world.rs`.- Triggerni ro'yxatdan o'chirish: agar mavjud bo'lsa, triggerni o'chiradi va olib tashlangan triggerga havola qiluvchi hisob/rol bo'yicha ruxsat yozuvlarini o'chiradi; dublikat ro'yxatdan o'chirish `Repetition(Unregister, TriggerId)` hosil qiladi. Voqealar: `TriggerEvent::Deleted`. Kod: `core/.../isi/triggers/mod.rs`.

### Yalpiz / Kuyish
Turlari: `Mint<O, D: Identifiable>` va `Burn<O, D: Identifiable>`, `MintBox`/`BurnBox` sifatida qutiga solingan.

- Asset (Raqamli) mint/burn: balanslar va taʼrifning `total_quantity` ni sozlaydi.
  - Old shartlar: `Numeric` qiymati `AssetDefinition.spec()` ni qondirishi kerak; `mintable` tomonidan ruxsat etilgan zarb:
    - `Infinitely`: har doim ruxsat beriladi.
    - `Once`: aynan bir marta ruxsat berilgan; birinchi yalpiz `mintable` ni `Not` ga aylantiradi va `AssetDefinitionEvent::MintabilityChanged` chiqaradi, shuningdek audit uchun batafsil `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`.
    - `Limited(n)`: `n` qo'shimcha zarb operatsiyalariga ruxsat beradi. Har bir muvaffaqiyatli yalpiz hisoblagichni kamaytiradi; u nolga yetganda, ta'rif `Not` ga o'tadi va yuqoridagi kabi bir xil `MintabilityChanged` hodisalarini chiqaradi.
    - `Not`: xato `MintabilityError::MintUnmintable`.
  - Davlat o'zgarishlari: agar zarbda etishmayotgan bo'lsa, aktiv yaratadi; yonish paytida balans nolga teng bo'lsa, aktiv yozuvini olib tashlaydi.
  - Voqealar: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (`Once` yoki `Limited(n)` o'z mablag'larini tugatganda).
  - Xatolar: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Kod: `core/.../isi/asset.rs`.- Trigger takrorlash yalpiz/yonish: o'zgarishlar `action.repeats` trigger uchun hisob.
  - Old shartlar: yalpizda filtr zarb qilinadigan bo'lishi kerak; arifmetika to'lib-toshib ketmasligi kerak.
  - Voqealar: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Xatolar: yaroqsiz yalpizda `MathError::Overflow`; Agar yo'q bo'lsa, `FindError::Trigger`. Kod: `core/.../isi/triggers/mod.rs`.

### Transfer
Turlari: `Transfer<S: Identifiable, O, D: Identifiable>`, `TransferBox` sifatida qutiga solingan.

- Obyekt (raqamli): `AssetId` manbasidan ayiring, `AssetId` manziliga qo'shing (bir xil ta'rif, boshqa hisob). Nollangan manba obyektini oʻchirish.
  - Old shartlar: manba aktivi mavjud; qiymat `spec` ni qondiradi.
  - Voqealar: `AssetEvent::Removed` (manba), `AssetEvent::Added` (maqsad).
  - Xatolar: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Kod: `core/.../isi/asset.rs`.

- Domenga egalik: `Domain.owned_by` ni maqsadli hisobga o'zgartiradi.
  - Old shartlar: ikkala hisob ham mavjud; domen mavjud.
  - Voqealar: `DomainEvent::OwnerChanged`.
  - Xatolar: `FindError::Account/Domain`. Kod: `core/.../isi/domain.rs`.

- AssetDefinition egaligi: `AssetDefinition.owned_by` ni maqsadli hisobga o'zgartiradi.
  - Old shartlar: ikkala hisob ham mavjud; ta'rifi mavjud; manba hozirda unga egalik qilishi kerak; vakolat manba hisobi, manba-domen egasi yoki aktiv-ta’rifi-domen egasi bo‘lishi kerak.
  - Voqealar: `AssetDefinitionEvent::OwnerChanged`.
  - Xatolar: `FindError::Account/AssetDefinition`. Kod: `core/.../isi/account.rs`.- NFT egaligi: `Nft.owned_by` ni maqsadli hisobga o'zgartiradi.
  - Old shartlar: ikkala hisob ham mavjud; NFT mavjud; manba hozirda unga egalik qilishi kerak; vakolat manba hisobi, manba-domen egasi, NFT-domen egasi yoki ushbu NFT uchun `CanTransferNft`-ni ushlab turishi kerak.
  - Voqealar: `NftEvent::OwnerChanged`.
  - Xatolar: `FindError::Account/Nft`, `InvariantViolation`, agar manba NFTga ega bo'lmasa. Kod: `core/.../isi/nft.rs`.

### Metadata: Kalit-qiymatini o'rnatish/o'chirish
Turlari: `SetKeyValue<T>` va `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` bilan. Taqdim etilgan qutidagi raqamlar.

- O'rnatish: `Metadata[key] = Json(value)`ni qo'shadi yoki almashtiradi.
- Olib tashlash: kalitni olib tashlaydi; yo'q bo'lsa xato.
- Voqealar: `<Target>Event::MetadataInserted` / `MetadataRemoved` eski/yangi qiymatlar bilan.
- Xatolar: `FindError::<Target>`, agar maqsad mavjud bo'lmasa; Olib tashlash uchun etishmayotgan kalitda `FindError::MetadataKey`. Kod: `crates/iroha_data_model/src/isi/transparent.rs` va maqsad uchun ijrochi impls.

### Ruxsatlar va rollar: berish / bekor qilish
Turlari: `Grant<O, D>` va `Revoke<O, D>`, `Permission`/`Role` dan `Account` va `TriggerEvent::Created(TriggerId)` va Norito.- Hisobga ruxsat berish: agar o'ziga xos bo'lmasa, `Permission` qo'shadi. Voqealar: `AccountEvent::PermissionAdded`. Xatolar: agar dublikat bo'lsa, `Repetition(Grant, Permission)`. Kod: `core/.../isi/account.rs`.
- Hisobdan ruxsatni bekor qilish: agar mavjud bo'lsa, o'chiradi. Voqealar: `AccountEvent::PermissionRemoved`. Xatolar: agar yo'q bo'lsa, `FindError::Permission`. Kod: `core/.../isi/account.rs`.
- Hisobga rol berish: agar mavjud bo'lmasa, `(account, role)` xaritasini qo'shadi. Voqealar: `AccountEvent::RoleGranted`. Xatolar: `Repetition(Grant, RoleId)`. Kod: `core/.../isi/account.rs`.
- Hisobdan rolni bekor qilish: agar mavjud bo'lsa, xaritalashni olib tashlaydi. Voqealar: `AccountEvent::RoleRevoked`. Xatolar: agar yo'q bo'lsa, `FindError::Role`. Kod: `core/.../isi/account.rs`.
- Rolga ruxsat berish: ruxsat qo'shilgan holda rolni qayta yaratadi. Voqealar: `RoleEvent::PermissionAdded`. Xatolar: `Repetition(Grant, Permission)`. Kod: `core/.../isi/world.rs`.
- Roldan ruxsatni bekor qilish: bu ruxsatsiz rolni qayta yaratadi. Voqealar: `RoleEvent::PermissionRemoved`. Xatolar: agar yo'q bo'lsa, `FindError::Permission`. Kod: `core/.../isi/world.rs`.### Triggerlar: Bajarish
Turi: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Xulq-atvor: trigger quyi tizimi uchun `ExecuteTriggerEvent { trigger_id, authority, args }`ni navbatga qo'yadi. Qo'lda bajarish faqat qo'ng'iroq bo'yicha triggerlar uchun ruxsat etiladi (`ExecuteTrigger` filtri); filtr mos kelishi kerak va qo'ng'iroq qiluvchi trigger harakat vakolati bo'lishi yoki bu vakolat uchun `CanExecuteTrigger` ushlab turishi kerak. Foydalanuvchi tomonidan taqdim etilgan ijrochi faol bo'lsa, ishga tushirish vaqti ijrochisi tomonidan tasdiqlanadi va tranzaksiya ijrochisi yoqilg'i byudjetini sarflaydi (baza `executor.fuel` va ixtiyoriy metama'lumotlar `additional_fuel`).
- Xatolar: ro'yxatdan o'tmagan bo'lsa, `FindError::Trigger`; `InvariantViolation`, agar nodavlat tomonidan chaqirilgan bo'lsa. Kod: `core/.../isi/triggers/mod.rs` (va `core/.../smartcontracts/isi/mod.rs` da testlar).

### Yangilash va jurnalga kirish
- `Upgrade { executor }`: taqdim etilgan `Executor` baytekodidan foydalangan holda ijrochini ko'chiradi, ijrochini va uning ma'lumotlar modelini yangilaydi, `ExecutorEvent::Upgraded` chiqaradi. Xatolar: migratsiya xatosi `InvalidParameterError::SmartContract` sifatida o'ralgan. Kod: `core/.../isi/world.rs`.
- `Log { level, msg }`: berilgan darajadagi tugun jurnalini chiqaradi; holati o'zgarmaydi. Kod: `core/.../isi/world.rs`.

### Xato modeli
Umumiy konvert: `InstructionExecutionError` baholash xatolari, soʻrovlar xatosi, konvertatsiyalar, obʼyekt topilmadi, takrorlanish, zarb qilish qobiliyati, matematika, notoʻgʻri parametr va oʻzgarmas buzilish variantlari bilan. Ro'yxatlar va yordamchilar `crates/iroha_data_model/src/isi/mod.rs` da `pub mod error` ostida.

---## Tranzaksiyalar va bajariladigan fayllar
- `Executable`: `Instructions(ConstVec<InstructionBox>)` yoki `Ivm(IvmBytecode)`; bayt-kod base64 sifatida ketma-ketlashtiriladi. Kod: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: metama'lumotlar, `chain_id`, `authority`, `creation_time_ms`, ixtiyoriy I10300 va I08700, metama'lumotlar bilan bajariladigan faylni tuzadi, belgilaydi va paketlaydi. `nonce`. Kod: `crates/iroha_data_model/src/transaction/`.
- Ishlash vaqtida `iroha_core` tegishli `*Box` yoki aniq ko'rsatmalarga tushirib, `Execute for InstructionBox` orqali `InstructionBox` partiyalarini bajaradi. Kod: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Ish vaqti ijrochisini tekshirish budjeti (foydalanuvchi tomonidan taqdim etilgan ijrochi): parametrlardan asosiy `executor.fuel` va tranzaksiya ichidagi koʻrsatmalar/trigger tekshiruvlari boʻyicha taqsimlangan `additional_fuel` (`u64`) ixtiyoriy tranzaksiya metamaʼlumotlari.

---## Invariantlar va eslatmalar (sinovlar va qo'riqchilardan)
- Ibtido himoyasi: `genesis` domenini yoki `genesis` domenidagi hisoblarni ro'yxatdan o'tkaza olmaydi; `genesis` hisobini ro‘yxatdan o‘tkazib bo‘lmaydi. Kod/sinovlar: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Raqamli aktivlar o'zlarining `NumericSpec` zarb qilish/o'tkazish/yoqishda qondirishi kerak; spetsifikatsiyaning mos kelmasligi `TypeError::AssetNumericSpec` hosil qiladi.
- Mintability: `Once` bitta yalpizga ruxsat beradi va keyin `Not` ga aylanadi; `Limited(n)` `Not` ga oʻtishdan oldin aynan `n` zarb qilish imkonini beradi. `Infinitely` da zarb qilishni taqiqlashga urinishlar `MintabilityError::ForbidMintOnMintable` ni keltirib chiqaradi va `Limited(0)` konfiguratsiyasi `MintabilityError::InvalidMintabilityTokens` hosil qiladi.
- Meta-ma'lumotlar operatsiyalari aniq; mavjud bo'lmagan kalitni olib tashlash xatodir.
- Trigger filtrlari sinib bo'lmaydigan bo'lishi mumkin; keyin `Register<Trigger>` faqat `Exactly(1)` takrorlanishiga ruxsat beradi.
- Trigger metama'lumotlar kaliti `__enabled` (bool) eshiklarini bajarish; yo'qolgan standart sozlamalar yoqilgan va o'chirilgan triggerlar ma'lumotlar/vaqt/chaqiruv yo'llari bo'ylab o'tkazib yuboriladi.
- Determinizm: barcha arifmetik tekshirilgan amallardan foydalanadi; under/overflow terilgan matematik xatolarni qaytaradi; nol qoldiqlari aktiv yozuvlarini tushiradi (yashirin holat yo'q).

---## Amaliy misollar
- zarb qilish va topshirish:
  - `Mint::asset_numeric(10, asset_id)` → 10 qo'shadi, agar spetsifikatsiya/mintability tomonidan ruxsat etilgan bo'lsa; voqealar: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 ta harakat qiladi; olib tashlash/qo'shish uchun hodisalar.
- Metadata yangilanishlari:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → yuqoriga; `RemoveKeyValue::account(...)` orqali olib tashlash.
- Rol/ruxsat boshqaruvi:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` va ularning `Revoke` hamkasblari.
- Trigger hayot aylanishi:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`, filtr tomonidan nazarda tutilgan zarblik tekshiruvi bilan; `ExecuteTrigger::new(id).with_args(&args)` sozlangan vakolatga mos kelishi kerak.
  - Triggerlarni `__enabled` metamaʼlumotlar kalitini `false` ga oʻrnatish orqali oʻchirib qoʻyish mumkin (yoqilgan uchun standart sozlamalar yoʻq); `SetKeyValue::trigger` yoki IVM `set_trigger_enabled` tizimi orqali almashtirish.
  - Trigger xotirasi yuklanganda tuzatiladi: takroriy identifikatorlar, mos kelmaydigan identifikatorlar va etishmayotgan baytekodga havola qiluvchi triggerlar o'chiriladi; bayt-kod mos yozuvlar soni qayta hisoblab chiqiladi.
  - If a trigger's IVM bytecode is missing at execution time, the trigger is removed and the execution is treated as a no-op with a failure outcome.
  - tugatilgan triggerlar darhol olib tashlanadi; agar ijro paytida tugallangan yozuvga duch kelsa, u kesiladi va yo'qolgan deb hisoblanadi.
- Parametrlarni yangilash:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` yangilanadi va `ConfigurationEvent::Changed` chiqaradi.CLI / Torii `aid` + taxalluslarga misollar:
- Kanonik yordam + aniq ism + uzun taxallus bilan ro'yxatdan o'ting:
  - `iroha ledger asset definition register --id aid:2f17c72466f84a4bb8a8e24884fdcd2f --name pkr --alias pkr#ubl@sbp`
- Kanonik yordam + aniq ism + qisqa taxallus bilan ro'yxatdan o'ting:
  - `iroha ledger asset definition register --id aid:550e8400e29b41d4a7164466554400dd --name pkr --alias pkr#sbp`
- Taxallus + hisob komponentlari bo'yicha mint:
  - `iroha ledger asset mint --definition-alias pkr#ubl@sbp --account <i105> --quantity 500`
- Kanonik yordam uchun taxallusni hal qiling:
  - JSON `{ "alias": "pkr#ubl@sbp" }` bilan `POST /v2/assets/aliases/resolve`

Migratsiya eslatmasi:
- `name#domain` matnli obyekt taʼrifi identifikatorlari birinchi versiyada ataylab qoʻllab-quvvatlanmaydi.
- Yalpish/yonish/o'tkazish chegaralaridagi aktiv identifikatorlari `norito:<hex>` kanonik bo'lib qoladi; `iroha tools encode asset-id` bilan `--definition aid:...` yoki `--alias ...` plus `--account` dan foydalaning.

---

## Kuzatilish (tanlangan manbalar)
 - Ma'lumotlar modeli yadrosi: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ta'riflari va reestri: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI bajarilishi: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Voqealar: `crates/iroha_data_model/src/events/**`.
 - Bitimlar: `crates/iroha_data_model/src/transaction/**`.

Agar siz ushbu spetsifikatsiyani ko'rsatilgan API/xulq-atvor jadvaliga kengaytirilishini yoki har bir aniq hodisa/xato bilan o'zaro bog'lanishini istasangiz, so'zni ayting va men uni kengaytiraman.