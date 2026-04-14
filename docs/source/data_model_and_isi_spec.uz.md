<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8d13f6d206f60d31217ed093a5bbedd7946d27b644f9b3321a577cc6065a901
source_last_modified: "2026-03-30T18:22:55.965549+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Iroha v2 ma'lumotlar modeli va ISI — Amalga oshirishdan olingan spetsifikatsiya

Ushbu spetsifikatsiya dizaynni ko'rib chiqishga yordam berish uchun `iroha_data_model` va `iroha_core` bo'ylab joriy dasturdan teskari ishlab chiqilgan. Backticklardagi yo'llar vakolatli kodga ishora qiladi.

## Qo'llash doirasi
- Kanonik ob'ektlarni (domenlar, hisoblar, aktivlar, NFTlar, rollar, ruxsatlar, tengdoshlar, triggerlar) va ularning identifikatorlarini belgilaydi.
- Holatni o'zgartiruvchi ko'rsatmalarni (ISI) tavsiflaydi: turlar, parametrlar, old shartlar, holat o'tishlari, chiqarilgan hodisalar va xatolik shartlari.
- Parametrlarni boshqarish, tranzaktsiyalar va ko'rsatmalarni ketma-ketlashtirishni umumlashtiradi.

Determinizm: Barcha ko'rsatmalar semantikasi apparatga bog'liq bo'lmagan sof holatga o'tishdir. Serializatsiya Norito dan foydalanadi; VM baytkodi IVM dan foydalanadi va zanjirda bajarilishidan oldin xost tomonidan tekshiriladi.

---

