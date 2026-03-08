---
lang: az
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Məlumat Modeli və ISI — Tətbiqdən Gələn Xüsusiyyət

Bu spesifikasiya dizaynın nəzərdən keçirilməsinə kömək etmək üçün `iroha_data_model` və `iroha_core` arasında mövcud tətbiqdən tərsinə hazırlanmışdır. Geri işarələrdəki yollar səlahiyyətli koda işarə edir.

## Əhatə dairəsi
- Kanonik qurumları (domenlər, hesablar, aktivlər, NFT-lər, rollar, icazələr, həmyaşıdlar, tetikler) və onların identifikatorlarını müəyyən edir.
- Vəziyyəti dəyişən təlimatları (ISI) təsvir edir: növlər, parametrlər, ilkin şərtlər, vəziyyət keçidləri, buraxılan hadisələr və səhv şərtləri.
- Parametrlərin idarə edilməsini, əməliyyatları və təlimatların seriyalaşdırılmasını ümumiləşdirir.

Determinizm: Bütün təlimat semantikası aparatdan asılı davranış olmadan təmiz vəziyyət keçidləridir. Serializasiya Norito istifadə edir; VM bayt kodu IVM istifadə edir və zəncirvari icradan əvvəl host tərəfində təsdiqlənir.

---

## Müəssisələr və İdentifikatorlar
ID-lər `Display`/`FromStr` gediş-gəliş ilə sabit sətir formalarına malikdir. Ad qaydaları boşluqları və qorunan `@ # $` simvollarını qadağan edir.- `Name` — təsdiqlənmiş mətn identifikatoru. Qaydalar: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domen: `{ id, logo, metadata, owned_by }`. İnşaatçılar: `NewDomain`. Kod: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — kanonik ünvanlar `AccountAddress` (IH58 / `sora…` sıxılmış / hex) vasitəsilə istehsal olunur və Torii `AccountAddress::parse_encoded` vasitəsilə girişləri normallaşdırır. IH58 üstünlük verilən hesab formatıdır; `sora…` forması yalnız Sora UX üçün ikinci ən yaxşısıdır. Tanış `alias` (rejected legacy form) sətri yalnız marşrut ləqəbi kimi saxlanılır. Hesab: `{ id, metadata }`. Kod: `crates/iroha_data_model/src/account.rs`.
- Hesabın qəbulu siyasəti — domenlər `iroha:account_admission_policy` metadata açarı altında Norito-JSON `AccountAdmissionPolicy` saxlamaqla gizli hesab yaradılmasına nəzarət edir. Açar olmadıqda, zəncir səviyyəli fərdi parametr `iroha:default_account_admission_policy` standartı təmin edir; bu da olmadıqda, sabit standart `ImplicitReceive` (ilk buraxılış) olur. Siyasət teqləri `mode` (`ExplicitOnly` və ya `ImplicitReceive`) üstəgəl hər bir əməliyyat üçün isteğe bağlı (defolt `16`) və blok başına yaratma qapaqları, isteğe bağlı `implicit_creation_fee` hesabı (burnburn və ya sink), Aktiv tərifinə görə `min_initial_amounts` və isteğe bağlı `default_role_on_create` (`AccountCreated`-dən sonra verilir, itkin olduqda `DefaultRoleError` ilə rədd edilir). Genesis qoşula bilməz; əlil/etibarsız siyasətlər `InstructionExecutionError::AccountAdmission` ilə naməlum hesablar üçün qəbz tipli təlimatları rədd edir. `AccountCreated`-dən əvvəl gizli hesablar `iroha:created_via="implicit"` metadata möhürü; defolt rollar növbəti `AccountRoleGranted` yayır və icraçı sahibinin əsas qaydaları yeni hesaba əlavə rollar olmadan öz aktivlərini/NFT-lərini xərcləməyə imkan verir. Kod: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Tərif: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Kod: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Kod: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Rol: `{ id, permissions: BTreeSet<Permission> }` inşaatçı ilə `NewRole { inner: Role, grant_to }`. Kod: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Kod: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — həmyaşıd şəxsiyyəti (ictimai açar) və ünvan. Kod: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Tətik: `{ id, action }`. Fəaliyyət: `{ executable, repeats, authority, filter, metadata }`. Kod: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` yoxlanılmış daxiletmə/çıxarma ilə. Kod: `crates/iroha_data_model/src/metadata.rs`.
- Abunə nümunəsi (tətbiq səviyyəsi): planlar `subscription_plan` metadatası olan `AssetDefinition` girişləridir; abunələr `subscription` metadatası olan `Nft` qeydləridir; faturalandırma, abunə NFT-lərinə istinad edən zaman tetikleyicileri tərəfindən həyata keçirilir. Baxın `docs/source/subscriptions_api.md` və `crates/iroha_data_model/src/subscription.rs`.
- **Kriptoqrafik primitivlər** (Xüsusiyyət `sm`):- `Sm2PublicKey` / `Sm2Signature` kanonik SEC1 nöqtəsini + SM2 üçün sabit enli `r∥s` kodlamasını əks etdirir. Konstruktorlar əyri üzvlüyü və fərqləndirici ID semantikasını (`DEFAULT_DISTID`) tətbiq edir, doğrulama isə səhv formalaşmış və ya yüksək diapazonlu skayarları rədd edir. Kod: `crates/iroha_crypto/src/sm.rs` və `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` GM/T 0004 həzmini Norito seriyalı `[u8; 32]` yeni tip kimi ifşa edir, manifestlərdə və ya telemetriyada hashlərin göründüyü yerdə istifadə olunur. Kod: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` 128-bit SM4 açarlarını təmsil edir və host sistemləri və verilənlər modeli qurğuları arasında paylaşılır. Kod: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Bu növlər mövcud Ed25519/BLS/ML-DSA primitivləri ilə yanaşı oturur və `sm` funksiyası işə salındıqdan sonra məlumat modeli istehlakçıları (Torii, SDK-lar, genezis alətləri) üçün əlçatan olur.

