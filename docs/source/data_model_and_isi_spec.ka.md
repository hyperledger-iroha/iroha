---
lang: ka
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 მონაცემთა მოდელი და ISI — იმპლემენტაციისგან მიღებული სპეციფიკაცია

ეს სპეციფიკაცია შემუშავებულია `iroha_data_model`-სა და `iroha_core`-ში მიმდინარე იმპლემენტაციისგან, რათა ხელი შეუწყოს დიზაინის მიმოხილვას. ბილიკები უკანა ხაზში მიუთითებს ავტორიტეტულ კოდზე.

## სფერო
- განსაზღვრავს კანონიკურ ერთეულებს (დომენები, ანგარიშები, აქტივები, NFT-ები, როლები, ნებართვები, თანატოლები, ტრიგერები) და მათ იდენტიფიკატორებს.
- აღწერს მდგომარეობის შეცვლის ინსტრუქციებს (ISI): ტიპებს, პარამეტრებს, წინაპირობებს, მდგომარეობის გადასვლებს, ემიტირებული მოვლენებს და შეცდომის პირობებს.
- აჯამებს პარამეტრების მართვას, ტრანზაქციებს და ინსტრუქციების სერიალიზაციას.

დეტერმინიზმი: ყველა ინსტრუქციის სემანტიკა არის სუფთა მდგომარეობის გადასვლები, აპარატურაზე დამოკიდებული ქცევის გარეშე. სერიალიზაცია იყენებს Norito; VM ბაიტიკოდი იყენებს IVM-ს და დამოწმებულია ჰოსტის მხრიდან ჯაჭვზე შესრულებამდე.

---

