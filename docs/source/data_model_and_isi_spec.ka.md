<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8d13f6d206f60d31217ed093a5bbedd7946d27b644f9b3321a577cc6065a901
source_last_modified: "2026-03-30T18:22:55.965549+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Iroha v2 მონაცემთა მოდელი და ISI — იმპლემენტაციისგან მიღებული სპეციფიკაცია

ეს სპეციფიკაცია შემუშავებულია `iroha_data_model`-სა და `iroha_core`-ში მიმდინარე განხორციელებისგან, რათა ხელი შეუწყოს დიზაინის მიმოხილვას. ბილიკები უკანა ხაზში მიუთითებს ავტორიტეტულ კოდზე.

## სფერო
- განსაზღვრავს კანონიკურ ერთეულებს (დომენები, ანგარიშები, აქტივები, NFT-ები, როლები, ნებართვები, თანატოლები, ტრიგერები) და მათ იდენტიფიკატორებს.
- აღწერს მდგომარეობის შეცვლის ინსტრუქციებს (ISI): ტიპებს, პარამეტრებს, წინაპირობებს, მდგომარეობის გადასვლებს, ემიტირებული მოვლენებს და შეცდომის პირობებს.
- აჯამებს პარამეტრების მართვას, ტრანზაქციებს და ინსტრუქციების სერიალიზაციას.

დეტერმინიზმი: ყველა ინსტრუქციის სემანტიკა არის სუფთა მდგომარეობის გადასვლები, აპარატურაზე დამოკიდებული ქცევის გარეშე. სერიალიზაცია იყენებს Norito; VM ბაიტიკოდი იყენებს IVM-ს და დამოწმებულია ჰოსტის მხრიდან ჯაჭვზე შესრულებამდე.

---

## პირები და იდენტიფიკატორები
ID-ებს აქვთ სტაბილური სიმებიანი ფორმები `Display`/`FromStr` ორმხრივი მოგზაურობით. სახელის წესები კრძალავს უფსკრული და დაცული `@ # $` სიმბოლოები.- `Name` — დადასტურებული ტექსტური იდენტიფიკატორი. წესები: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. დომენი: `{ id, logo, metadata, owned_by }`. მშენებლები: `NewDomain`. კოდი: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — კანონიკური მისამართები იწარმოება `AccountAddress`-ის საშუალებით, რადგან I105 და Torii ახდენს შეყვანის ნორმალიზებას `AccountAddress::parse_encoded`-ის მეშვეობით. მკაცრი გაშვების ანალიზები იღებს მხოლოდ კანონიკურ I105-ს. ჯაჭვური ანგარიშის მეტსახელები იყენებენ `name@domain.dataspace` ან `name@dataspace` და გადაწყვეტენ კანონიკურ `AccountId` მნიშვნელობებს; ისინი არ მიიღება მკაცრი `AccountId` პარსერების მიერ. ანგარიში: `{ id, metadata }`. კოდი: `crates/iroha_data_model/src/account.rs`.- ანგარიშის დაშვების პოლიტიკა — დომენები აკონტროლებენ ანგარიშის იმპლიციტურ შექმნას Norito-JSON `AccountAdmissionPolicy` მეტამონაცემების გასაღების `iroha:account_admission_policy`-ის შენახვით. როდესაც გასაღები არ არის, ჯაჭვის დონის მორგებული პარამეტრი `iroha:default_account_admission_policy` უზრუნველყოფს ნაგულისხმევს; როდესაც ეს ასევე არ არის, მყარი ნაგულისხმევი არის `ImplicitReceive` (პირველი გამოშვება). პოლიტიკის თეგები `mode` (`ExplicitOnly` ან `ImplicitReceive`) პლუს არჩევითი თითო ტრანზაქცია (ნაგულისხმევი `16`) და თითო ბლოკის შექმნის ქუდები, არჩევითი `implicit_creation_fee` ანგარიში `min_initial_amounts` აქტივის განმარტებაზე და არასავალდებულო `default_role_on_create` (გაცემულია `AccountCreated`-ის შემდეგ, უარყოფილია `DefaultRoleError`-ით, თუ აკლია). Genesis-ს არ შეუძლია მონაწილეობა მიიღოს; გათიშული/არასწორი პოლიტიკა უარყოფს ქვითრის სტილის ინსტრუქციებს უცნობი ანგარიშებისთვის `InstructionExecutionError::AccountAdmission`-ით. იმპლიციტური ანგარიშების შტამპი მეტამონაცემების `iroha:created_via="implicit"` `AccountCreated`-მდე; ნაგულისხმევი როლები ასხივებენ შემდგომ `AccountRoleGranted`-ს, ხოლო შემსრულებლის მფლობელის საბაზისო წესები საშუალებას აძლევს ახალ ანგარიშს დახარჯოს საკუთარი აქტივები/NFT დამატებითი როლების გარეშე. კოდი: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.- `AssetDefinitionId` — კანონიკური უპრეფიქსირებული Base58 მისამართი კანონიკური აქტივების განსაზღვრის ბაიტებზე. ეს არის საჯარო აქტივის ID. განმარტება: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. `alias` ლიტერალები უნდა იყოს `<name>#<domain>.<dataspace>` ან `<name>#<dataspace>`, `<name>` უდრის აქტივის განმარტების სახელს და ისინი გადაწყვეტენ მხოლოდ კანონიკურ Base58 აქტივის ID-ს. კოდი: `crates/iroha_data_model/src/asset/definition.rs`.
  - მეტამონაცემები იჯარის მეტამონაცემები შენარჩუნებულია შენახული აქტივების განმარტების რიგისგან დამოუკიდებლად. Core/Torii მატერიალიზდება `alias` სავალდებულო ჩანაწერიდან, როდესაც განმარტებები იკითხება.
  - Torii აქტივების განსაზღვრის პასუხები ავლენს `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`-ს, სადაც `status` არის `permanent`, `leased_active`, Norito, ან Norito.
  - ალიასის სელექტორები წყვეტენ ბლოკის შექმნის უახლესი დროის წინააღმდეგ. `grace_until_ms`-ის შემდეგ, ალიასის სელექტორები წყვეტენ გადაჭრას მაშინაც კი, თუ ფონის წმენდას ჯერ არ მოუშორებია მოძველებული შეკვრა; პირდაპირი განმარტებით წაკითხულებმა შეიძლება კვლავ შეატყობინოს მოძველებული სავალდებულო, როგორც `expired_pending_cleanup`.
