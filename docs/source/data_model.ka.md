---
lang: ka
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 მონაცემთა მოდელი – ღრმა ჩაყვინთვის

ეს დოკუმენტი განმარტავს სტრუქტურებს, იდენტიფიკატორებს, მახასიათებლებს და პროტოკოლებს, რომლებიც ქმნიან Iroha v2 მონაცემთა მოდელს, რომელიც დანერგილია `iroha_data_model` კრატში და გამოიყენება სამუშაო სივრცეში. ეს არის ზუსტი მითითება, რომელსაც შეგიძლიათ გადახედოთ და შესთავაზოთ განახლებები.

## სფერო და საფუძვლები

- მიზანი: მიაწოდეთ კანონიკური ტიპები დომენის ობიექტებისთვის (დომენები, ანგარიშები, აქტივები, NFT, როლები, ნებართვები, თანატოლები), მდგომარეობის შეცვლის ინსტრუქციები (ISI), მოთხოვნები, ტრიგერები, ტრანზაქციები, ბლოკები და პარამეტრები.
- სერიალიზაცია: ყველა საჯარო ტიპი იღებს Norito კოდეკებს (`norito::codec::{Encode, Decode}`) და სქემას (`iroha_schema::IntoSchema`). JSON გამოიყენება შერჩევით (მაგ., HTTP და `Json` დატვირთვისთვის) ფუნქციების დროშების მიღმა.
- IVM შენიშვნა: გარკვეული დესერიალიზაციის დროის ვალიდაცია გათიშულია Iroha ვირტუალური აპარატის (IVM) მიზნებისას, ვინაიდან ჰოსტი ასრულებს ვალიდაციას კონტრაქტების გამოძახებამდე (იხილეთ კრატის დოკუმენტები I1003NI00X-ში).
- FFI კარიბჭე: ზოგიერთი ტიპი პირობითად არის ანოტირებული FFI-სთვის `iroha_ffi`-ის საშუალებით `ffi_export`/`ffi_import`-ის უკან, რათა თავიდან იქნას აცილებული ზედნადები, როდესაც FFI არ არის საჭირო.

## ძირითადი თვისებები და დამხმარეები- `Identifiable`: ერთეულებს აქვთ სტაბილური `Id` და `fn id(&self) -> &Self::Id`. უნდა იყოს მიღებული `IdEqOrdHash`-ით რუქის/კომპლექტის კეთილგანწყობისთვის.
- `Registrable`/`Registered`: ბევრი ერთეული (მაგ., `Domain`, `AssetDefinition`, `Role`) იყენებს აღმშენებლის შაბლონს. `Registered` აკავშირებს გაშვების ტიპს მსუბუქ მშენებლის ტიპთან (`With`), რომელიც შესაფერისია სარეგისტრაციო ტრანზაქციებისთვის.
- `HasMetadata`: ერთიანი წვდომა გასაღების/მნიშვნელობის `Metadata` რუკაზე.
- `IntoKeyValue`: მეხსიერების გაყოფის დამხმარე `Key` (ID) და `Value` (მონაცემების) ცალკე შესანახად დუბლირების შესამცირებლად.
- `Owned<T>`/`Ref<'world, K, V>`: მსუბუქი შეფუთვები, რომლებიც გამოიყენება საცავებში და შეკითხვის ფილტრებში, ზედმეტი ასლების თავიდან ასაცილებლად.