Vacib xüsusiyyətlər: `Identifiable`, `Registered`/`Registrable` (inşaatçı nümunəsi), `HasMetadata`, `IntoKeyValue`. Kod: `crates/iroha_data_model/src/lib.rs`.

Hadisələr: Hər bir varlıqda mutasiyalar üzrə yayılan hadisələr var (yarat/sil/sahibi dəyişdi/metadata dəyişdirildi və s.). Kod: `crates/iroha_data_model/src/events/`.

---

## Parametrlər (Zəncirvari Konfiqurasiya)
- Ailələr: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, plus `custom: BTreeMap`.
- Fərqlər üçün tək nömrələr: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Aqreqator: `Parameters`. Kod: `crates/iroha_data_model/src/parameter/system.rs`.

Parametrlərin qurulması (ISI): `SetParameter(Parameter)` müvafiq sahəni yeniləyir və `ConfigurationEvent::Changed` yayır. Kod: `crates/iroha_data_model/src/isi/transparent.rs`, `crates/iroha_core/src/smartcontracts/isi/world.rs`-də icraçı.

---

## Təlimatların Seriyalaşdırılması və Qeydiyyatı
- Əsas xüsusiyyət: `Instruction: Send + Sync + 'static`, `dyn_encode()`, `as_any()`, stabil `id()` (defolt olaraq konkret tip adına).
- `InstructionBox`: `Box<dyn Instruction>` sarğı. Clone/Eq/Ord `(type_id, encoded_bytes)` üzərində işləyir, beləliklə bərabərlik dəyərə görədir.
- Norito seriyası `InstructionBox` üçün `(String wire_id, Vec<u8> payload)` kimi seriallaşdırılır (tel identifikasiyası yoxdursa, `type_name`-ə qayıdır). Deserializasiya konstruktorlar üçün qlobal `InstructionRegistry` xəritəçəkmə identifikatorlarından istifadə edir. Defolt reyestrə bütün daxili ISI daxildir. Kodu: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: Növlər, Semantika, Səhvlər
İcra `iroha_core::smartcontracts::isi`-də `Execute for <Instruction>` vasitəsilə həyata keçirilir. Aşağıda ictimai təsirlər, ilkin şərtlər, yayılan hadisələr və səhvlər sadalanır.