- `AssetId`: საჯარო აქტივის იდენტიფიკატორი კანონიკური შიშველი Base58 ფორმით. აქტივების მეტსახელები, როგორიცაა `name#dataspace` ან `name#domain.dataspace`, გადაწყვეტს `AssetId`-ს. შიდა ბუღალტრული აღრიცხვა შეიძლება დამატებით გამოავლინოს გაყოფილი `asset + account + optional dataspace` ველები, სადაც საჭიროა, მაგრამ ეს კომპოზიციური ფორმა არ არის საჯარო `AssetId`.
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. კოდი: `crates/iroha_data_model/src/nft.rs`.- `RoleId` — `name`. როლი: `{ id, permissions: BTreeSet<Permission> }` მშენებელთან `NewRole { inner: Role, grant_to }`. კოდი: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. კოდი: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — თანატოლების ვინაობა (საჯარო გასაღები) და მისამართი. კოდი: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. ტრიგერი: `{ id, action }`. მოქმედება: `{ executable, repeats, authority, filter, metadata }`. კოდი: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` მონიშნული ჩანართით/ამოღება. კოდი: `crates/iroha_data_model/src/metadata.rs`.
- გამოწერის ნიმუში (აპლიკაციის ფენა): გეგმები არის `AssetDefinition` ჩანაწერები `subscription_plan` მეტამონაცემებით; გამოწერები არის `Nft` ჩანაწერები `subscription` მეტამონაცემებით; ბილინგი შესრულებულია დროითი ტრიგერით, რომელიც მიუთითებს გამოწერის NFT-ებზე. იხილეთ `docs/source/subscriptions_api.md` და `crates/iroha_data_model/src/subscription.rs`.
- **კრიპტოგრაფიული პრიმიტივები** (მახასიათებელი `sm`):
  - `Sm2PublicKey` / `Sm2Signature` ასახავს კანონიკურ SEC1 წერტილს + ფიქსირებული სიგანის `r∥s` კოდირებას SM2-ისთვის. კონსტრუქტორები ახორციელებენ მრუდის წევრობას და განმასხვავებელ ID სემანტიკას (`DEFAULT_DISTID`), ხოლო დადასტურება უარყოფს არასწორი ან მაღალი დიაპაზონის სკალერებს. კოდი: `crates/iroha_crypto/src/sm.rs` და `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` ასახავს GM/T 0004 დაიჯესტს, როგორც Norito-სერიალიზაციადი `[u8; 32]` ახალი ტიპის, რომელიც გამოიყენება ყველგან, სადაც ჰეშები გამოჩნდება მანიფესტებში ან ტელემეტრიაში. კოდი: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` წარმოადგენს 128-ბიტიან SM4 კლავიშებს და გაზიარებულია ჰოსტის სისტელებსა და მონაცემთა მოდელის მოწყობილობებს შორის. კოდი: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  ეს ტიპები მოთავსებულია არსებულ Ed25519/BLS/ML-DSA პრიმიტივებთან ერთად და ხელმისაწვდომია მონაცემთა მოდელის მომხმარებლებისთვის (Torii, SDKs, genesis tooling) `sm` ფუნქციის ჩართვის შემდეგ.
- მონაცემთა სივრციდან მიღებული ურთიერთობის მაღაზიები (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, ზოლის რელეს გადაუდებელი გადარიცხვის რეესტრი) და მონაცემთა სივრცის სამიზნეების ნებართვები (I0120X100X1) იჭრება `State::set_nexus(...)`-ზე, როდესაც მონაცემთა სივრცეები ქრება აქტიური `dataspace_catalog`-დან, რაც ხელს უშლის მონაცემთა სივრცის ძველ ცნობებს გაშვების კატალოგის განახლების შემდეგ. ზოლის დიაპაზონის DA/რელეის ქეშები (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) ასევე იჭრება, როდესაც ზოლი ამოიწურება ან ხელახლა მინიჭება მონაცემთა სივრცის სხვა სივრცეში შეუძლებელია, ასე რომ, lane-local state. Space Directory ISI-ები (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) ასევე ამოწმებს `dataspace` აქტიურ კატალოგს და უარყოფს უცნობ ID-ებს `InvalidParameter`-ით.