## სახელები და იდენტიფიკატორები- `Name`: სწორი ტექსტური იდენტიფიკატორი. არ იძლევა უფსკრული და დაჯავშნილი სიმბოლოები `@`, `#`, `$` (გამოიყენება კომპოზიტურ ID-ებში). კონსტრუირებადია `FromStr`-ის მეშვეობით ვალიდაციით. სახელები ნორმალიზებულია Unicode NFC-ზე ანალიზზე (კანონიკურად ექვივალენტური მართლწერები განიხილება, როგორც იდენტური და ინახება შედგენილი). სპეციალური სახელწოდება `genesis` დაცულია (შემოწმებულია ასოების გარეშე).
- `IdBox`: ჯამის ტიპის კონვერტი ნებისმიერი მხარდაჭერილი ID-სთვის (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). სასარგებლოა ზოგადი ნაკადებისთვის და Norito კოდირებისთვის, როგორც ერთი ტიპის.
- `ChainId`: გაუმჭვირვალე ჯაჭვის იდენტიფიკატორი, რომელიც გამოიყენება ტრანზაქციებში განმეორებით დაცვისთვის.ID-ების სიმებიანი ფორმები (ორმხრივი `Display`/`FromStr`-ით):
- `DomainId`: `name` (მაგ., `wonderland`).
- `AccountId`: ანგარიშის კანონიკური დომენის იდენტიფიკატორი, კოდირებული მხოლოდ `AccountAddress`-ით, როგორც I105. პარსერის შეყვანები უნდა იყოს კანონიკური I105; დომენის სუფიქსები (`@domain`), კანონიკური I105 ლიტერალები, მეტსახელის ლიტერალები, კანონიკური თექვსმეტობითი პარსერის შეყვანა, ძველი `norito:` დატვირთვა და `uaid:`/`opaque:` არის ანგარიშის რეჟიმები.
- `AssetDefinitionId`: კანონიკური `aid:<32-lower-hex-no-dash>` (UUID-v4 ბაიტი).
- `AssetId`: კანონიკური კოდირებული ლიტერალი `norito:<hex>` (მემკვიდრეობითი ტექსტური ფორმები არ არის მხარდაჭერილი პირველ გამოშვებაში).
- `NftId`: `nft$domain` (მაგ., `rose$garden`).
- `PeerId`: `public_key` (თანასწორობა არის საჯარო გასაღებით).

## პირები

### დომენი
- `DomainId { name: Name }` – უნიკალური სახელი.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- მშენებელი: `NewDomain` `with_logo`, `with_metadata`, შემდეგ `Registrable::build(authority)` კომპლექტი `owned_by`.### ანგარიში
- `AccountId` არის კანონიკური ანგარიშის დომენის გარეშე, ჩასმული კონტროლერის მიერ და დაშიფრულია როგორც კანონიკური I105.
- `ScopedAccountId { account: AccountId, domain: DomainId }` ახორციელებს დომენის გამოკვეთილ კონტექსტს მხოლოდ იქ, სადაც საჭიროა სკოპური ხედი.
- `Account { id, metadata, label?, uaid? }` — `label` არის არასავალდებულო სტაბილური მეტსახელი, რომელსაც იყენებენ ხელახალი ჩანაწერები, `uaid` ატარებს არასავალდებულო Nexus ფართო [უნივერსალური ანგარიშის ID](Kotodama).
- Builder: `NewAccount` via `Account::new(id)`; რეგისტრაცია მოითხოვს აშკარა `ScopedAccountId` დომენს და არ გამოიტანს დასკვნას ნაგულისხმევიდან.

### აქტივების განმარტებები და აქტივები
- `AssetDefinitionId { aid_bytes: [u8; 16] }` ტექსტურად გამოფენილია, როგორც `aid:<32-hex-no-dash>`.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name` საჭიროა ადამიანის მიმართული ეკრანის ტექსტი და არ უნდა შეიცავდეს `#`/`@`.
  - `alias` არჩევითია და უნდა იყოს ერთ-ერთი:
    - `<name>#<domain>@<dataspace>`
    - `<name>#<dataspace>`
    მარცხენა სეგმენტით ზუსტად შეესაბამება `AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - მშენებლები: `AssetDefinition::new(id, spec)` ან კომფორტული `numeric(id)`; `name` საჭიროა და უნდა დაყენდეს `.with_name(...)`-ის მეშვეობით.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` შესანახად მოსახერხებელი `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: `Global` შეუზღუდავი ნაშთებისთვის და `Dataspace(DataSpaceId)` მონაცემთა სივრცით შეზღუდული ნაშთებისთვის.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` გამოფენილია შემაჯამებელი API-ებისთვის.### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (შინაარსი არის თვითნებური გასაღები/მნიშვნელობის მეტამონაცემები).
- მშენებელი: `NewNft` `Nft::new(id, content)`-ით.

### როლები და ნებართვები
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` მშენებელთან `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` და დატვირთვის სქემა უნდა შეესაბამებოდეს აქტიურ `ExecutorDataModel`-ს (იხ. ქვემოთ).

### თანატოლები
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` და გასარჩევი `public_key@address` სიმებიანი ფორმა.