### Qeydiyyat / Qeydiyyatdan Çıxar
Növlər: `Register<T: Registered>` və `Unregister<T: Identifiable>`, konkret hədəfləri əhatə edən `RegisterBox`/`UnregisterBox` cəmi növləri ilə.

- Register Peer: dünya həmyaşıdları dəstinə əlavələr.
  - İlkin şərtlər: artıq mövcud olmamalıdır.
  - Hadisələr: `PeerEvent::Added`.
  - Səhvlər: dublikat varsa `Repetition(Register, PeerId)`; Axtarışlarda `FindError`. Kod: `core/.../isi/world.rs`.

- Domain Qeydiyyatı: `owned_by = authority` ilə `NewDomain`-dən qurulur. İcazə verilməyib: `genesis` domeni.
  - İlkin şərtlər: domenin mövcud olmaması; `genesis` deyil.
  - Hadisələr: `DomainEvent::Created`.
  - Səhvlər: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Kod: `core/.../isi/world.rs`.- Qeydiyyat Hesabı: `NewAccount`-dən qurulur, `genesis` domenində icazə verilmir; `genesis` hesabı qeydiyyata alına bilməz.
  - İlkin şərtlər: domen mövcud olmalıdır; hesabın olmaması; genezis sahəsində deyil.
  - Hadisələr: `DomainEvent::Account(AccountEvent::Created)`.
  - Səhvlər: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Kod: `core/.../isi/domain.rs`.

- AssetDefinition-ı qeydiyyatdan keçirin: inşaatçıdan tikilir; dəstləri `owned_by = authority`.
  - İlkin şərtlər: yoxluğun tərifi; domen mövcuddur.
  - Hadisələr: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Səhvlər: `Repetition(Register, AssetDefinitionId)`. Kod: `core/.../isi/domain.rs`.

- NFT-ni qeydiyyatdan keçirin: inşaatçıdan tikilir; dəstləri `owned_by = authority`.
  - İlkin şərtlər: NFT-nin mövcud olmaması; domen mövcuddur.
  - Hadisələr: `DomainEvent::Nft(NftEvent::Created)`.
  - Səhvlər: `Repetition(Register, NftId)`. Kod: `core/.../isi/nft.rs`.

- Qeydiyyat Rolu: `NewRole { inner, grant_to }`-dən qurur (ilk sahib hesab-rol xəritəsi vasitəsilə qeydə alınıb), `inner: Role`-i saxlayır.
  - İlkin şərtlər: rolun olmaması.
  - Hadisələr: `RoleEvent::Created`.
  - Səhvlər: `Repetition(Register, RoleId)`. Kodu: `core/.../isi/world.rs`.