მნიშვნელოვანი თვისებები: `Identifiable`, `Registered`/`Registrable` (მშენებლის ნიმუში), `HasMetadata`, `IntoKeyValue`. კოდი: `crates/iroha_data_model/src/lib.rs`.

მოვლენები: ყველა ერთეულს აქვს მუტაციების შედეგად გამოსხივებული მოვლენები (შექმნა/წაშლა/მფლობელი შეიცვალა/მეტამონაცემები შეიცვალა და ა.შ.). კოდი: `crates/iroha_data_model/src/events/`.

---## პარამეტრები (ჯაჭვის კონფიგურაცია)
- ოჯახები: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, პლუს `custom: BTreeMap`.
- განსხვავებების ცალკეული ნომრები: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. აგრეგატორი: `Parameters`. კოდი: `crates/iroha_data_model/src/parameter/system.rs`.

პარამეტრების დაყენება (ISI): `SetParameter(Parameter)` განაახლებს შესაბამის ველს და გამოსცემს `ConfigurationEvent::Changed`. კოდი: `crates/iroha_data_model/src/isi/transparent.rs`, შემსრულებელი `crates/iroha_core/src/smartcontracts/isi/world.rs`-ში.

---

## ინსტრუქციის სერიალიზაცია და რეგისტრაცია
- ძირითადი თვისება: `Instruction: Send + Sync + 'static` `dyn_encode()`-ით, `as_any()`, სტაბილური `id()` (ნაგულისხმევია ბეტონის ტიპის სახელზე).
- `InstructionBox`: `Box<dyn Instruction>` შეფუთვა. Clone/Eq/Ord მოქმედებს `(type_id, encoded_bytes)`-ზე, ამიტომ თანასწორობა არის მნიშვნელობის მიხედვით.
- Norito სერდი `InstructionBox`-ისთვის სერიულირდება როგორც `(String wire_id, Vec<u8> payload)` (უბრუნდება `type_name`-ს, თუ მავთულის ID არ არის). დესერიალიზაცია იყენებს გლობალურ `InstructionRegistry` რუკების იდენტიფიკატორებს კონსტრუქტორებისთვის. ნაგულისხმევი რეესტრი მოიცავს ყველა ჩაშენებულ ISI-ს. კოდი: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: ტიპები, სემანტიკა, შეცდომები
შესრულება ხორციელდება `Execute for <Instruction>`-ის მეშვეობით `iroha_core::smartcontracts::isi`-ში. ქვემოთ ჩამოთვლილია საჯარო ეფექტები, წინაპირობები, ემიტირებული მოვლენები და შეცდომები.

### რეგისტრაცია / გაუქმება
ტიპები: `Register<T: Registered>` და `Unregister<T: Identifiable>`, ჯამური ტიპებით `RegisterBox`/`UnregisterBox`, რომლებიც ფარავს ბეტონის სამიზნეებს.- რეგისტრაცია Peer: ჩანართები მსოფლიო თანატოლების კომპლექტში.
  - წინაპირობები: უკვე არ უნდა არსებობდეს.
  - ღონისძიებები: `PeerEvent::Added`.
  - შეცდომები: `Repetition(Register, PeerId)` თუ დუბლიკატია; `FindError` ძიებაზე. კოდი: `core/.../isi/world.rs`.

- დაარეგისტრირე დომენი: აშენებულია `NewDomain`-დან `owned_by = authority`-ით. დაუშვებელია: `genesis` დომენი.
  - წინაპირობები: დომენის არარსებობა; არა `genesis`.
  - ღონისძიებები: `DomainEvent::Created`.
  - შეცდომები: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. კოდი: `core/.../isi/world.rs`.

- ანგარიშის რეგისტრაცია: აშენებული `NewAccount`-დან, დაუშვებელია `genesis` დომენში; `genesis` ანგარიშის რეგისტრაცია შეუძლებელია.
  - წინაპირობები: დომენი უნდა არსებობდეს; ანგარიშის არარსებობა; არა გენეზის სფეროში.
  - ღონისძიებები: `DomainEvent::Account(AccountEvent::Created)`.
  - შეცდომები: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. კოდი: `core/.../isi/domain.rs`.

- რეგისტრაცია AssetDefinition: builds from builder; კომპლექტი `owned_by = authority`.
  - წინაპირობები: განმარტება არარსებობა; დომენი არსებობს; `name` საჭიროა, არ უნდა იყოს ცარიელი მოჭრის შემდეგ და არ უნდა შეიცავდეს `#`/`@`.
  - ღონისძიებები: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - შეცდომები: `Repetition(Register, AssetDefinitionId)`. კოდი: `core/.../isi/domain.rs`.

- რეგისტრაცია NFT: აშენებები მშენებლისგან; კომპლექტი `owned_by = authority`.
  - წინაპირობები: NFT არარსებობა; დომენი არსებობს.
  - ღონისძიებები: `DomainEvent::Nft(NftEvent::Created)`.
  - შეცდომები: `Repetition(Register, NftId)`. კოდი: `core/.../isi/nft.rs`.- რეგისტრაცია როლი: აშენებს `NewRole { inner, grant_to }`-დან (პირველი მფლობელი ჩაწერილია ანგარიშის როლების რუკების მეშვეობით), ინახავს `inner: Role`.
  - წინაპირობები: როლის არარსებობა.
  - ღონისძიებები: `RoleEvent::Created`.
  - შეცდომები: `Repetition(Register, RoleId)`. კოდი: `core/.../isi/world.rs`.