### კრიპტოგრაფიული პრიმიტივები (მახასიათებელი `sm`)
- `Sm2PublicKey` და `Sm2Signature`: SEC1-თან თავსებადი წერტილები და ფიქსირებული სიგანის `r∥s` ხელმოწერები SM2-სთვის. კონსტრუქტორები ამოწმებენ მრუდის წევრობას და განმასხვავებელ ID-ებს; Norito კოდირება ასახავს `iroha_crypto`-ის მიერ გამოყენებულ კანონიკურ წარმოდგენას.
- `Sm3Hash`: `[u8; 32]` ახალი ტიპი, რომელიც წარმოადგენს GM/T 0004 დაიჯესტს, რომელიც გამოიყენება მანიფესტებში, ტელემეტრიაში და სისკალურ პასუხებში.
- `Sm4Key`: 128-ბიტიანი სიმეტრიული გასაღების გადასაფარებელი, რომელიც გაზიარებულია ჰოსტის სისტემასა და მონაცემთა მოდელის მოწყობილობებს შორის.
ეს ტიპები დგანან არსებულ Ed25519/BLS/ML-DSA პრიმიტივებთან ერთად და გახდებიან საჯარო სქემის ნაწილი, როგორც კი სამუშაო სივრცე აშენდება `--features sm`-ით.### ტრიგერები და მოვლენები
- `TriggerId { name: Name }` და `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` ან `Exactly(u32)`; შეკვეთისა და ამოწურვის კომუნალური მომსახურება შედის.
  - უსაფრთხოება: `TriggerCompleted` არ შეიძლება გამოყენებულ იქნას მოქმედების ფილტრად (დამოწმებული (დე)სერიალიზაციის დროს).
- `EventBox`: ჯამის ტიპი მილსადენისთვის, მილსადენის ჯგუფისთვის, მონაცემები, დრო, შესრულება-გამშვები და ტრიგერით დასრულებული მოვლენები; `EventFilterBox` ასახავს გამოწერებს და ააქტიურებს ფილტრებს.

## პარამეტრები და კონფიგურაცია

- სისტემის პარამეტრების ოჯახები (ყველა `Default`ed, გადამყვანები და კონვერტირება ინდივიდუალურ რიცხვებად):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` აჯგუფებს ყველა ოჯახს და `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- ერთი პარამეტრიანი ნომრები: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` განსხვავებების მსგავსი განახლებისთვის და გამეორებისთვის.
- მორგებული პარამეტრები: შემსრულებლის მიერ განსაზღვრული, გადატანილი როგორც `Json`, იდენტიფიცირებული `CustomParameterId`-ით (a `Name`).

## ISI (Iroha სპეციალური ინსტრუქციები)- ძირითადი მახასიათებელი: `Instruction` `dyn_encode`, `as_any` და სტაბილური თითო ტიპის იდენტიფიკატორით `id()` (ნაგულისხმევია კონკრეტული ტიპის სახელით). ყველა ინსტრუქცია არის `Send + Sync + 'static`.
- `InstructionBox`: ფლობს `Box<dyn Instruction>` შეფუთვას კლონით/eq/ord-ით, რომელიც განხორციელებულია ID ტიპის + კოდირებული ბაიტების მეშვეობით.
- ჩაშენებული ინსტრუქციის ოჯახები ორგანიზებულია შემდეგნაირად:
  - `mint_burn`, `transfer`, `register` და დამხმარეების `transparent` ნაკრები.
  - აკრიფეთ ნომრები მეტა ნაკადებისთვის: `InstructionType`, ყუთში თანხები, როგორიცაა `SetKeyValueBox` (დომენი/ანგარიში/asset_def/nft/ტრიგერი).
- შეცდომები: მდიდარი შეცდომის მოდელი `isi::error`-ის ქვეშ (შეფასების ტიპის შეცდომები, შეცდომების პოვნა, შეჯამება, მათემატიკა, არასწორი პარამეტრები, გამეორება, ინვარიანტები).
- ინსტრუქციის რეესტრი: `instruction_registry!{ ... }` მაკრო აშენებს გაშვების დეკოდირების რეესტრს, რომელსაც აქვს ტიპის სახელი. გამოიყენება `InstructionBox` კლონის და Norito სერდის მიერ დინამიური (დე)სერიალიზაციის მისაღწევად. თუ რეესტრი აშკარად არ არის დაყენებული `set_instruction_registry(...)`-ის მეშვეობით, ჩაშენებული ნაგულისხმევი რეესტრი ყველა ძირითადი ISI-ით ზარმაცად არის დაინსტალირებული პირველივე გამოყენებისას, რათა შეინარჩუნოს ორობითი ფაილები.