## Ob'ektlar va identifikatorlar
Identifikatorlar `Display`/`FromStr` bo‘ylab sayohatga ega barqaror string shakllariga ega. Nom qoidalari bo'sh joy va ajratilgan `@ # $` belgilarni taqiqlaydi.- `Name` - tasdiqlangan matn identifikatori. Qoidalar: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domen: `{ id, logo, metadata, owned_by }`. Quruvchilar: `NewDomain`. Kod: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — kanonik manzillar `AccountAddress` orqali ishlab chiqariladi, chunki I105 va Torii `AccountAddress::parse_encoded` orqali kirishlarni normallashtiradi. Qattiq ish vaqtini tahlil qilish faqat kanonik I105 ni qabul qiladi. Zanjirdagi hisob taxalluslari `name@domain.dataspace` yoki `name@dataspace` dan foydalanadi va kanonik `AccountId` qiymatlariga mos keladi; ular qat'iy `AccountId` tahlilchilari tomonidan qabul qilinmaydi. Hisob: `{ id, metadata }`. Kod: `crates/iroha_data_model/src/account.rs`.- Hisobni qabul qilish siyosati — domenlar `iroha:account_admission_policy` metamaʼlumotlar kaliti ostida Norito-JSON `AccountAdmissionPolicy` ni saqlash orqali yashirin hisob yaratishni nazorat qiladi. Kalit yo'q bo'lganda, zanjir darajasidagi maxsus parametr `iroha:default_account_admission_policy` standartni ta'minlaydi; u ham bo'lmasa, qattiq standart `ImplicitReceive` (birinchi versiya). Siyosat teglari `mode` (`ExplicitOnly` yoki `ImplicitReceive`) va har bir tranzaksiya uchun ixtiyoriy (standart `16`) va har bir blok yaratish cheklovlari, ixtiyoriy `implicit_creation_fee` hisobi (`implicit_creation_fee`), Har bir aktiv taʼrifi boʻyicha `min_initial_amounts` va ixtiyoriy `default_role_on_create` (`AccountCreated` dan keyin beriladi, agar yoʻq boʻlsa, `DefaultRoleError` bilan rad etiladi). Ibtido ishtirok eta olmaydi; o'chirilgan/yaroqsiz siyosatlar `InstructionExecutionError::AccountAdmission` bilan noma'lum hisoblar uchun kvitansiya uslubidagi ko'rsatmalarni rad etadi. `AccountCreated` dan oldingi `iroha:created_via="implicit"` metamaʼlumotlarining yashirin hisob shtampi; standart rollar keyingi `AccountRoleGranted` ni chiqaradi va ijrochi egasining asosiy qoidalari yangi hisobning o'z aktivlarini/NFTlarini qo'shimcha rollarsiz sarflashga imkon beradi. Kod: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.- `AssetDefinitionId` — kanonik aktivni aniqlash baytlari ustidagi kanonik prefikssiz Base58 manzili. Bu ommaviy aktiv identifikatori. Ta'rif: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. `alias` literallari `<name>#<domain>.<dataspace>` yoki `<name>#<dataspace>` boʻlishi kerak, `<name>` obyekt taʼrifi nomiga teng va ular faqat kanonik Base58 obyekt identifikatoriga mos keladi. Kod: `crates/iroha_data_model/src/asset/definition.rs`.
  - Ijara taxallusi metamaʼlumotlari saqlangan obyekt taʼrifi qatoridan alohida saqlanadi. Core/Torii ta'riflar o'qilganda majburiy yozuvdan `alias` ni amalga oshiradi.
  - Torii aktivlarni aniqlash javoblari `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, bu yerda `status` `permanent`, `leased_active`, `leased_grace` yoki I0104.
  - Taxallus selektorlari so'nggi belgilangan blok yaratish vaqtiga qarshi hal qiladi. `grace_until_ms` dan so'ng, taxallus tanlagichlari fonni tozalash eski bog'lanishni hali olib tashlamagan bo'lsa ham echishni to'xtatadi; to'g'ridan-to'g'ri aniqlangan o'qishlar hali ham `expired_pending_cleanup` sifatida eski bog'lanish haqida xabar berishi mumkin.
- `AssetId`: kanonik yalang'och Base58 shaklidagi davlat aktivlari identifikatori. `name#dataspace` yoki `name#domain.dataspace` kabi obyekt taxalluslari `AssetId` ga mos keladi. Ichki buxgalteriya xoldinglari qo'shimcha ravishda kerak bo'lganda `asset + account + optional dataspace` bo'lingan maydonlarni ochishi mumkin, ammo bu kompozit shakl ommaviy `AssetId` emas.
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Kod: `crates/iroha_data_model/src/nft.rs`.- `RoleId` — `name`. Rol: `{ id, permissions: BTreeSet<Permission> }` quruvchi `NewRole { inner: Role, grant_to }` bilan. Kod: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Kod: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — tengdosh identifikatori (ommaviy kalit) va manzil. Kod: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Trigger: `{ id, action }`. Harakat: `{ executable, repeats, authority, filter, metadata }`. Kod: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` qoʻshish/olib tashlash tekshirilgan. Kod: `crates/iroha_data_model/src/metadata.rs`.
- Obuna namunasi (ilova qatlami): rejalar `subscription_plan` metama'lumotlariga ega `AssetDefinition` yozuvlari; obunalar `subscription` metama'lumotlariga ega `Nft` yozuvlari; hisob-kitob obuna NFTlariga havola qiluvchi vaqt triggerlari orqali amalga oshiriladi. `docs/source/subscriptions_api.md` va `crates/iroha_data_model/src/subscription.rs` ga qarang.
- **Kriptografik primitivlar** (`sm` xususiyati):
  - `Sm2PublicKey` / `Sm2Signature` kanonik SEC1 nuqtasini + SM2 uchun qattiq kenglikdagi `r∥s` kodlashni aks ettiradi. Konstruktorlar egri chiziqli a'zolikni va farqlovchi ID semantikasini (`DEFAULT_DISTID`) ta'minlaydi, tekshirish esa noto'g'ri shakllangan yoki yuqori diapazonli skalerlarni rad etadi. Kod: `crates/iroha_crypto/src/sm.rs` va `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` GM/T 0004 dayjestini Norito-seriyali `[u8; 32]` yangi turi sifatida manifestlarda yoki telemetriyada xeshlar paydo bo'lganda ishlatiladi. Kod: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` 128 bitli SM4 kalitlarini ifodalaydi va xost tizimlari va ma'lumotlar modeli qurilmalari o'rtasida taqsimlanadi. Kod: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Bu turlar mavjud Ed25519/BLS/ML-DSA primitivlari bilan birga joylashadi va `sm` funksiyasi yoqilgandan keyin maʼlumotlar modeli isteʼmolchilari (Torii, SDK, genezis asboblari) foydalanishi mumkin.