- რეგისტრაცია ტრიგერი: ინახავს ტრიგერს შესაბამის ტრიგერში, რომელიც მითითებულია ფილტრის ტიპის მიხედვით.
  - წინაპირობები: თუ ფილტრი არ არის დასამუშავებელი, `action.repeats` უნდა იყოს `Exactly(1)` (წინააღმდეგ შემთხვევაში `MathError::Overflow`). ID-ების დუბლიკატი აკრძალულია.
  - ღონისძიებები: `TriggerEvent::Created(TriggerId)`.
  - შეცდომები: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` კონვერტაციის/ვალიდაციის წარუმატებლობის შესახებ. კოდი: `core/.../isi/triggers/mod.rs`.- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger გაუქმება: აშორებს სამიზნეს; ასხივებს წაშლის მოვლენებს. კასკადის დამატებითი მოცილება:- დომენის გაუქმება: აშორებს დომენის ერთეულს პლუს მის სელექტორს/მოწონების პოლიტიკის მდგომარეობას; შლის აქტივების დეფინიციებს დომენში (და კონფიდენციალურ `zk_assets` გვერდით მდგომარეობას, რომელიც ჩართულია ამ განმარტებებით), ამ განმარტებების აქტივებს (და თითო აქტივის მეტამონაცემებს), NFT-ებს დომენში და ანგარიშის ალიასის პროგნოზებს, რომლებიც დაფუძნებულია ამოღებულ დომენში. ის ასევე წყვეტს ანგარიშის/როლური ფარგლების ნებართვის ჩანაწერებს, რომლებიც მიუთითებს წაშლილ დომენზე ან მასთან ერთად წაშლილ რესურსებზე (დომენის ნებართვები, აქტივების განსაზღვრის/აქტივების ნებართვები წაშლილი განმარტებებისთვის და NFT ნებართვები წაშლილი NFT ID-ებისთვის). დომენის წაშლა არ წაშლის ან არ წერს გლობალურ `AccountId`-ს, მის tx-sequence/UAID მდგომარეობას, უცხოურ აქტივს ან NFT მფლობელობას, ტრიგერების უფლებამოსილებას ან სხვა გარე აუდიტის/კონფიგურაციის მითითებებს, რომლებიც მიუთითებს გადარჩენილ ანგარიშზე. დამცავი რელსები: უარყოფს, როდესაც დომენში ნებისმიერი აქტივის განმარტება კვლავ მითითებულია რეპო-შეთანხმებით, ანგარიშსწორების წიგნი, საჯარო ზოლის ჯილდო/პრეტენზია, ხაზგარეშე შემწეობა/გადაცემა, ანგარიშსწორების რეპოს ნაგულისხმევი (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), მმართველობა-configuribility/citizea-virreligity აქტივების განსაზღვრის მითითებები, Oracle-economics-ის კონფიგურირებული ჯილდო/სლეში/სადავო ობლიგაციების აქტივების განსაზღვრის მითითებები, ან Nexus საკომისიო/ფსონის აქტივების განსაზღვრის მითითებები (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). მოვლენები: `DomainEvent::Deleted`, პლუს თითო ელემენტის წაშლის მოვლენები წაშლილი დომენის რესურსისთვისces. შეცდომები: `FindError::Domain` თუ აკლია; `InvariantViolation` შენარჩუნებული აქტივების განსაზღვრების მითითების კონფლიქტებზე. კოდი: `core/.../isi/world.rs`.- ანგარიშის გაუქმება: ამოიღებს ანგარიშის ნებართვებს, როლებს, tx-სეკვიურობის მრიცხველს, ანგარიშის ეტიკეტების შედგენას და UAID-ის შეკვრას; წაშლის ანგარიშის კუთვნილ აქტივებს (და თითო აქტივის მეტამონაცემებს); წაშლის ანგარიშის კუთვნილ NFT-ებს; შლის ტრიგერებს, რომელთა ავტორიტეტიც არის ეს ანგარიში; prunes ანგარიში-/როლური მასშტაბის ნებართვის ჩანაწერები, რომლებიც მიუთითებს წაშლილ ანგარიშზე, ანგარიშის/როლური ფარგლების NFT-სამიზნე ნებართვები წაშლილი საკუთრებაში არსებული NFT ID-ებისთვის და ანგარიშის/როლის მასშტაბის ტრიგერების სამიზნე ნებართვები წაშლილი ტრიგერებისთვის. დამცავი რელსები: უარყოფს, თუ ანგარიში კვლავ ფლობს დომენს, აქტივის განსაზღვრა, SoraFS პროვაიდერის სავალდებულო, აქტიური მოქალაქეობის ჩანაწერი, საჯარო ზოლის დადება/დაჯილდოების მდგომარეობა (დაჯილდოების მოთხოვნის გასაღებების ჩათვლით, სადაც ანგარიში გამოჩნდება, როგორც მოსარჩელე ან ჯილდო-აქტივის მფლობელი), აქტიური oracle-ის პროვაიდერის მდგომარეობა (მათ შორის, ტვიტერის მიმწოდებელი-binistory, ტვიტერის მიმწოდებელი პროვაიდერი). ჩანაწერები, ან Oracle-economics-ის კონფიგურირებული ჯილდო/დაჭრილი ანგარიშის მითითებები), აქტიური Nexus საკომისიო/საქონლის ანგარიშის მითითებები (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; გაანალიზებული, როგორც ხელახლა იდენტიფიცირებული დომენის ცალმხრივი და კანონიკური ანგარიშების გარეშე. ლიტერალები), აქტიური რეპო-შეთანხმების მდგომარეობა, აქტიური ანგარიშსწორების მდგომარეობა, აქტიური ხაზგარეშე დაშვება/გადაცემა ან ოფლაინ ვერდიქტის გაუქმების მდგომარეობა, აქტიური ხაზგარეშე ესკრუ-ანგარიშის კონფიგურაციის მითითებები აქტიური აქტივების განსაზღვრებისთვის (`settlement.offline.escrow_accounts`), აქტიური მმართველობის მდგომარეობა (წინადადება/ეტაპის დამტკიცებაals/locks/slashes/საბჭო/პარლამენტის სიები, წინადადებების პარლამენტის კადრები, წარდგენის დროში განახლებული წინადადებების ჩანაწერები, მმართველობით კონფიგურირებული ესქრო/სლეშ-მიმღები/ვირუსული ფონდის ანგარიშების მითითებები, მმართველობა SoraFS ტელემეტრია SoraFS ტელემეტრიის წარმდგენის მითითებები via / Norito `gov.sorafs_telemetry.per_provider_submitters`, ან მმართველობით კონფიგურირებული SoraFS პროვაიდერის მფლობელის მითითებები `gov.sorafs_provider_owners`-ის საშუალებით), კონფიგურირებული კონტენტი აქვეყნებს ნებადართული სიის ანგარიშის მითითებებს (`content.publish_allow_accounts`), აქტიური სოციალური escrow გამომგზავნის შტატი, აქტიური კონტენტი, აქტიური სოციალური მესაკუთრის სტატუსი, DA ხაზის სარელეო გადაუდებელი ვალიდატორის გადაფარვის მდგომარეობა, ან აქტიური SoraFS პინი-რეესტრის გამცემის/შემკვრელი ჩანაწერები (პინის მანიფესტები, მანიფესტის მეტსახელები, რეპლიკაციის ბრძანებები). მოვლენები: `AccountEvent::Deleted`, პლუს `NftEvent::Deleted` თითო ამოღებულ NFT-ზე. შეცდომები: `FindError::Account` თუ აკლია; `InvariantViolation` მესაკუთრე ობლებს. კოდი: `core/.../isi/domain.rs`.- Unregister AssetDefinition: წაშლის ამ განმარტების ყველა აქტივს და მათ თითო აქტივზე მეტამონაცემებს და წაშლის კონფიდენციალურ `zk_assets` გვერდით მდგომარეობას, რომელიც დამაგრებულია ამ განსაზღვრებით; ასევე წყვეტს შესატყვისი `settlement.offline.escrow_accounts` ჩანაწერის და ანგარიშის/როლის ფარგლების ნებართვის ჩანაწერებს, რომლებიც მიუთითებს ამოღებულ აქტივის განმარტებაზე ან მის აქტივების მაგალითებზე. დამცავი რელსები: უარყოფს, როდესაც განმარტება ჯერ კიდევ მითითებულია რეპო-შეთანხმებით, ანგარიშსწორების წიგნი, საჯარო ზოლის ჯილდო/პრეტენზია, ხაზგარეშე შემწეობა/გადაცემის მდგომარეობა, ანგარიშსწორების რეპოს ნაგულისხმევი (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), მმართველობით კონფიგურირებული ხმის მიცემა/ligibility/citizenal. აქტივების განსაზღვრის მითითებები, Oracle-economics-ის კონფიგურირებული ჯილდო/სლეში/სადავო ობლიგაციების აქტივების განსაზღვრის ცნობები, ან Nexus საკომისიო/ფსონის აქტივების განსაზღვრის ცნობები (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). მოვლენები: `AssetDefinitionEvent::Deleted` და `AssetEvent::Deleted` თითო აქტივზე. შეცდომები: `FindError::AssetDefinition`, `InvariantViolation` მითითების კონფლიქტებზე. კოდი: `core/.../isi/domain.rs`.
  - NFT-ის გაუქმება: ამოიღებს NFT-ს და ასუფთავებს ანგარიშს-/როლზე მორგებული ნებართვის ჩანაწერებს, რომლებიც მიუთითებს ამოღებულ NFT-ზე. ღონისძიებები: `NftEvent::Deleted`. შეცდომები: `FindError::Nft`. კოდი: `core/.../isi/nft.rs`.
  - როლის გაუქმება: პირველ რიგში გააუქმებს როლს ყველა ანგარიშიდან; შემდეგ ხსნის როლს. ღონისძიებები: `RoleEvent::Deleted`. შეცდომები: `FindError::Role`. კოდი: `core/.../isi/world.rs`.- Unregister Trigger: ამოიღებს ტრიგერს, თუ არსებობს და წყვეტს ანგარიშს/როლზე მორგებული ნებართვის ჩანაწერებს, რომლებიც მიუთითებენ ამოღებულ ტრიგერზე; დერეგისტრაციის დუბლიკატი იძლევა `Repetition(Unregister, TriggerId)`. ღონისძიებები: `TriggerEvent::Deleted`. კოდი: `core/.../isi/triggers/mod.rs`.

### ზარაფხანა / დამწვრობა
ტიპები: `Mint<O, D: Identifiable>` და `Burn<O, D: Identifiable>`, ყუთში როგორც `MintBox`/`BurnBox`.

- აქტივი (რიცხვითი) ზარაფხანა/დაწვა: არეგულირებს ნაშთებს და განსაზღვრავს `total_quantity`.
  - წინაპირობები: `Numeric` მნიშვნელობა უნდა აკმაყოფილებდეს `AssetDefinition.spec()`; ზარაფხანა დაშვებულია `mintable`-ით:
    - `Infinitely`: ყოველთვის დასაშვებია.
    - `Once`: დასაშვებია ზუსტად ერთხელ; პირველი ზარაფხანა აბრუნებს `mintable`-ს `Not`-ზე და ასხივებს `AssetDefinitionEvent::MintabilityChanged`, პლუს დეტალური `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` აუდიტორობისთვის.
    - `Limited(n)`: საშუალებას იძლევა `n` დამატებითი ზარაფხანის ოპერაციები. ყოველი წარმატებული ზარაფხანა ამცირებს მრიცხველს; როდესაც ის მიაღწევს ნულს, განსაზღვრება გადადის `Not`-ზე და ასხივებს იგივე `MintabilityChanged` მოვლენებს, როგორც ზემოთ.
    - `Not`: შეცდომა `MintabilityError::MintUnmintable`.
  - სახელმწიფო ცვლილებები: ქმნის აქტივს, თუ აკლია ზარაფხანას; ამოიღებს აქტივის ჩანაწერს, თუ ნაშთი ნულის ტოლია დამწვრობისას.
  - ღონისძიებები: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (როდესაც `Once` ან `Limited(n)` ამოწურავს თავის შემწეობას).
  - შეცდომები: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. კოდი: `core/.../isi/asset.rs`.- ტრიგერის გამეორებები ზარაფხანა/დაწვა: ცვლილებები `action.repeats` ითვლის ტრიგერისთვის.
  - წინაპირობები: პიტნაზე, ფილტრი უნდა იყოს დასამუშავებელი; არითმეტიკა არ უნდა გადადიოდეს/დაქვეითდეს.
  - ღონისძიებები: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - შეცდომები: `MathError::Overflow` არასწორი ზარაფხანაზე; `FindError::Trigger` თუ აკლია. კოდი: `core/.../isi/triggers/mod.rs`.

### ტრანსფერი
ტიპები: `Transfer<S: Identifiable, O, D: Identifiable>`, ყუთში როგორც `TransferBox`.

- აქტივი (რიცხვითი): გამოაკელი წყარო `AssetId`, დაამატეთ დანიშნულების ადგილი `AssetId` (იგივე განმარტება, განსხვავებული ანგარიში). ნულოვანი წყაროს აქტივის წაშლა.
  - წინაპირობები: წყარო აქტივი არსებობს; მნიშვნელობა აკმაყოფილებს `spec`.
  - ღონისძიებები: `AssetEvent::Removed` (წყარო), `AssetEvent::Added` (დანიშნულება).
  - შეცდომები: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. კოდი: `core/.../isi/asset.rs`.

- დომენის საკუთრება: ცვლის `Domain.owned_by` დანიშნულების ანგარიშს.
  - წინაპირობები: ორივე ანგარიში არსებობს; დომენი არსებობს.
  - ღონისძიებები: `DomainEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/Domain`. კოდი: `core/.../isi/domain.rs`.

- AssetDefinition მფლობელობა: ცვლის `AssetDefinition.owned_by` დანიშნულების ანგარიშზე.
  - წინაპირობები: ორივე ანგარიში არსებობს; განმარტება არსებობს; წყარო უნდა ფლობდეს მას; ავტორიტეტი უნდა იყოს წყაროს ანგარიში, წყაროს დომენის მფლობელი ან აქტივის განსაზღვრის დომენის მფლობელი.
  - ღონისძიებები: `AssetDefinitionEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/AssetDefinition`. კოდი: `core/.../isi/account.rs`.- NFT მფლობელობა: ცვლის `Nft.owned_by` დანიშნულების ანგარიშს.
  - წინაპირობები: ორივე ანგარიში არსებობს; NFT არსებობს; წყარო უნდა ფლობდეს მას; ავტორიტეტი უნდა იყოს წყაროს ანგარიში, წყაროს დომენის მფლობელი, NFT დომენის მფლობელი ან ფლობდეს `CanTransferNft` ამ NFT-სთვის.
  - ღონისძიებები: `NftEvent::OwnerChanged`.
  - შეცდომები: `FindError::Account/Nft`, `InvariantViolation` თუ წყარო არ ფლობს NFT-ს. კოდი: `core/.../isi/nft.rs`.

### მეტამონაცემები: დააყენეთ/ამოშალეთ გასაღები-მნიშვნელობა
ტიპები: `SetKeyValue<T>` და `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`-ით. მოწოდებულია ყუთში ჩასმული ნომრები.

- კომპლექტი: აყენებს ან ცვლის `Metadata[key] = Json(value)`.
- ამოღება: ამოიღებს გასაღებს; შეცდომა თუ აკლია.
- მოვლენები: `<Target>Event::MetadataInserted` / `MetadataRemoved` ძველი/ახალი მნიშვნელობებით.
- შეცდომები: `FindError::<Target>` თუ სამიზნე არ არსებობს; `FindError::MetadataKey` ამოღებულ გასაღებზე. კოდი: `crates/iroha_data_model/src/isi/transparent.rs` და შემსრულებელი მიზნებისთვის.

### ნებართვები და როლები: გაცემა / გაუქმება
ტიპები: `Grant<O, D>` და `Revoke<O, D>`, ყუთებში `Permission`/`Role`-დან/`Account`-მდე და `Account`-მდე და `Account`-მდე და `Account`-მდე და Norito.- ანგარიშის ნებართვის მინიჭება: ამატებს `Permission`, თუ ​​უკვე თანდაყოლილი არ არის. ღონისძიებები: `AccountEvent::PermissionAdded`. შეცდომები: `Repetition(Grant, Permission)` თუ დუბლიკატია. კოდი: `core/.../isi/account.rs`.
- გააუქმეთ ნებართვა ანგარიშიდან: ამოღებულია, თუ არსებობს. ღონისძიებები: `AccountEvent::PermissionRemoved`. შეცდომები: `FindError::Permission` თუ არ არსებობს. კოდი: `core/.../isi/account.rs`.
- როლის მინიჭება ანგარიშზე: არყოფნის შემთხვევაში ათავსებს `(account, role)` რუკებს. ღონისძიებები: `AccountEvent::RoleGranted`. შეცდომები: `Repetition(Grant, RoleId)`. კოდი: `core/.../isi/account.rs`.
- როლის გაუქმება ანგარიშიდან: აშორებს რუკებს, თუ არსებობს. ღონისძიებები: `AccountEvent::RoleRevoked`. შეცდომები: `FindError::Role` თუ არ არსებობს. კოდი: `core/.../isi/account.rs`.
- როლის ნებართვის მინიჭება: აღადგენს როლს დამატებული ნებართვით. ღონისძიებები: `RoleEvent::PermissionAdded`. შეცდომები: `Repetition(Grant, Permission)`. კოდი: `core/.../isi/world.rs`.
- Role-დან ნებართვის გაუქმება: აღადგენს როლს ამ ნებართვის გარეშე. ღონისძიებები: `RoleEvent::PermissionRemoved`. შეცდომები: `FindError::Permission` თუ არ არსებობს. კოდი: `core/.../isi/world.rs`.### ტრიგერები: შეასრულეთ
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
- `TransactionBuilder`/`SignedTransaction`: აყალიბებს, ხელს აწერს და ათავსებს შესრულებადს მეტამონაცემებით, `chain_id`, `authority`, `creation_time_ms`, სურვილისამებრ, 10X03 და I08. `nonce`. კოდი: `crates/iroha_data_model/src/transaction/`.
- გაშვების დროს, `iroha_core` ახორციელებს `InstructionBox` პარტიებს `Execute for InstructionBox`-ის მეშვეობით, ჩამორთმევით შესაბამის `*Box`-მდე ან კონკრეტულ ინსტრუქციაზე. კოდი: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- გაშვების შემსრულებლის ვალიდაციის ბიუჯეტი (მომხმარებლის მიერ მოწოდებული შემსრულებელი): ბაზის `executor.fuel` პარამეტრებიდან პლუს არასავალდებულო ტრანზაქციის მეტამონაცემები `additional_fuel` (`u64`), გაზიარებული ინსტრუქციების/ტრიგერების ვალიდაციაში ტრანზაქციის ფარგლებში.

---## ინვარიანტები და შენიშვნები (ტესტებიდან და მცველებიდან)
- Genesis-ის დაცვა: ვერ დაარეგისტრირებს `genesis` დომენს ან ანგარიშებს `genesis` დომენში; `genesis` ანგარიშის რეგისტრაცია შეუძლებელია. კოდი/ტესტები: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- რიცხვითი აქტივები უნდა აკმაყოფილებდეს მათ `NumericSpec` ზარაფხანაზე/გადაცემაზე/დაწვაზე; სპეციფიკაციების შეუსაბამობა იძლევა `TypeError::AssetNumericSpec`.
- ზარაფხანადობა: `Once` საშუალებას იძლევა ერთი ზარაფხანა და შემდეგ გადატრიალდება `Not`-ზე; `Limited(n)` საშუალებას აძლევს ზუსტად `n` მოჭრას `Not`-ზე გადაბრუნებამდე. `Infinitely`-ზე მოჭრის აკრძალვის მცდელობები იწვევს `MintabilityError::ForbidMintOnMintable`, ხოლო `Limited(0)`-ის კონფიგურაცია იძლევა `MintabilityError::InvalidMintabilityTokens`-ს.
- მეტამონაცემების ოპერაციები საკვანძო-ზუსტია; არარსებული გასაღების ამოღება შეცდომაა.
- ტრიგერის ფილტრები შეიძლება იყოს არასამუშაო; მაშინ `Register<Trigger>` მხოლოდ `Exactly(1)` გამეორების საშუალებას იძლევა.
- ტრიგერის მეტამონაცემების გასაღები `__enabled` (bool) კარიბჭის შესრულება; ნაგულისხმევი ნაგულისხმევი არ არის ჩართული და გამორთული ტრიგერები გამოტოვებულია მონაცემთა/დროის/გამოძახების ბილიკებზე.
- დეტერმინიზმი: ყველა არითმეტიკა იყენებს შემოწმებულ ოპერაციებს; under/overflow აბრუნებს აკრეფილ მათემატიკურ შეცდომებს; ნულოვანი ნაშთები ამცირებს აქტივების ჩანაწერებს (დამალული მდგომარეობის გარეშე).

---## პრაქტიკული მაგალითები
- მოჭრა და გადატანა:
  - `Mint::asset_numeric(10, asset_id)` → ამატებს 10-ს, თუ ეს ნებადართულია სპეციფიკაციებით/დამზადებით; მოვლენები: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → მოძრაობს 5; მოხსნის/დამატების ღონისძიებები.
- მეტამონაცემების განახლებები:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; ამოღება `RemoveKeyValue::account(...)`-ის საშუალებით.
- როლების/ნებართვების მართვა:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` და მათი `Revoke` კოლეგები.
- ტრიგერის სასიცოცხლო ციკლი:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` ფილტრით ნაგულისხმევი დამუშავების შემოწმებით; `ExecuteTrigger::new(id).with_args(&args)` უნდა შეესაბამებოდეს კონფიგურირებულ ავტორიტეტს.
  - ტრიგერები შეიძლება გამორთოთ მეტამონაცემების კლავიშის `__enabled`-ზე `false`-ზე დაყენებით (ნაგულისხმევი ნაგულისხმევი არ არის ჩართული); გადართვა `SetKeyValue::trigger` ან IVM `set_trigger_enabled` syscall-ის მეშვეობით.
  - ტრიგერების საცავი შეკეთებულია ჩატვირთვისას: დუბლიკატი ID, შეუსაბამო ID და გამოტოვებული ბაიტეკოდის მითითების ტრიგერები იშლება; ბაიტიკოდის მითითების რაოდენობა ხელახლა გამოითვლება.
  - თუ ტრიგერის IVM ბაიტეკოდი არ არის შესრულების დროს, ტრიგერი ამოღებულია და შესრულება განიხილება, როგორც არა-ოპტიმალური მარცხის შედეგი.
  - ამოწურული ტრიგერები დაუყოვნებლივ მოიხსნება; თუ ამოწურული ჩანაწერი შეგხვდება შესრულების დროს, ის იჭრება და განიხილება როგორც დაკარგული.
- პარამეტრის განახლება:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` განაახლებს და გამოსცემს `ConfigurationEvent::Changed`.CLI / Torii აქტივის განსაზღვრის ID + მეტსახელის მაგალითები:
- დარეგისტრირდით კანონიკური Base58 id + აშკარა სახელით + გრძელი მეტსახელით:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#bankb.sbp`
- დარეგისტრირდით კანონიკური Base58 id + აშკარა სახელით + მოკლე მეტსახელით:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- ზარაფხანა მეტსახელით + ანგარიშის კომპონენტები:
  - `iroha ledger asset mint --definition-alias pkr#bankb.sbp --account <i105> --quantity 500`