## ტრანზაქციები- `Executable`: ან `Instructions(ConstVec<InstructionBox>)` ან `Ivm(IvmBytecode)`. `IvmBytecode` სერიდება როგორც base64 (გამჭვირვალე ახალი ტიპი `Vec<u8>`-ზე).
- `TransactionBuilder`: აყალიბებს ტრანზაქციის სასარგებლო დატვირთვას `chain`, `authority`, `creation_time_ms`, სურვილისამებრ `time_to_live_ms` და `ScopedAccountId`, `ScopedAccountId`, `nonce`, `nonce`, `chain`, `authority`, Norito. `Executable`.
  - დამხმარეები: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, I18200000216X, `set_ttl`, I1828NI000001
- `SignedTransaction` (ვერსიით `iroha_version`): ატარებს `TransactionSignature` და დატვირთვას; უზრუნველყოფს ჰეშინგს და ხელმოწერის გადამოწმებას.
- შესვლის პუნქტები და შედეგები:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` ჰეშირების დამხმარეებით.
  - `ExecutionStep(ConstVec<InstructionBox>)`: ინსტრუქციების ერთი შეკვეთილი პარტია ტრანზაქციაში.

## ბლოკები- `SignedBlock` (ვერსიით) კაფსულებს:
  - `signatures: BTreeSet<BlockSignature>` (ვალიდატორებისგან),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (მეორადი შესრულების მდგომარეობა), რომელიც შეიცავს `time_triggers`, შესვლის/შედეგის Merkle ხეებს, `transaction_results` და `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- კომუნალური საშუალებები: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, I182NI0000000
- Merkle roots: ტრანზაქციის შესვლის წერტილები და შედეგები მიიღება მერკლის ხეების მეშვეობით; შედეგად Merkle root მოთავსებულია ბლოკის სათაურში.
- ბლოკის ჩართვის მტკიცებულებები (`BlockProofs`) ავლენს როგორც შესვლის/შედეგის Merkle-ის მტკიცებულებებს, ასევე `fastpq_transcripts` რუქას, რათა ჯაჭვის მიღმა პროვაიდერებმა შეძლონ ტრანსაქციის ჰეშთან დაკავშირებული გადაცემის დელტას მოძიება.
- `ExecWitness` შეტყობინებები (გადაცემული Torii-ის მეშვეობით და კონსენსუსის ჭორებით დაფუძნებული ყულაბა) ახლა მოიცავს როგორც `fastpq_transcripts`-ს, ასევე პროვერდისთვის მზად `fastpq_batches: Vec<FastpqTransitionBatch>`-ს ჩაშენებული X0, roots (Norito,Norito) perm_root, tx_set_hash), ასე რომ, გარე პროვერებს შეუძლიათ მიიღონ კანონიკური FASTPQ რიგები ტრანსკრიპტების ხელახალი კოდირების გარეშე.

## შეკითხვები- ორი არომატი:
  - სინგულარული: დანერგვა `SingularQuery<Output>` (მაგ., `FindParameters`, `FindExecutorDataModel`).
  - განმეორებადი: დანერგვა `Query<Item>` (მაგ., `FindAccounts`, `FindAssets`, `FindDomains` და ა.შ.).
- ტიპის წაშლილი ფორმები:
  - `QueryBox<T>` არის ყუთში, წაშლილი `Query<Item = T>` Norito სერდიით, რომელსაც მხარს უჭერს გლობალური რეესტრი.
  - `QueryWithFilter<T> { query, predicate, selector }` აწყვილებს მოთხოვნას DSL პრედიკატთან/სელექტორთან; გარდაიქმნება წაშლილ გამეორებად მოთხოვნად `From`-ის საშუალებით.
- რეესტრი და კოდეკები:
  - `query_registry!{ ... }` აშენებს გლობალურ რეესტრს, რომელიც ასახავს კონკრეტული მოთხოვნის ტიპებს კონსტრუქტორებს ტიპის სახელის მიხედვით დინამიური დეკოდირებისთვის.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` და `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` არის ჯამის ტიპი ჰომოგენურ ვექტორებზე (მაგ., `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), პლუს ეფექტური კუპე და გაფართოების დამხმარეები.
- DSL: დანერგილია `query::dsl`-ში პროექციის ნიშნებით (`HasProjection<PredicateMarker>` / `SelectorMarker`) კომპილაციის დროში შემოწმებული პრედიკატებისა და სელექტორებისთვის. საჭიროების შემთხვევაში, `fast_dsl` ფუნქცია გამოავლენს მსუბუქ ვარიანტს.