- Register Trigger: triggeri süzgəc növü ilə müəyyən edilmiş müvafiq tətikdə saxlayır.
  - İlkin şərtlər: Əgər filtr zərb edilə bilməzsə, `action.repeats` `Exactly(1)` olmalıdır (əks halda `MathError::Overflow`). Dublikat şəxsiyyət vəsiqələri qadağandır.
  - Hadisələr: `TriggerEvent::Created(TriggerId)`.
  - Səhvlər: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` konvertasiya/təsdiqləmə uğursuzluqları. Kod: `core/.../isi/triggers/mod.rs`.

- Peer/Domain/Account/AssetDefinition/NFT/Role/Triggerin qeydiyyatdan çıxarılması: hədəfi silir; silinmə hadisələrini yayır. Əlavə silsilələr:
  - Domeni qeydiyyatdan çıxarın: domendəki bütün hesabları, onların rollarını, icazələrini, tx ardıcıllığı sayğaclarını, hesab etiketlərini və UAID bağlamalarını silir; onların aktivlərini (və hər bir aktiv metadatasını) silir; domendəki bütün aktiv təriflərini silir; domendəki NFT-ləri və silinmiş hesablara məxsus hər hansı NFT-ləri silir; səlahiyyət domeni uyğun gələn tetikleyicileri aradan qaldırır. Hadisələr: `DomainEvent::Deleted`, üstəgəl hər bir element silinmə hadisələri. Səhvlər: itkin olduqda `FindError::Domain`. Kodu: `core/.../isi/world.rs`.
  - Hesabı Qeydiyyatdan Çıxar: hesabın icazələrini, rollarını, tx ardıcıllığı sayğacını, hesab etiketinin xəritələşdirilməsini və UAID bağlamalarını silir; hesaba məxsus aktivləri (və hər bir aktiv metadatasını) silir; hesaba məxsus NFT-ləri silir; səlahiyyəti həmin hesab olan tetikleyicileri aradan qaldırır. Hadisələr: `AccountEvent::Deleted`, üstəgəl silinmiş NFT üçün `NftEvent::Deleted`. Səhvlər: itkin olduqda `FindError::Account`. Kod: `core/.../isi/domain.rs`.
  - AssetDefinition'ı qeydiyyatdan çıxarın: həmin tərifin bütün aktivlərini və onların hər bir aktiv metadatasını silir. Hadisələr: hər aktiv üçün `AssetDefinitionEvent::Deleted` və `AssetEvent::Deleted`. Səhvlər: `FindError::AssetDefinition`. Kod: `core/.../isi/domain.rs`.
  - NFT-nin qeydiyyatdan çıxarılması: NFT-ni silir. Hadisələr: `NftEvent::Deleted`. Səhvlər: `FindError::Nft`. Kod: `core/.../isi/nft.rs`.
  - Rolu Qeydiyyatdan Çıxar: əvvəlcə bütün hesablardan rolu ləğv edir; sonra rolu silir. Hadisələr: `RoleEvent::Deleted`. Səhvlər: `FindError::Role`. Kodu: `core/.../isi/world.rs`.
  - Tətiyi Unregister: əgər varsa, tətiyi aradan qaldırır; dublikatın qeydiyyatdan çıxarılması `Repetition(Unregister, TriggerId)` verir. Hadisələr: `TriggerEvent::Deleted`. Kod: `core/.../isi/triggers/mod.rs`.

### Nanə / Yandırmaq
Növlər: `Mint<O, D: Identifiable>` və `Burn<O, D: Identifiable>`, qutuda `MintBox`/`BurnBox`.- Aktiv (Rəqəm) nanə/yandır: qalıqları və tərifin `total_quantity`-ni tənzimləyir.
  - İlkin şərtlər: `Numeric` dəyəri `AssetDefinition.spec()` tələblərinə cavab verməlidir; `mintable` tərəfindən icazə verilən nanə:
    - `Infinitely`: həmişə icazə verilir.
    - `Once`: bir dəfə icazə verilir; ilk nanə `mintable`-i `Not`-ə çevirir və `AssetDefinitionEvent::MintabilityChanged`, üstəgəl audit üçün ətraflı `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` buraxır.
    - `Limited(n)`: `n` əlavə nanə əməliyyatlarına imkan verir. Hər bir uğurlu nanə sayğacı azaldır; sıfıra çatdıqda tərif `Not`-ə çevrilir və yuxarıdakı kimi eyni `MintabilityChanged` hadisələrini yayır.
    - `Not`: xəta `MintabilityError::MintUnmintable`.
  - Dövlət dəyişiklikləri: zərfdə itkin olduqda aktiv yaradır; Yanma zamanı balans sıfır olarsa, aktiv girişini silir.
  - Hadisələr: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (`Once` və ya `Limited(n)` öz ehtiyatını tükəndikdə).
  - Səhvlər: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Kodu: `core/.../isi/asset.rs`.

- Tətik təkrarları nanə/yandır: dəyişikliklər `action.repeats` bir tətik üçün sayılır.
  - İlkin şərtlər: nanə üzərində süzgəc zərb oluna bilən olmalıdır; arifmetik daşmamalı/aşmamalıdır.
  - Hadisələr: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Səhvlər: etibarsız nanə üzərində `MathError::Overflow`; Əgər itkinsə `FindError::Trigger`. Kodu: `core/.../isi/triggers/mod.rs`.

### Transfer
Növlər: `Transfer<S: Identifiable, O, D: Identifiable>`, qutuda `TransferBox`.

- Aktiv (Rəqəm): mənbədən `AssetId` çıxın, təyinat yeri `AssetId` əlavə edin (eyni tərif, fərqli hesab). Sıfırlanmış mənbə aktivini silin.
  - İlkin şərtlər: mənbə aktivi mövcuddur; dəyər `spec` uyğun gəlir.
  - Hadisələr: `AssetEvent::Removed` (mənbə), `AssetEvent::Added` (təyinat).
  - Səhvlər: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Kod: `core/.../isi/asset.rs`.

- Domen sahibliyi: `Domain.owned_by` təyinat hesabına dəyişir.
  - İlkin şərtlər: hər iki hesab mövcuddur; domen mövcuddur.
  - Hadisələr: `DomainEvent::OwnerChanged`.
  - Səhvlər: `FindError::Account/Domain`. Kod: `core/.../isi/domain.rs`.

- AssetDefinition mülkiyyəti: `AssetDefinition.owned_by` təyinat hesabına dəyişir.
  - İlkin şərtlər: hər iki hesab mövcuddur; tərif mövcuddur; mənbə hazırda ona sahib olmalıdır.
  - Hadisələr: `AssetDefinitionEvent::OwnerChanged`.
  - Səhvlər: `FindError::Account/AssetDefinition`. Kod: `core/.../isi/account.rs`.

- NFT mülkiyyəti: `Nft.owned_by` təyinat hesabına dəyişir.
  - İlkin şərtlər: hər iki hesab mövcuddur; NFT mövcuddur; mənbə hazırda ona sahib olmalıdır.
  - Hadisələr: `NftEvent::OwnerChanged`.
  - Səhvlər: mənbə NFT-yə sahib deyilsə, `FindError::Account/Nft`, `InvariantViolation`. Kodu: `core/.../isi/nft.rs`.

### Metaməlumatlar: Açar-Dəyəri təyin edin/Silin
Növlər: `SetKeyValue<T>` və `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` ilə `RemoveKeyValue<T>`. Qutulu nömrələr verilir.

- Set: `Metadata[key] = Json(value)` daxil edir və ya əvəz edir.
- Sil: açarı çıxarır; yoxdursa səhv.
- Hadisələr: köhnə/yeni dəyərlərlə `<Target>Event::MetadataInserted` / `MetadataRemoved`.
- Səhvlər: hədəf mövcud deyilsə, `FindError::<Target>`; `FindError::MetadataKey` çıxarılmaq üçün çatışmayan açar. Kod: `crates/iroha_data_model/src/isi/transparent.rs` və hər hədəf üçün icraçı göstərişləri.### İcazələr və Rollar: Qrant / Ləğv et
Növlər: `Grant<O, D>` və `Revoke<O, D>`, `Permission`/`Role`-dən `Account`-ə qədər və Norito-dan Norito-ə qədər qutulu nömrələrlə.

- Hesaba İcazə verin: artıq xas olmadığı halda `Permission` əlavə edir. Hadisələr: `AccountEvent::PermissionAdded`. Səhvlər: dublikat varsa `Repetition(Grant, Permission)`. Kod: `core/.../isi/account.rs`.
- Hesabdan İcazəni Ləğv et: varsa, silinir. Hadisələr: `AccountEvent::PermissionRemoved`. Səhvlər: `FindError::Permission` olmadıqda. Kod: `core/.../isi/account.rs`.
- Hesaba Rol Grant: yoxdursa, `(account, role)` xəritəsini daxil edir. Hadisələr: `AccountEvent::RoleGranted`. Səhvlər: `Repetition(Grant, RoleId)`. Kodu: `core/.../isi/account.rs`.
- Hesabdan Rolu Ləğv et: varsa, xəritələməni silir. Hadisələr: `AccountEvent::RoleRevoked`. Səhvlər: `FindError::Role` yoxdursa. Kodu: `core/.../isi/account.rs`.
- Rol üçün icazə verin: əlavə icazə ilə rolu yenidən qurur. Hadisələr: `RoleEvent::PermissionAdded`. Səhvlər: `Repetition(Grant, Permission)`. Kod: `core/.../isi/world.rs`.
- Roldan İcazəni Ləğv et: bu icazə olmadan rolu yenidən qurur. Hadisələr: `RoleEvent::PermissionRemoved`. Səhvlər: olmadıqda `FindError::Permission`. Kod: `core/.../isi/world.rs`.

### Tətiklər: İcra edin
Növ: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Davranış: trigger alt sistemi üçün `ExecuteTriggerEvent { trigger_id, authority, args }` növbəsini saxlayır. Əllə icraya yalnız çağırış üzrə tetikleyiciler üçün icazə verilir (`ExecuteTrigger` filtri); filtr uyğun olmalıdır və zəng edən şəxs tetikleyici fəaliyyət orqanı olmalıdır və ya həmin səlahiyyət üçün `CanExecuteTrigger` saxlamalıdır. İstifadəçi tərəfindən təmin edilmiş icraçı aktiv olduqda, tətik icrası icra vaxtı icraçısı tərəfindən təsdiqlənir və əməliyyatın icraçısının yanacaq büdcəsini sərf edir (baza `executor.fuel` və əlavə metadata `additional_fuel`).
- Səhvlər: qeydiyyatdan keçməyibsə `FindError::Trigger`; `InvariantViolation` səlahiyyətli olmayan şəxs tərəfindən çağırılırsa. Kod: `core/.../isi/triggers/mod.rs` (və testlər `core/.../smartcontracts/isi/mod.rs`).

### Təkmilləşdirin və daxil olun
- `Upgrade { executor }`: təmin edilmiş `Executor` bayt kodundan istifadə edərək icraçını köçürür, icraçı və onun məlumat modelini yeniləyir, `ExecutorEvent::Upgraded` yayır. Səhvlər: miqrasiya uğursuzluğunda `InvalidParameterError::SmartContract` kimi bükülmüşdür. Kod: `core/.../isi/world.rs`.
- `Log { level, msg }`: verilmiş səviyyə ilə qovşaq jurnalını buraxır; vəziyyət dəyişmir. Kod: `core/.../isi/world.rs`.

### Xəta Modeli
Ümumi zərf: `InstructionExecutionError` qiymətləndirmə xətaları, sorğu uğursuzluqları, çevrilmələr, tapılmayan obyekt, təkrarlama, zərb alətləri, riyaziyyat, etibarsız parametr və dəyişməz pozuntu variantları ilə. Sadalamalar və köməkçilər `crates/iroha_data_model/src/isi/mod.rs` daxilində `pub mod error` altındadır.

---## Əməliyyatlar və İcra olunanlar
- `Executable`: ya `Instructions(ConstVec<InstructionBox>)`, ya da `Ivm(IvmBytecode)`; bayt kodu base64 kimi seriallaşdırılır. Kodu: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: metadata, `chain_id`, `authority`, `creation_time_ms`, isteğe bağlı I00300 və metadata ilə icra olunanı qurur, işarələyir və paketləyir. `nonce`. Kodu: `crates/iroha_data_model/src/transaction/`.
- İş vaxtı `iroha_core` müvafiq `*Box` və ya konkret təlimata endirərək `Execute for InstructionBox` vasitəsilə `InstructionBox` partiyalarını icra edir. Kod: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- İcra vaxtı icraçısının doğrulama büdcəsi (istifadəçi tərəfindən təmin edilən icraçı): parametrlərdən əsas `executor.fuel` və əməliyyat daxilində təlimat/tətikləmə yoxlamaları arasında paylaşılan isteğe bağlı əməliyyat metadata `additional_fuel` (`u64`).

---

## İnvariantlar və Qeydlər (testlərdən və qoruyuculardan)
- Yaradılış mühafizəsi: `genesis` domenini və ya `genesis` domenində hesabları qeydiyyatdan keçirə bilməz; `genesis` hesabı qeydiyyata alına bilməz. Kod/testlər: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Rəqəm aktivləri nağdlaşdırma/köçürmə/yandırma zamanı `NumericSpec` tələblərinə cavab verməlidir; spesifik uyğunsuzluq `TypeError::AssetNumericSpec` verir.
- Zərərlilik: `Once` tək nanəyə imkan verir və sonra `Not`-ə çevrilir; `Limited(n)`, `Not`-ə keçməzdən əvvəl dəqiq `n` nanələrə icazə verir. `Infinitely` üzərində zərbi qadağan etmək cəhdləri `MintabilityError::ForbidMintOnMintable` səbəb olur və `Limited(0)` konfiqurasiyası `MintabilityError::InvalidMintabilityTokens` verir.
- Metadata əməliyyatları açar-dəqiqdir; mövcud olmayan açarın silinməsi xətadır.
- Tətik filtrləri döyülməz ola bilər; sonra `Register<Trigger>` yalnız `Exactly(1)` təkrarlarına icazə verir.
- Tetikleyici metadata açarı `__enabled` (bool) qapıların icrası; buraxılmış defoltlar aktivləşdirilib və söndürülmüş tetikler data/zaman/zəng yolları üzrə atlanır.
- Determinizm: bütün arifmetik yoxlanılan əməliyyatlardan istifadə edir; under/overflow tipli riyazi səhvləri qaytarır; sıfır qalıqlar aktiv girişlərini buraxır (gizli vəziyyət yoxdur).

---## Praktik Nümunələr
- Zərb və köçürmə:
  - `Mint::asset_numeric(10, asset_id)` → spesifikasiyalar/zərbetmə qabiliyyəti ilə icazə verilirsə, 10 əlavə edir; hadisələr: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 hərəkət edir; çıxarılması/əlavə üçün hadisələr.
- Metadata yeniləmələri:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → yuxarı; `RemoveKeyValue::account(...)` vasitəsilə çıxarılması.
- Rol/icazənin idarə edilməsi:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` və onların `Revoke` analoqları.
- Trigger həyat dövrü:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`, süzgəc tərəfindən nəzərdə tutulan ədviyyat yoxlanışı ilə; `ExecuteTrigger::new(id).with_args(&args)` konfiqurasiya edilmiş səlahiyyətə uyğun olmalıdır.
  - `__enabled` metadata açarını `false`-a təyin etməklə triggerləri söndürmək olar (aktiv üçün defolt parametrlər yoxdur); `SetKeyValue::trigger` və ya IVM `set_trigger_enabled` sistem zəngi ilə keçid.
  - Tətik yaddaşı yükləndikdə təmir edilir: dublikat identifikatorlar, uyğun olmayan idlər və çatışmayan bayt koduna istinad edən triggerlər atılır; bayt kodu istinad sayları yenidən hesablanır.
  - Əgər icra zamanı triggerin IVM bayt kodu çatışmırsa, tetikleyici silinir və icra uğursuzluq nəticəsi ilə heç bir əməliyyat kimi qəbul edilir.
  - Tükənmiş tətiklər dərhal aradan qaldırılır; icra zamanı tükənmiş bir girişə rast gəlinərsə, o, budanır və itkin hesab edilir.
- Parametr yeniləməsi:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` yeniləyir və `ConfigurationEvent::Changed` yayır.

---

## İzləmə (seçilmiş mənbələr)
 - Məlumat modeli nüvəsi: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI tərifləri və reyestri: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI icrası: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Hadisələr: `crates/iroha_data_model/src/events/**`.
 - Əməliyyatlar: `crates/iroha_data_model/src/transaction/**`.

Bu spesifikasiyanın göstərilən API/davranış cədvəlində genişləndirilməsini və ya hər bir konkret hadisə/səhvlə çarpaz əlaqələndirilməsini istəyirsinizsə, sözü deyin və mən onu genişləndirəcəyəm.