- Ma'lumotlar maydonidan olingan aloqalar do'konlari (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, tarmoqli releyli favqulodda vaziyatni bekor qilish registrlari) va ma'lumotlar maydoni uchun maqsadli ruxsatlar (permissions of dataspace-target permissions/8NI0le00/8NI1000) do'konlar) ma'lumotlar bo'shliqlari faol `dataspace_catalog` dan yo'qolganda, `State::set_nexus(...)` da kesiladi, bu esa ish vaqti katalogi yangilanishlaridan keyin eskirgan ma'lumotlar maydoni havolalarini oldini oladi. Yo‘lak ko‘lamli DA/rele keshlari (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) shuningdek, yo‘l o‘chirilganda yoki boshqa ma’lumotlar maydoniga qayta tayinlanganda kesiladi. Space Directory ISI'lar (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) shuningdek, `dataspace` ni faol katalogga nisbatan tasdiqlaydi va `InvalidParameter` bilan noma'lum identifikatorlarni rad etadi.

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
- `InstructionBox` uchun Norito serde `(String wire_id, Vec<u8> payload)` sifatida seriyalanadi (agar sim identifikatori bo'lmasa, `type_name` ga qaytadi). Deserializatsiya konstruktorlar uchun global `InstructionRegistry` xaritalash identifikatorlaridan foydalanadi. Standart registr barcha o'rnatilgan ISIni o'z ichiga oladi. Kod: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: turlari, semantikasi, xatolari
Amalga oshirish `iroha_core::smartcontracts::isi` da `Execute for <Instruction>` orqali amalga oshiriladi. Quyida ommaviy effektlar, old shartlar, chiqarilgan hodisalar va xatolar ro'yxati keltirilgan.

### Ro'yxatdan o'tish / Ro'yxatdan o'tish
Turlari: `Register<T: Registered>` va `Unregister<T: Identifiable>`, `RegisterBox`/`UnregisterBox` yig'indisi aniq maqsadlarni qamrab oladi.- Register Peer: dunyodagi tengdoshlar to'plamiga qo'shimchalar.
  - Old shartlar: mavjud bo'lmasligi kerak.
  - Voqealar: `PeerEvent::Added`.
  - Xatolar: `Repetition(Register, PeerId)` agar takroriy bo'lsa; Qidiruvlarda `FindError`. Kod: `core/.../isi/world.rs`.

- Domenni ro'yxatdan o'tkazish: `NewDomain` dan `owned_by = authority` bilan tuziladi. Ruxsat berilmagan: `genesis` domeni.
  - Old shartlar: domenning mavjud emasligi; emas `genesis`.
  - Voqealar: `DomainEvent::Created`.
  - Xatolar: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Kod: `core/.../isi/world.rs`.

- Hisob qaydnomasini ro'yxatdan o'tkazish: `genesis` domenida taqiqlangan, `NewAccount` dan tuziladi; `genesis` hisobini ro‘yxatdan o‘tkazib bo‘lmaydi.
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
  - Xatolar: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` konvertatsiya/tasdiqlashda xatoliklar. Kod: `core/.../isi/triggers/mod.rs`.- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger registrini bekor qilish: maqsadni olib tashlaydi; oʻchirish hodisalarini chiqaradi. Qo'shimcha kaskadli olib tashlashlar:- Domenni ro'yxatdan o'chirish: domen ob'ektini va uning selektor/tasdiqlash siyosati holatini olib tashlaydi; domendagi aktiv taʼriflarini (va shu taʼriflar bilan kalitlangan `zk_assets` maxfiy holati), ushbu taʼriflar aktivlarini (va har bir aktiv metamaʼlumotlarini), domendagi NFTʼlarni va olib tashlangan domenga asoslangan hisob-taxallus prognozlarini oʻchiradi. Shuningdek, u olib tashlangan domenga yoki u bilan birga oʻchirilgan resurslarga (domen ruxsatnomalari, olib tashlangan taʼriflar uchun aktiv taʼrifi/aktiv ruxsatnomalari va olib tashlangan NFT identifikatorlari uchun NFT ruxsatnomalari) havola qiluvchi hisob/rol doirasidagi ruxsatnomalarni oʻchiradi. Domenni olib tashlash global `AccountId`, uning tx-sequence/UAID holati, xorijiy aktiv yoki NFT egaligi, ishga tushirish vakolati yoki saqlanib qolgan hisobga ishora qiluvchi boshqa tashqi audit/konfiguratsiya maʼlumotnomalarini oʻchirib tashlamaydi yoki qayta yozmaydi. Qo‘riqlash panjaralari: domendagi har qanday aktiv ta’rifiga hali ham repo-kelishuvi, hisob-kitob kitobi, jamoat yo‘lidagi mukofot/da’vo, oflayn ruxsat/o‘tkazish, hisob-kitob repo defoltlari (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), boshqaruv-konfiguratsiyalangan/viruslilik-repo-shartnomasi orqali havola qilinganda rad etadi. aktiv-ta'rifi ma'lumotnomalari, oracle-economics sozlangan mukofot/slash/dispute-bond aktiv-ta'rifi ma'lumotnomalari yoki Nexus to'lovi/staking aktivi-ta'rifi ma'lumotnomalari (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Voqealar: `DomainEvent::Deleted`, shuningdek, olib tashlangan domen resursi uchun har bir elementni oʻchirish voqealarices. Xatolar: agar yo'q bo'lsa, `FindError::Domain`; `InvariantViolation` saqlangan obyekt taʼrifi mos yozuvlar ziddiyatlari. Kod: `core/.../isi/world.rs`.- Hisob qaydnomasini ro‘yxatdan o‘chirish: hisob ruxsatnomalari, rollari, tx-ketma-ket hisoblagichi, hisob yorlig‘i xaritasi va UAID bog‘lanishlarini olib tashlaydi; hisobga tegishli aktivlarni (va har bir aktiv metamaʼlumotlarini) oʻchirib tashlaydi; hisobga tegishli NFTlarni o'chiradi; vakolati shu hisob bo'lgan triggerlarni olib tashlaydi; oʻchirilgan hisob qaydnomasiga havola qiluvchi hisob/rol boʻyicha ruxsat yozuvlarini, olib tashlangan tegishli NFT identifikatorlari uchun hisob/rol boʻyicha NFT-maqsad ruxsatlarini va olib tashlangan triggerlar uchun hisob/rol doirasidagi trigger-maqsadli ruxsatlarni oʻchirib tashlaydi. Qo'riqlash bloklari: agar hisob hali ham domenga ega bo'lsa, ob'ekt ta'rifi, SoraFS provayder bilan bog'lanishi, faol fuqarolik rekordi, umumiy staking/mukofot holati (jumladan, hisob da'vogar yoki mukofot-aktiv egasi sifatida ko'rinadigan mukofot-da'vo kalitlari), faol oracle holati (jumladan, twitter-hisob-kitoblar, oracle-provayderlar-provayderlari)ni rad etadi yoki oracle-economics tomonidan konfiguratsiya qilingan mukofot/slash hisobi havolalari), faol Nexus toʻlovi/staking hisob qaydnomasi (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; kanonik sifatida tahlil qilingan), faol domensiz hisob qaydnomasi va faol domensiz hisob qaydnomasi muvaffaqiyatsiz tugadi. repo-kelishuv holati, faol hisob-kitoblar daftarchasi holati, faol oflayn imtiyoz/o‘tkazish yoki oflayn hukmni bekor qilish holati, faol aktiv ta’riflari uchun faol oflayn eskrou-hisob konfiguratsiyasi havolalari (`settlement.offline.escrow_accounts`), faol boshqaruv holati (taklif/bosqichni tasdiqlashals/locks/slashes/kengash/parlament ro'yxatlari, taklif parlamenti snapshotlari, ish vaqtini yangilash taklifchisi yozuvlari, boshqaruv tomonidan sozlangan escrow/slash-receiver/virusli-pul hisobi ma'lumotnomalari, boshqaruv SoraFS telemetriya ma'lumotnomalari I1070NI orqali `gov.sorafs_telemetry.per_provider_submitters` yoki `gov.sorafs_provider_owners` orqali boshqaruv tomonidan sozlangan SoraFS provayder-egasi maʼlumotnomalari, sozlangan kontentni nashr qilish ruxsatnomalari roʻyxati hisob qaydnomalari (`content.publish_allow_accounts`), faol ijtimoiy escrow joʻnatuvchisi holati, faol kontentni yuboruvchi holati, faol kontent egasining holati faol chiziqli reley favqulodda tekshiruvi bekor qilish holati yoki faol SoraFS pin-registr emitenti/bog'lovchi yozuvlari (pin manifestlari, manifest taxalluslari, replikatsiya buyurtmalari). Voqealar: `AccountEvent::Deleted`, shuningdek, olib tashlangan NFT uchun `NftEvent::Deleted`. Xatolar: agar yo'q bo'lsa, `FindError::Account`; `InvariantViolation` egalik etim bolalar. Kod: `core/.../isi/domain.rs`.- AssetDefinition registrini bekor qilish: ushbu taʼrifning barcha aktivlarini va ularning har bir aktiv metamaʼlumotlarini oʻchirib tashlaydi va shu taʼrif bilan kalitlangan `zk_assets` konfidentsial holatini oʻchiradi; shuningdek, oʻchirilgan aktiv taʼrifiga yoki uning aktiv namunalariga havola qiluvchi mos keladigan `settlement.offline.escrow_accounts` yozuvi va hisob/rol boʻyicha ruxsat yozuvlarini kesadi. Qo'riqlash panjaralari: ta'rif hali ham repo-kelishuvi, hisob-kitob kitobi, ommaviy yo'lak mukofoti/da'vosi, oflayn to'lov/o'tkazma holati, hisob-kitob repo defoltlari (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), boshqaruv tomonidan sozlangan ovoz berish/fuqarolik huquqiga ega bo'lgan taqdirda rad etadi. aktiv-ta'rifi ma'lumotnomalari, oracle-economics sozlangan mukofot/slash/nizo-obligatsiya aktiv-ta'rifi ma'lumotnomalari yoki Nexus to'lovi/staking aktivi ta'rifi ma'lumotnomalari (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Voqealar: har bir aktiv uchun `AssetDefinitionEvent::Deleted` va `AssetEvent::Deleted`. Xatolar: `FindError::AssetDefinition`, `InvariantViolation` mos yozuvlar ziddiyatlarida. Kod: `core/.../isi/domain.rs`.
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

- Obyekt (Raqamli): `AssetId` manbasidan ayiring, `AssetId` manziliga qo'shing (bir xil ta'rif, boshqa hisob). Nollangan manba obyektini oʻchirish.
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
  - Xatolar: `FindError::Account/Nft`, `InvariantViolation`, agar manba NFTga egalik qilmasa. Kod: `core/.../isi/nft.rs`.

### Metadata: Kalit-qiymatini o'rnatish/o'chirish
Turlari: `SetKeyValue<T>` va `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` bilan. Qutidagi raqamlar taqdim etiladi.

- O'rnatish: `Metadata[key] = Json(value)`ni qo'shadi yoki almashtiradi.
- Olib tashlash: kalitni olib tashlaydi; yo'q bo'lsa xato.
- Voqealar: eski/yangi qiymatlar bilan `<Target>Event::MetadataInserted` / `MetadataRemoved`.
- Xatolar: `FindError::<Target>`, agar maqsad mavjud bo'lmasa; Olib tashlash uchun etishmayotgan kalitda `FindError::MetadataKey`. Kod: `crates/iroha_data_model/src/isi/transparent.rs` va maqsad uchun ijrochi impls.

### Ruxsatlar va rollar: berish / bekor qilish
Turlari: `Grant<O, D>` va `Revoke<O, D>`, `Permission`/`Role` dan `Account` gacha va `Permission` va Norito.- Hisobga ruxsat berish: agar o'ziga xos bo'lmasa, `Permission` qo'shadi. Voqealar: `AccountEvent::PermissionAdded`. Xatolar: agar takroriy bo'lsa, `Repetition(Grant, Permission)`. Kod: `core/.../isi/account.rs`.
- Hisobdan ruxsatni bekor qilish: agar mavjud bo'lsa, o'chiradi. Voqealar: `AccountEvent::PermissionRemoved`. Xatolar: agar yo'q bo'lsa, `FindError::Permission`. Kod: `core/.../isi/account.rs`.
- Hisobga rol berish: agar mavjud bo'lmasa, `(account, role)` xaritasini qo'shadi. Voqealar: `AccountEvent::RoleGranted`. Xatolar: `Repetition(Grant, RoleId)`. Kod: `core/.../isi/account.rs`.
- Hisobdan rolni bekor qilish: agar mavjud bo'lsa, xaritalashni olib tashlaydi. Voqealar: `AccountEvent::RoleRevoked`. Xatolar: agar yo'q bo'lsa, `FindError::Role`. Kod: `core/.../isi/account.rs`.
- Rolga ruxsat berish: ruxsat qo'shilgan holda rolni qayta yaratadi. Voqealar: `RoleEvent::PermissionAdded`. Xatolar: `Repetition(Grant, Permission)`. Kod: `core/.../isi/world.rs`.
- Roldan ruxsatni bekor qilish: bu ruxsatsiz rolni qayta yaratadi. Voqealar: `RoleEvent::PermissionRemoved`. Xatolar: agar yo'q bo'lsa, `FindError::Permission`. Kod: `core/.../isi/world.rs`.### Triggerlar: Bajarish
Turi: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Xulq-atvor: trigger quyi tizimi uchun `ExecuteTriggerEvent { trigger_id, authority, args }`ni navbatga qo'yadi. Qo'lda bajarish faqat qo'ng'iroq bo'yicha triggerlar uchun ruxsat etiladi (`ExecuteTrigger` filtri); filtr mos kelishi kerak va qo'ng'iroq qiluvchi trigger harakat vakolati bo'lishi yoki ushbu vakolat uchun `CanExecuteTrigger` ushlab turishi kerak. Foydalanuvchi tomonidan taqdim etilgan ijrochi faol boʻlsa, ishga tushirish vaqti ijrochi tomonidan tasdiqlanadi va tranzaksiya ijrochisi yoqilgʻi byudjetini sarflaydi (asosiy `executor.fuel` va ixtiyoriy metamaʼlumotlar `additional_fuel`).
- Xatolar: ro'yxatdan o'tmagan bo'lsa, `FindError::Trigger`; `InvariantViolation`, agar nodavlat shaxs tomonidan chaqirilgan bo'lsa. Kod: `core/.../isi/triggers/mod.rs` (va testlar `core/.../smartcontracts/isi/mod.rs`).

### Yangilash va jurnalga kirish
- `Upgrade { executor }`: taqdim etilgan `Executor` baytekodidan foydalangan holda ijrochini ko'chiradi, ijrochini va uning ma'lumotlar modelini yangilaydi, `ExecutorEvent::Upgraded` chiqaradi. Xatolar: migratsiya xatosi `InvalidParameterError::SmartContract` sifatida o'ralgan. Kod: `core/.../isi/world.rs`.
- `Log { level, msg }`: berilgan darajadagi tugun jurnalini chiqaradi; holati o'zgarmaydi. Kod: `core/.../isi/world.rs`.

### Xato modeli
Umumiy konvert: `InstructionExecutionError` baholash xatolari, soʻrovlar xatosi, konvertatsiyalar, obʼyekt topilmadi, takrorlanish, zarb qilish qobiliyati, matematika, notoʻgʻri parametr va oʻzgarmas buzilish variantlari bilan. Ro'yxatlar va yordamchilar `crates/iroha_data_model/src/isi/mod.rs` da `pub mod error` ostida.

---## Tranzaksiyalar va bajariladigan fayllar
- `Executable`: `Instructions(ConstVec<InstructionBox>)` yoki `Ivm(IvmBytecode)`; bayt-kod base64 sifatida ketma-ketlashtiriladi. Kod: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: metamaʼlumotlar, `chain_id`, `authority`, `creation_time_ms`, ixtiyoriy I103NI0 va I08910 bilan metamaʼlumotlar bilan tuziladi, belgilanadi va paketlanadi. `nonce`. Kod: `crates/iroha_data_model/src/transaction/`.
- Ishlash vaqtida `iroha_core` tegishli `*Box` yoki aniq ko'rsatmalarga tushirib, `Execute for InstructionBox` orqali `InstructionBox` partiyalarini bajaradi. Kod: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Ish vaqti ijrochisini tekshirish budjeti (foydalanuvchi tomonidan taqdim etilgan ijrochi): parametrlardan asosiy `executor.fuel` va tranzaksiya ichidagi koʻrsatmalar/trigger tekshiruvlari boʻylab taqsimlangan `additional_fuel` (`u64`) ixtiyoriy tranzaksiya metamaʼlumotlari.

---## Invariantlar va eslatmalar (sinovlar va qo'riqchilardan)
- Genesis himoyasi: `genesis` domenini yoki `genesis` domenidagi hisoblarni ro'yxatdan o'tkaza olmaydi; `genesis` hisobini roʻyxatdan oʻtkazib boʻlmaydi. Kod/sinovlar: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Raqamli aktivlar o'zlarining `NumericSpec` zarb qilish/ko'chirish/yoqishda qondirishi kerak; spetsifikatsiyaning mos kelmasligi `TypeError::AssetNumericSpec` hosil qiladi.
- Mintability: `Once` bitta yalpizga ruxsat beradi va keyin `Not` ga aylanadi; `Limited(n)`, `Not` ga o'tishdan oldin aynan `n` zarb qilish imkonini beradi. `Infinitely` da zarb qilishni taqiqlashga urinishlar `MintabilityError::ForbidMintOnMintable` ni keltirib chiqaradi va `Limited(0)` konfiguratsiyasi `MintabilityError::InvalidMintabilityTokens` hosil qiladi.
- Meta-ma'lumotlar operatsiyalari aniq; mavjud bo'lmagan kalitni olib tashlash xatodir.
- Trigger filtrlari sinib bo'lmaydigan bo'lishi mumkin; keyin `Register<Trigger>` faqat `Exactly(1)` takrorlanishiga ruxsat beradi.
- Trigger metama'lumotlar kaliti `__enabled` (bool) eshiklarini bajarish; yo'qolgan standart sozlamalar yoqilgan va o'chirilgan triggerlar ma'lumotlar/vaqt/chaqiruv yo'llari bo'ylab o'tkazib yuboriladi.
- Determinizm: barcha arifmetik tekshirilgan amallardan foydalanadi; under/overflow terilgan matematik xatolarni qaytaradi; nol qoldiqlari aktiv yozuvlarini tushiradi (yashirin holat yo'q).

---## Amaliy misollar
- zarb qilish va topshirish:
  - `Mint::asset_numeric(10, asset_id)` → spetsifikatsiya/zarb qilish imkoniyati ruxsat etilgan bo'lsa, 10 qo'shadi; voqealar: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 ta harakat qiladi; olib tashlash/qo'shish uchun hodisalar.
- Metadata yangilanishlari:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → yuqoriga; `RemoveKeyValue::account(...)` orqali olib tashlash.
- Rol/ruxsat boshqaruvi:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` va ularning `Revoke` hamkasblari.
- Trigger hayot aylanishi:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`, filtr tomonidan nazarda tutilgan zarblik tekshiruvi bilan; `ExecuteTrigger::new(id).with_args(&args)` sozlangan vakolatga mos kelishi kerak.
  - Triggerlarni `__enabled` metamaʼlumotlar kalitini `false` ga oʻrnatish orqali oʻchirib qoʻyish mumkin (yoqilgan uchun standart sozlamalar yoʻq); `SetKeyValue::trigger` yoki IVM `set_trigger_enabled` tizimi orqali almashtirish.
  - Trigger xotirasi yuklanganda tuzatiladi: takroriy identifikatorlar, mos kelmaydigan identifikatorlar va etishmayotgan baytekodga havola qiluvchi triggerlar o'chiriladi; bayt-kod mos yozuvlar soni qayta hisoblab chiqiladi.
  - Agar ishga tushirish vaqtida triggerning IVM baytkodi yo'q bo'lsa, trigger o'chiriladi va bajarilish no-op sifatida ko'rib chiqiladi va natijada muvaffaqiyatsizlikka uchraydi.
  - tugatilgan triggerlar darhol olib tashlanadi; agar ijro paytida tugallangan yozuvga duch kelsa, u kesiladi va yo'qolgan deb hisoblanadi.
- Parametrlarni yangilash:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` yangilanadi va `ConfigurationEvent::Changed` chiqaradi.CLI / Torii aktiv ta'rifi identifikatori + taxallusga misollar:
- Kanonik Base58 id + aniq ism + uzun taxallus bilan ro'yxatdan o'ting:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- Kanonik Base58 identifikatori + aniq ism + qisqa taxallus bilan ro'yxatdan o'ting:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- Taxallus + hisob komponentlari bo'yicha mint:
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- Kanonik Base58 identifikatoriga taxallusni hal qiling:
  - JSON `{ "alias": "pkr#ubl.sbp" }` bilan `POST /v1/assets/aliases/resolve`

Migratsiya eslatmasi:
- `name#domain` matnli obyekt taʼrifi identifikatorlari birinchi versiyada ataylab qoʻllab-quvvatlanmaydi; kanonik Base58 identifikatorlaridan foydalaning yoki nuqtali taxallusni hal qiling.
- Ommaviy aktivlar tanlovchilari kanonik Base58 aktiv taʼrifi identifikatorlari va boʻlingan egalik maydonlaridan foydalanadi (`account`, ixtiyoriy `scope`). Xom kodlangan `AssetId` literallari ichki yordamchi bo'lib qoladi va Torii/CLI selektor yuzasining bir qismi emas.
- Obyektlar roʻyxati/soʻrov filtrlari va navlari qoʻshimcha ravishda `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` va `alias_binding.bound_at_ms`ni qabul qiladi.

---

## Kuzatilish (tanlangan manbalar)
 - Ma'lumotlar modeli yadrosi: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ta'riflari va reestri: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI bajarilishi: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Voqealar: `crates/iroha_data_model/src/events/**`.
 - Bitimlar: `crates/iroha_data_model/src/transaction/**`.

Agar siz ushbu spetsifikatsiyani ko'rsatilgan API/xulq-atvor jadvaliga kengaytirilishini yoki har bir aniq hodisa/xato bilan o'zaro bog'lanishini istasangiz, so'zni ayting va men uni kengaytiraman.