## შემსრულებელი და გაფართოება- `Executor { bytecode: IvmBytecode }`: ვალიდატორის მიერ შესრულებული კოდის ნაკრები.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` აცხადებს შემსრულებლის მიერ განსაზღვრულ დომენს:
  - მორგებული კონფიგურაციის პარამეტრები,
  - მორგებული ინსტრუქციის იდენტიფიკატორები,
  - ნებართვის ნიშნის იდენტიფიკატორები,
  - JSON სქემა, რომელიც აღწერს კლიენტის ინსტრუმენტების მორგებულ ტიპებს.
- პერსონალიზაციის ნიმუშები არსებობს `data_model/samples/executor_custom_data_model`-ში, რომელიც აჩვენებს:
  - მორგებული ნებართვის ნიშანი `iroha_executor_data_model::permission::Permission` გამოყვანის საშუალებით,
  - მორგებული პარამეტრი განისაზღვრება, როგორც ტიპი, რომელიც კონვერტირებადია `CustomParameter`-ში,
  - მორგებული ინსტრუქციები სერიული `CustomInstruction`-ში შესასრულებლად.

### CustomInstruction (აღმასრულებელი განსაზღვრული ISI)- ტიპი: `isi::CustomInstruction { payload: Json }` სტაბილური მავთულის ID-ით `"iroha.custom"`.
- დანიშნულება: კონვერტი შემსრულებლის სპეციფიკური ინსტრუქციებისთვის კერძო/კონსორციუმის ქსელებში ან პროტოტიპებისთვის, საჯარო მონაცემთა მოდელის ჩანგლის გარეშე.
- შემსრულებლის ნაგულისხმევი ქცევა: `iroha_core`-ში ჩაშენებული შემსრულებელი არ ახორციელებს `CustomInstruction`-ს და შეხვედრის შემთხვევაში პანიკაში ჩავარდება. მორგებულმა შემსრულებელმა უნდა ჩამოაგდოს `InstructionBox` `CustomInstruction`-მდე და განმსაზღვრელი ინტერპრეტაცია მოახდინოს დატვირთვის ყველა ვალიდატორზე.
- Norito: დაშიფვრავს/გაშიფრავს `norito::codec::{Encode, Decode}`-ის მეშვეობით სქემით; `Json` ტვირთამწეობა სერიულად არის განსაზღვრული. ორმხრივი მგზავრობა სტაბილურია მანამ, სანამ ინსტრუქციის რეესტრში შედის `CustomInstruction` (ის ნაგულისხმევი რეესტრის ნაწილია).
- IVM: Kotodama იკრიბება IVM ბაიტეკოდზე (`.to`) და არის აპლიკაციის ლოგიკის რეკომენდებული გზა. გამოიყენეთ `CustomInstruction` მხოლოდ შემსრულებლის დონის გაფართოებებისთვის, რომლებიც ჯერ არ არის გამოხატული Kotodama-ში. უზრუნველყოს დეტერმინიზმი და იდენტური შემსრულებელი ორობითი ფაილები თანატოლებს შორის.
- არა საჯარო ქსელებისთვის: არ გამოიყენოთ საჯარო ჯაჭვებისთვის, სადაც ჰეტეროგენული შემსრულებლები რისკავს კონსენსუსის ჩანგლებს. ამჯობინეთ შემოგთავაზოთ ახალი ჩაშენებული ISI ზემოთ, როდესაც გჭირდებათ პლატფორმის ფუნქციები.

## მეტამონაცემები- `Metadata(BTreeMap<Name, Json>)`: გასაღები/მნიშვნელობის შენახვა, რომელიც მიმაგრებულია მრავალ ერთეულზე (`Domain`, `Account`, `AssetDefinition`, `Nft`, ტრიგერები და ტრანზაქციები).
- API: `contains`, `iter`, `get`, `insert` და (`transparent_api`-ით) `remove`.

## თვისებები და დეტერმინიზმი

- აღჭურვილია არასავალდებულო API-ების კონტროლით (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, Norito, `wonderland`0, `fast_dsl`001 `fault_injection`).
- დეტერმინიზმი: ყველა სერიალიზაცია იყენებს Norito დაშიფვრას, რომ იყოს პორტატული აპარატურაში. IVM ბაიტიკოდი არის გაუმჭვირვალე ბაიტის ბლოკი; აღსრულებამ არ უნდა შემოიღოს არადეტერმინისტული შემცირება. ჰოსტი ამოწმებს ტრანზაქციებს და აწვდის შეყვანებს IVM-ს დეტერმინისტულად.