## პირები და იდენტიფიკატორები
ID-ებს აქვთ სტაბილური სიმებიანი ფორმები `Display`/`FromStr` ორმხრივი მოგზაურობით. სახელის წესები კრძალავს უფსკრული და დაცული `@ # $` სიმბოლოები.- `Name` — დადასტურებული ტექსტური იდენტიფიკატორი. წესები: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. დომენი: `{ id, logo, metadata, owned_by }`. მშენებლები: `NewDomain`. კოდი: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — კანონიკური მისამართები იწარმოება `AccountAddress` (IH58 / `sora…` შეკუმშული / თექვსმეტობითი) საშუალებით და Torii ახდენს შეყვანის ნორმალიზებას Torii-ის მეშვეობით. IH58 არის ანგარიშის სასურველი ფორმატი; `sora…` ფორმა მეორე საუკეთესოა მხოლოდ Sora UX-ისთვის. ნაცნობი `alias@domain` სტრიქონი შენახულია მხოლოდ მარშრუტიზაციის მეტსახელად. ანგარიში: `{ id, metadata }`. კოდი: `crates/iroha_data_model/src/account.rs`.
- ანგარიშის დაშვების პოლიტიკა — დომენები აკონტროლებენ ანგარიშის იმპლიციურ შექმნას Norito-JSON `AccountAdmissionPolicy`-ის შენახვით მეტამონაცემების გასაღების `iroha:account_admission_policy`-ის ქვეშ. როდესაც გასაღები არ არის, ჯაჭვის დონის მორგებული პარამეტრი `iroha:default_account_admission_policy` უზრუნველყოფს ნაგულისხმევს; როდესაც ეს ასევე არ არის, მყარი ნაგულისხმევი არის `ImplicitReceive` (პირველი გამოშვება). პოლიტიკის თეგები `mode` (`ExplicitOnly` ან `ImplicitReceive`) პლუს არჩევითი თითო ტრანზაქცია (ნაგულისხმევი `16`) და თითო ბლოკის შექმნის ქუდები, არჩევითი `implicit_creation_fee` ანგარიში `min_initial_amounts` აქტივის განმარტებით და არასავალდებულო `default_role_on_create` (გაცემულია `AccountCreated`-ის შემდეგ, უარყოფილია `DefaultRoleError`-ით, თუ აკლია). Genesis-ს არ შეუძლია მონაწილეობა მიიღოს; გათიშული/არასწორი პოლიტიკა უარყოფს ქვითრის სტილის ინსტრუქციებს უცნობი ანგარიშებისთვის `InstructionExecutionError::AccountAdmission`-ით. იმპლიციტური ანგარიშების შტამპი მეტამონაცემების `iroha:created_via="implicit"` ადრე `AccountCreated`; ნაგულისხმევი როლები გამოსცემს შემდგომ `AccountRoleGranted`-ს, ხოლო შემსრულებლის მფლობელის საბაზისო წესები საშუალებას აძლევს ახალ ანგარიშს დახარჯოს საკუთარი აქტივები/NFT დამატებითი როლების გარეშე. კოდი: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. განმარტება: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. კოდი: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. კოდი: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. როლი: `{ id, permissions: BTreeSet<Permission> }` მშენებელთან `NewRole { inner: Role, grant_to }`. კოდი: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. კოდი: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — თანატოლების ვინაობა (საჯარო გასაღები) და მისამართი. კოდი: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. ტრიგერი: `{ id, action }`. მოქმედება: `{ executable, repeats, authority, filter, metadata }`. კოდი: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` მონიშნული ჩანართით/მოხსნით. კოდი: `crates/iroha_data_model/src/metadata.rs`.
- გამოწერის ნიმუში (აპლიკაციის ფენა): გეგმები არის `AssetDefinition` ჩანაწერები `subscription_plan` მეტამონაცემებით; გამოწერები არის `Nft` ჩანაწერები `subscription` მეტამონაცემებით; ბილინგი შესრულებულია დროითი ტრიგერით, რომელიც მიუთითებს გამოწერის NFT-ებზე. იხილეთ `docs/source/subscriptions_api.md` და `crates/iroha_data_model/src/subscription.rs`.
- **კრიპტოგრაფიული პრიმიტივები** (მახასიათებელი `sm`):- `Sm2PublicKey` / `Sm2Signature` ასახავს კანონიკურ SEC1 წერტილს + ფიქსირებული სიგანის `r∥s` კოდირებას SM2-ისთვის. კონსტრუქტორები ახორციელებენ მრუდის წევრობას და განმასხვავებელ ID სემანტიკას (`DEFAULT_DISTID`), ხოლო დადასტურება უარყოფს არასწორი ან მაღალი დიაპაზონის სკალერებს. კოდი: `crates/iroha_crypto/src/sm.rs` და `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` ასახავს GM/T 0004 დაიჯესტს, როგორც Norito-სერიალიზაციადი `[u8; 32]` ახალი ტიპის, რომელიც გამოიყენება ყველგან, სადაც ჰეშები გამოჩნდება მანიფესტებში ან ტელემეტრიაში. კოდი: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` წარმოადგენს 128-ბიტიან SM4 კლავიშებს და გაზიარებულია ჰოსტის სისტელებსა და მონაცემთა მოდელის მოწყობილობებს შორის. კოდი: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  ეს ტიპები მოთავსებულია არსებულ Ed25519/BLS/ML-DSA პრიმიტივებთან ერთად და ხელმისაწვდომია მონაცემთა მოდელის მომხმარებლებისთვის (Torii, SDKs, genesis tooling) `sm` ფუნქციის ჩართვის შემდეგ.

მნიშვნელოვანი თვისებები: `Identifiable`, `Registered`/`Registrable` (მშენებლის ნიმუში), `HasMetadata`, `IntoKeyValue`. კოდი: `crates/iroha_data_model/src/lib.rs`.

მოვლენები: ყველა ერთეულს აქვს მუტაციების შედეგად გამოსხივებული მოვლენები (შექმნა/წაშლა/მფლობელი შეიცვალა/მეტამონაცემები შეიცვალა და ა.შ.). კოდი: `crates/iroha_data_model/src/events/`.

---

## პარამეტრები (ჯაჭვის კონფიგურაცია)
- ოჯახები: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, პლუს `custom: BTreeMap`.
- განსხვავებების ცალკეული ნომრები: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. აგრეგატორი: `Parameters`. კოდი: `crates/iroha_data_model/src/parameter/system.rs`.