- გადაწყვიტეთ მეტსახელი კანონიკური Base58 id-ით:
  - `POST /v1/assets/aliases/resolve` JSON `{ "alias": "pkr#bankb.sbp" }`-ით

მიგრაციის შენიშვნა:
- `name#domain` ტექსტური აქტივების განსაზღვრის ID-ები განზრახ მხარდაჭერილი რჩება პირველ გამოშვებაში; გამოიყენეთ კანონიკური Base58 ID-ები ან ამოიღეთ წერტილოვანი მეტსახელი.
- საჯარო აქტივების სელექტორები იყენებენ კანონიკურ Base58 აქტივების განსაზღვრის ID-ებს პლუს მფლობელობის გაყოფის ველებს (`account`, სურვილისამებრ `scope`). ნედლი კოდირებული `AssetId` ლიტერალები რჩება შიდა დამხმარეებად და არ არის Torii/CLI სელექტორის ზედაპირის ნაწილი.
- აქტივების განსაზღვრების სია/შეკითხვის ფილტრები და დალაგება დამატებით იღებს `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` და `alias_binding.bound_at_ms`.

---

## მიკვლევადობა (არჩეული წყაროები)
 - მონაცემთა მოდელის ბირთვი: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI განმარტებები და რეესტრი: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI შესრულება: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - ღონისძიებები: `crates/iroha_data_model/src/events/**`.
 - ტრანზაქციები: `crates/iroha_data_model/src/transaction/**`.

თუ გსურთ, რომ ეს სპეციფიკა გაფართოვდეს API/ქცევის ცხრილად ან ჯვარედინი ბმული იყოს ყველა კონკრეტულ მოვლენასთან/შეცდომასთან, თქვით სიტყვა და მე გავაგრძელებ.