### გამჭვირვალე API (`transparent_api`)- მიზანი: ავლენს სრულ, ცვალებადი წვდომას `#[model]` სტრუქტურებზე/რიცხვებზე შიდა კომპონენტებისთვის, როგორიცაა Torii, შემსრულებლები და ინტეგრაციის ტესტები. ამის გარეშე, ეს ელემენტები განზრახ გაუმჭვირვალეა, ამიტომ გარე SDK-ები ხედავენ მხოლოდ უსაფრთხო კონსტრუქტორებს და დაშიფრულ დატვირთვას.
- მექანიკა: `iroha_data_model_derive::model` მაკრო გადაწერს თითოეულ საჯარო ველს `#[cfg(feature = "transparent_api")] pub`-ით და ინახავს პირად ასლს ნაგულისხმევი კონსტრუქციისთვის. ფუნქციის ჩართვა აბრუნებს ამ cfgs-ებს, ასე რომ, `Account`, `Domain`, `Asset` და ა.შ. დესტრუქტურა ხდება მათი განმსაზღვრელი მოდულების გარეთ.
- ზედაპირის ამოცნობა: ყუთი ახორციელებს `TRANSPARENT_API: bool` მუდმივ ექსპორტს (გენერირდება `transparent_api.rs` ან `non_transparent_api.rs`). Downstream კოდს შეუძლია შეამოწმოს ეს დროშა და განშტოება, როდესაც ის უნდა დაუბრუნდეს გაუმჭვირვალე დამხმარეებს.
- ჩართვა: დაამატეთ `features = ["transparent_api"]` დამოკიდებულებას `Cargo.toml`-ში. სამუშაო სივრცის ყუთები, რომლებსაც სჭირდებათ JSON პროექცია (მაგ., `iroha_torii`) დროშის ავტომატურად გადამისამართება, მაგრამ მესამე მხარის მომხმარებლებმა უნდა გამორთონ ის, თუ ისინი არ აკონტროლებენ განლაგებას და არ მიიღებენ უფრო ფართო API ზედაპირს.

## სწრაფი მაგალითები

შექმენით დომენი და ანგარიში, განსაზღვრეთ აქტივი და შექმენით ტრანზაქცია ინსტრუქციებით:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "aid:2f17c72466f84a4bb8a8e24884fdcd2f".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

მოითხოვეთ ანგარიშები და აქტივები DSL-ით:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

გამოიყენეთ IVM ჭკვიანი კონტრაქტის ბაიტეკოდი:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

`aid` / მეტსახელი სწრაფი მითითება (CLI + Torii):

```bash
# Register an asset definition with canonical aid + explicit name + alias
iroha ledger asset definition register \
  --id aid:2f17c72466f84a4bb8a8e24884fdcd2f \
  --name pkr \
  --alias pkr#ubl@sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id aid:550e8400e29b41d4a7164466554400dd \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl@sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical aid via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl@sbp"}'
```მიგრაციის შენიშვნა:
- ძველი `name#domain` აქტივების განსაზღვრის ID არ არის მიღებული v1-ში.
- ზარაფხანის/დაწვის/გადაცემის აქტივების ID-ები რჩება კანონიკური `norito:<hex>`; ააშენეთ ისინი:
  - `iroha tools encode asset-id --definition aid:... --account <i105>`
  - ან `--alias <name>#<domain>@<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## ვერსია

- `SignedTransaction`, `SignedBlock` და `SignedQuery` არის კანონიკური Norito-ში კოდირებული სტრუქტურები. თითოეული ახორციელებს `iroha_version::Version`, რათა დააფიქსიროს თავისი დატვირთვა მიმდინარე ABI ვერსიასთან (ამჟამად `1`), როდესაც კოდირებულია `EncodeVersioned`-ით.

## მიმოხილვის შენიშვნები / პოტენციური განახლებები

- მოთხოვნა DSL: განიხილეთ მომხმარებლისთვის სტაბილური ქვეჯგუფის დოკუმენტირება და მაგალითები საერთო ფილტრებისთვის/სელექტორებისთვის.
- ინსტრუქციების ოჯახები: გააფართოვეთ საჯარო დოკუმენტები, სადაც ჩამოთვლილია `mint_burn`, `register`, `transfer` ჩაშენებული ISI ვარიანტები.

---
თუ რომელიმე ნაწილს მეტი სიღრმე სჭირდება (მაგ. სრული ISI კატალოგი, შეკითხვის რეესტრის სრული სია, ან სათაურის ველების დაბლოკვა), შემატყობინეთ და მე გავაგრძელებ ამ სექციებს შესაბამისად.