პარამეტრების დაყენება (ISI): `SetParameter(Parameter)` განაახლებს შესაბამის ველს და გამოსცემს `ConfigurationEvent::Changed`. კოდი: `crates/iroha_data_model/src/isi/transparent.rs`, შემსრულებელი `crates/iroha_core/src/smartcontracts/isi/world.rs`-ში.

---

## ინსტრუქციის სერიალიზაცია და რეგისტრაცია
- ძირითადი თვისება: `Instruction: Send + Sync + 'static` `dyn_encode()`-ით, `as_any()`, სტაბილური `id()` (ნაგულისხმევია ბეტონის ტიპის სახელზე).
- `InstructionBox`: `Box<dyn Instruction>` შეფუთვა. Clone/Eq/Ord მოქმედებს `(type_id, encoded_bytes)`-ზე, ასე რომ, თანასწორობა არის მნიშვნელობის მიხედვით.
- Norito სერდი `InstructionBox`-სთვის სერიდება როგორც `(String wire_id, Vec<u8> payload)` (უბრუნდება `type_name`-ს, თუ არ არის მავთულის ID). დესერიალიზაცია იყენებს გლობალურ `InstructionRegistry` რუკების იდენტიფიკატორებს კონსტრუქტორებისთვის. ნაგულისხმევი რეესტრი მოიცავს ყველა ჩაშენებულ ISI-ს. კოდი: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: ტიპები, სემანტიკა, შეცდომები
შესრულება ხორციელდება `Execute for <Instruction>`-ით `iroha_core::smartcontracts::isi`-ში. ქვემოთ ჩამოთვლილია საჯარო ეფექტები, წინაპირობები, ემიტირებული მოვლენები და შეცდომები.

### რეგისტრაცია / გაუქმება
ტიპები: `Register<T: Registered>` და `Unregister<T: Identifiable>`, ჯამის ტიპებით `RegisterBox`/`UnregisterBox`, რომლებიც ფარავს ბეტონის სამიზნეებს.

- რეგისტრაცია Peer: ჩანართები მსოფლიო თანატოლების კომპლექტში.
  - წინაპირობები: უკვე არ უნდა არსებობდეს.
  - ღონისძიებები: `PeerEvent::Added`.
  - შეცდომები: `Repetition(Register, PeerId)` თუ დუბლიკატია; `FindError` ძიებაზე. კოდი: `core/.../isi/world.rs`.

- დაარეგისტრირე დომენი: აშენებულია `NewDomain`-დან `owned_by = authority`-ით. დაუშვებელია: `genesis` დომენი.
  - წინაპირობები: დომენის არარსებობა; არა `genesis`.
  - ღონისძიებები: `DomainEvent::Created`.
  - შეცდომები: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. კოდი: `core/.../isi/world.rs`.- ანგარიშის რეგისტრაცია: აშენებული `NewAccount`-დან, დაუშვებელია `genesis` დომენში; `genesis` ანგარიშის რეგისტრაცია შეუძლებელია.
  - წინაპირობები: დომენი უნდა არსებობდეს; ანგარიშის არარსებობა; არა გენეზის სფეროში.
  - ღონისძიებები: `DomainEvent::Account(AccountEvent::Created)`.
  - შეცდომები: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. კოდი: `core/.../isi/domain.rs`.

- რეგისტრაცია AssetDefinition: builds from builder; კომპლექტი `owned_by = authority`.
  - წინაპირობები: განმარტება არარსებობა; დომენი არსებობს.
  - ღონისძიებები: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - შეცდომები: `Repetition(Register, AssetDefinitionId)`. კოდი: `core/.../isi/domain.rs`.

- რეგისტრაცია NFT: აშენებები მშენებლისგან; კომპლექტი `owned_by = authority`.
  - წინაპირობები: NFT არარსებობა; დომენი არსებობს.
  - ღონისძიებები: `DomainEvent::Nft(NftEvent::Created)`.
  - შეცდომები: `Repetition(Register, NftId)`. კოდი: `core/.../isi/nft.rs`.

- რეგისტრაცია როლი: აშენებს `NewRole { inner, grant_to }`-დან (პირველი მფლობელი დაფიქსირდა ანგარიშის როლების რუკების მეშვეობით), ინახავს `inner: Role`.
  - წინაპირობები: როლის არარსებობა.
  - ღონისძიებები: `RoleEvent::Created`.
  - შეცდომები: `Repetition(Register, RoleId)`. კოდი: `core/.../isi/world.rs`.

- რეგისტრაცია ტრიგერი: ინახავს ტრიგერს შესაბამის ტრიგერში, რომელიც მითითებულია ფილტრის ტიპის მიხედვით.
  - წინაპირობები: თუ ფილტრი არ არის დასამუშავებელი, `action.repeats` უნდა იყოს `Exactly(1)` (წინააღმდეგ შემთხვევაში `MathError::Overflow`). ID-ების დუბლიკატი აკრძალულია.
  - ღონისძიებები: `TriggerEvent::Created(TriggerId)`.
  - შეცდომები: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` კონვერტაციის/ვალიდაციის წარუმატებლობის შესახებ. კოდი: `core/.../isi/triggers/mod.rs`.

- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger გაუქმება: აშორებს სამიზნეს; ასხივებს წაშლის მოვლენებს. კასკადის დამატებითი მოცილება:
  - დომენის რეგისტრაციის გაუქმება: აშორებს დომენის ყველა ანგარიშს, მათ როლებს, ნებართვებს, tx-sequence მრიცხველებს, ანგარიშების ლეიბლებს და UAID-ის შეკვრას; შლის მათ აქტივებს (და თითო აქტივის მეტამონაცემებს); შლის ყველა აქტივის განმარტებას დომენში; წაშლის NFT-ებს დომენში და ნებისმიერ NFT-ს, რომელიც ეკუთვნის ამოღებულ ანგარიშებს; შლის ტრიგერებს, რომელთა ავტორიტეტის დომენი ემთხვევა. მოვლენები: `DomainEvent::Deleted`, პლუს თითო ელემენტის წაშლის მოვლენები. შეცდომები: `FindError::Domain` თუ აკლია. კოდი: `core/.../isi/world.rs`.
  - ანგარიშის გაუქმება: ამოიღებს ანგარიშის ნებართვებს, როლებს, tx-სეკვიურობის მრიცხველს, ანგარიშის ეტიკეტების შედგენას და UAID-ის შეკვრას; წაშლის ანგარიშის კუთვნილ აქტივებს (და თითო აქტივის მეტამონაცემებს); წაშლის ანგარიშის კუთვნილ NFT-ებს; შლის ტრიგერებს, რომელთა ავტორიტეტიც ეს ანგარიშია. მოვლენები: `AccountEvent::Deleted`, პლუს `NftEvent::Deleted` თითო ამოღებულ NFT-ზე. შეცდომები: `FindError::Account` თუ აკლია. კოდი: `core/.../isi/domain.rs`.
  - Unregister AssetDefinition: წაშლის ამ განმარტების ყველა აქტივს და მათ თითო აქტივზე მეტამონაცემებს. მოვლენები: `AssetDefinitionEvent::Deleted` და `AssetEvent::Deleted` თითო აქტივზე. შეცდომები: `FindError::AssetDefinition`. კოდი: `core/.../isi/domain.rs`.
  - გააუქმეთ რეგისტრაცია NFT: შლის NFT. ღონისძიებები: `NftEvent::Deleted`. შეცდომები: `FindError::Nft`. კოდი: `core/.../isi/nft.rs`.
  - როლის გაუქმება: პირველ რიგში გააუქმებს როლს ყველა ანგარიშიდან; შემდეგ ხსნის როლს. ღონისძიებები: `RoleEvent::Deleted`. შეცდომები: `FindError::Role`. კოდი: `core/.../isi/world.rs`.
  - Unregister Trigger: ხსნის ტრიგერს, თუ არსებობს; დერეგისტრაციის დუბლიკატი იძლევა `Repetition(Unregister, TriggerId)`. ღონისძიებები: `TriggerEvent::Deleted`. კოდი: `core/.../isi/triggers/mod.rs`.

### ზარაფხანა / დამწვრობა
ტიპები: `Mint<O, D: Identifiable>` და `Burn<O, D: Identifiable>`, ყუთში როგორც `MintBox`/`BurnBox`.- აქტივი (რიცხვითი) ზარაფხანა/დაწვა: არეგულირებს ნაშთებს და განმარტებას `total_quantity`.
  - წინაპირობები: `Numeric` მნიშვნელობა უნდა აკმაყოფილებდეს `AssetDefinition.spec()`; ზარაფხანა დაშვებულია `mintable`-ით:
    - `Infinitely`: ყოველთვის დასაშვებია.
    - `Once`: დასაშვებია ზუსტად ერთხელ; პირველი ზარაფხანა აბრუნებს `mintable`-ს `Not`-ზე და ასხივებს `AssetDefinitionEvent::MintabilityChanged`, პლუს დეტალურ `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`-ს აუდიტორობისთვის.
    - `Limited(n)`: საშუალებას იძლევა `n` დამატებითი ზარაფხანის ოპერაციები. ყოველი წარმატებული ზარაფხანა ამცირებს მრიცხველს; როდესაც ის მიაღწევს ნულს, განმარტება გადადის `Not`-ზე და გამოსცემს იგივე `MintabilityChanged` მოვლენებს, როგორც ზემოთ.
    - `Not`: შეცდომა `MintabilityError::MintUnmintable`.
  - სახელმწიფო ცვლილებები: ქმნის აქტივს, თუ აკლია ზარაფხანას; ამოიღებს აქტივის ჩანაწერს, თუ ნაშთი ნულის ტოლია დამწვრობისას.
  - ღონისძიებები: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (როდესაც `Once` ან `Limited(n)` ამოწურავს თავის შემწეობას).
  - შეცდომები: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. კოდი: `core/.../isi/asset.rs`.

- ტრიგერის გამეორებები ზარაფხანა/დაწვა: ცვლილებები `action.repeats` ითვლება ტრიგერისთვის.
  - წინაპირობები: პიტნაზე, ფილტრი უნდა იყოს დასამუშავებელი; არითმეტიკა არ უნდა გადადიოდეს/დაქვეითდეს.
  - ღონისძიებები: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - შეცდომები: `MathError::Overflow` არასწორი ზარაფხანაზე; `FindError::Trigger` თუ აკლია. კოდი: `core/.../isi/triggers/mod.rs`.

### ტრანსფერი
ტიპები: `Transfer<S: Identifiable, O, D: Identifiable>`, ყუთში როგორც `TransferBox`.

- აქტივი (რიცხვითი): გამოაკელი წყარო `AssetId`, დაამატეთ დანიშნულების ადგილი `AssetId` (იგივე განმარტება, განსხვავებული ანგარიში). ნულოვანი წყაროს აქტივის წაშლა.
  - წინაპირობები: წყარო აქტივი არსებობს; მნიშვნელობა აკმაყოფილებს `spec`.
  - ღონისძიებები: `AssetEvent::Removed` (წყარო), `AssetEvent::Added` (დანიშნულება).
  - შეცდომები: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. კოდი: `core/.../isi/asset.rs`.

- დომენის მფლობელობა: ცვლის `Domain.owned_by` დანიშნულების ანგარიშს.
  - წინაპირობები: ორივე ანგარიში არსებობს; დომენი არსებობს.
  - ღონისძიებები: `DomainEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/Domain`. კოდი: `core/.../isi/domain.rs`.

- AssetDefinition მფლობელობა: ცვლის `AssetDefinition.owned_by` დანიშნულების ანგარიშს.
  - წინაპირობები: ორივე ანგარიში არსებობს; განმარტება არსებობს; წყარო ამჟამად უნდა ფლობდეს მას.
  - ღონისძიებები: `AssetDefinitionEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/AssetDefinition`. კოდი: `core/.../isi/account.rs`.

- NFT მფლობელობა: ცვლის `Nft.owned_by` დანიშნულების ანგარიშს.
  - წინაპირობები: ორივე ანგარიში არსებობს; NFT არსებობს; წყარო ამჟამად უნდა ფლობდეს მას.
  - ღონისძიებები: `NftEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/Nft`, `InvariantViolation`, თუ ​​წყარო არ ფლობს NFT-ს. კოდი: `core/.../isi/nft.rs`.

### მეტამონაცემები: დააყენეთ/ამოშალეთ გასაღები-მნიშვნელობა
ტიპები: `SetKeyValue<T>` და `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`-ით. მოწოდებულია ყუთში ჩასმული ნომრები.

- კომპლექტი: აყენებს ან ცვლის `Metadata[key] = Json(value)`.
- ამოღება: ამოიღებს გასაღებს; შეცდომა თუ აკლია.
- მოვლენები: `<Target>Event::MetadataInserted` / `MetadataRemoved` ძველი/ახალი მნიშვნელობებით.
- შეცდომები: `FindError::<Target>` თუ სამიზნე არ არსებობს; `FindError::MetadataKey` ამოღებულ გასაღებზე. კოდი: `crates/iroha_data_model/src/isi/transparent.rs` და აღმასრულებელი impls თითო სამიზნე.### ნებართვები და როლები: გაცემა / გაუქმება
ტიპები: `Grant<O, D>` და `Revoke<O, D>`, ყუთებში `Permission`/`Role`-დან/`Account`-მდე და `Account`-მდე და `Account`-მდე და Norito-მდე.

- ანგარიშის ნებართვის მინიჭება: ამატებს `Permission`, თუ ​​უკვე არ არის თანდაყოლილი. ღონისძიებები: `AccountEvent::PermissionAdded`. შეცდომები: `Repetition(Grant, Permission)` თუ დუბლიკატია. კოდი: `core/.../isi/account.rs`.
- გააუქმეთ ნებართვა ანგარიშიდან: ამოღებულია, თუ არსებობს. ღონისძიებები: `AccountEvent::PermissionRemoved`. შეცდომები: `FindError::Permission` თუ არ არსებობს. კოდი: `core/.../isi/account.rs`.
- როლის მინიჭება ანგარიშზე: არყოფნის შემთხვევაში ათავსებს `(account, role)` რუკებს. ღონისძიებები: `AccountEvent::RoleGranted`. შეცდომები: `Repetition(Grant, RoleId)`. კოდი: `core/.../isi/account.rs`.
- როლის გაუქმება ანგარიშიდან: აშორებს რუკებს, თუ არსებობს. ღონისძიებები: `AccountEvent::RoleRevoked`. შეცდომები: `FindError::Role` თუ არ არსებობს. კოდი: `core/.../isi/account.rs`.
- როლის ნებართვის მინიჭება: აღადგენს როლს დამატებული ნებართვით. ღონისძიებები: `RoleEvent::PermissionAdded`. შეცდომები: `Repetition(Grant, Permission)`. კოდი: `core/.../isi/world.rs`.
- Role-დან ნებართვის გაუქმება: აღადგენს როლს ამ ნებართვის გარეშე. ღონისძიებები: `RoleEvent::PermissionRemoved`. შეცდომები: `FindError::Permission` თუ არ არსებობს. კოდი: `core/.../isi/world.rs`.

### ტრიგერები: შეასრულეთ
ტიპი: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- ქცევა: აყენებს `ExecuteTriggerEvent { trigger_id, authority, args }`-ს ტრიგერის ქვესისტემისთვის. ხელით შესრულება ნებადართულია მხოლოდ ზარის გამომწვევებისთვის (`ExecuteTrigger` ფილტრი); ფილტრი უნდა ემთხვეოდეს და აბონენტი უნდა იყოს ტრიგერის მოქმედების ავტორიტეტი ან დაიჭიროს `CanExecuteTrigger` ამ უფლებამოსილებისთვის. როდესაც მომხმარებლის მიერ მოწოდებული შემსრულებელი აქტიურია, ტრიგერის შესრულება დამოწმებულია გაშვების დროის შემსრულებელის მიერ და მოიხმარს ტრანზაქციის შემსრულებლის საწვავის ბიუჯეტს (ბაზა `executor.fuel` პლუს არჩევითი მეტამონაცემები `additional_fuel`).
- შეცდომები: `FindError::Trigger` თუ არ არის რეგისტრირებული; `InvariantViolation` თუ დაურეკავს არაუფლებამოსილს. კოდი: `core/.../isi/triggers/mod.rs` (და ტესტები `core/.../smartcontracts/isi/mod.rs`-ში).

### განახლება და შესვლა
- `Upgrade { executor }`: ახდენს შემსრულებლის მიგრაციას მოწოდებული `Executor` ბაიტიკოდის გამოყენებით, განაახლებს შემსრულებელს და მის მონაცემთა მოდელს, გამოსცემს `ExecutorEvent::Upgraded`. შეცდომები: შეფუთულია როგორც `InvalidParameterError::SmartContract` მიგრაციის წარუმატებლობისას. კოდი: `core/.../isi/world.rs`.
- `Log { level, msg }`: გამოსცემს კვანძის ჟურნალს მოცემულ დონეზე; სახელმწიფო არ იცვლება. კოდი: `core/.../isi/world.rs`.

### შეცდომის მოდელი
საერთო კონვერტი: `InstructionExecutionError` ვარიანტებით შეფასების შეცდომებისთვის, შეკითხვის წარუმატებლობები, კონვერტაციები, ერთეული ვერ მოიძებნა, გამეორება, ფორმირება, მათემატიკა, არასწორი პარამეტრი და უცვლელი დარღვევა. ჩამოთვლები და დამხმარეები არის `crates/iroha_data_model/src/isi/mod.rs`-ში `pub mod error`-ში.

---## ტრანზაქციები და შესრულებადი
- `Executable`: ან `Instructions(ConstVec<InstructionBox>)` ან `Ivm(IvmBytecode)`; ბაიტეკოდი სერიდება როგორც base64. კოდი: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: აშენებს, ხელს აწერს და აფუჭებს შესრულებადს მეტამონაცემებით, `chain_id`, `authority`, Norito და არჩევითი I08. `nonce`. კოდი: `crates/iroha_data_model/src/transaction/`.
- გაშვების დროს, `iroha_core` ახორციელებს `InstructionBox` პარტიებს `Execute for InstructionBox`-ის მეშვეობით, ჩამორთმევით შესაბამის `*Box`-ზე ან კონკრეტულ ინსტრუქციაზე. კოდი: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- შესრულების ვადის შემსრულებელი ვალიდაციის ბიუჯეტი (მომხმარებლის მიერ მოწოდებული შემსრულებელი): ბაზის `executor.fuel` პარამეტრებიდან პლუს არჩევითი ტრანზაქციის მეტამონაცემები `additional_fuel` (`u64`), გაზიარებული ინსტრუქციების/ტრიგერების ვალიდაციაში ტრანზაქციის ფარგლებში.

---

## ინვარიანტები და შენიშვნები (ტესტებიდან და მცველებიდან)
- Genesis-ის დაცვა: ვერ დაარეგისტრირებს `genesis` დომენი ან ანგარიშები `genesis` დომენში; `genesis` ანგარიშის რეგისტრაცია შეუძლებელია. კოდი/ტესტები: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- რიცხვითი აქტივები უნდა აკმაყოფილებდეს მათ `NumericSpec` ზარაფხანაზე/გადაცემაზე/დაწვაზე; სპეციფიკაციების შეუსაბამობა იძლევა `TypeError::AssetNumericSpec`.
- ზარაფხანადობა: `Once` საშუალებას იძლევა ერთი ზარაფხანა და შემდეგ გადატრიალდება `Not`-ზე; `Limited(n)` საშუალებას აძლევს ზუსტად `n` მოჭრას `Not`-ზე გადაბრუნებამდე. `Infinitely`-ზე მოჭრის აკრძალვის მცდელობები იწვევს `MintabilityError::ForbidMintOnMintable`, ხოლო `Limited(0)` კონფიგურაცია იძლევა `MintabilityError::InvalidMintabilityTokens`.
- მეტამონაცემების ოპერაციები საკვანძო-ზუსტია; არარსებული გასაღების ამოღება შეცდომაა.
- ტრიგერის ფილტრები შეიძლება იყოს არასამუშაო; მაშინ `Register<Trigger>` იძლევა მხოლოდ `Exactly(1)` გამეორების საშუალებას.
- ტრიგერის მეტამონაცემების გასაღები `__enabled` (bool) კარიბჭის შესრულება; ნაგულისხმევი ნაგულისხმევი არ არის ჩართული და გამორთული ტრიგერები გამოტოვებულია მონაცემთა/დროის/გამოძახების ბილიკებზე.
- დეტერმინიზმი: ყველა არითმეტიკა იყენებს შემოწმებულ ოპერაციებს; under/overflow აბრუნებს აკრეფილ მათემატიკურ შეცდომებს; ნულოვანი ნაშთები ამცირებს აქტივების ჩანაწერებს (დამალული მდგომარეობის გარეშე).

---## პრაქტიკული მაგალითები
- მოჭრა და გადატანა:
  - `Mint::asset_numeric(10, asset_id)` → ამატებს 10-ს, თუ ეს ნებადართულია სპეციფიკაციებით/დამზადებით; მოვლენები: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → მოძრაობს 5; მოხსნის/დამატების ღონისძიებები.
- მეტამონაცემების განახლებები:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; ამოღება `RemoveKeyValue::account(...)`-ით.
- როლების/ნებართვების მართვა:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` და მათი `Revoke` კოლეგები.
- ტრიგერის სასიცოცხლო ციკლი:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` ფილტრით ნაგულისხმევი დამუშავების შემოწმებით; `ExecuteTrigger::new(id).with_args(&args)` უნდა შეესაბამებოდეს კონფიგურირებულ ავტორიტეტს.
  - ტრიგერები შეიძლება გამორთოთ მეტამონაცემების კლავიშის `__enabled`-ზე `false`-ზე დაყენებით (ნაგულისხმევი ნაგულისხმევი არ არის ჩართული); გადართვა `SetKeyValue::trigger` ან IVM `set_trigger_enabled` syscall-ის მეშვეობით.
  - ტრიგერების საცავი შეკეთებულია ჩატვირთვისას: დუბლიკატი ID, შეუსაბამო ID და გამოტოვებული ბაიტეკოდის მითითების ტრიგერები იშლება; ბაიტიკოდის მითითების რაოდენობა ხელახლა გამოითვლება.
  - თუ ტრიგერის IVM ბაიტიკოდი არ არის შესრულების დროს, ტრიგერი ამოღებულია და შესრულება განიხილება, როგორც არა-ოპტიმალური მარცხის შედეგი.
  - ამოწურული ტრიგერები დაუყოვნებლივ მოიხსნება; თუ ამოწურული ჩანაწერი შეგხვდება შესრულების დროს, ის იჭრება და განიხილება როგორც დაკარგული.
- პარამეტრის განახლება:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` განაახლებს და გამოსცემს `ConfigurationEvent::Changed`.

---

## მიკვლევადობა (არჩეული წყაროები)
 - მონაცემთა მოდელის ბირთვი: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI განმარტებები და რეესტრი: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI შესრულება: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - ღონისძიებები: `crates/iroha_data_model/src/events/**`.
 - ტრანზაქციები: `crates/iroha_data_model/src/transaction/**`.

თუ გსურთ, რომ ეს სპეციფიკა გაფართოვდეს API/ქცევის ცხრილად ან ჯვარედინი ბმული იყოს ყველა კონკრეტულ მოვლენასთან/შეცდომასთან, თქვით სიტყვა და მე გავაგრძელებ.