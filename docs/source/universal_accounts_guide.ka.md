<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# უნივერსალური ანგარიშის სახელმძღვანელო

ეს სახელმძღვანელო ასახავს UAID-ის (უნივერსალური ანგარიშის ID) მოთხოვნებს
Nexus საგზაო რუკა და ათავსებს მათ ოპერატორზე + SDK ორიენტირებულ გზამკვლევში.
იგი მოიცავს UAID-ის დერივაციას, პორტფელის/მანიფესტის შემოწმებას, მარეგულირებლის შაბლონებს,
და მტკიცებულება, რომელიც უნდა ახლდეს ყველა `iroha app space-directory manifest-ს
public` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID-ის სწრაფი მითითება- UAIDs არის `uaid:<hex>` ლიტერალი, სადაც `<hex>` არის Blake2b-256 დაიჯესტი
  LSB დაყენებულია `1`-ზე. კანონიკური ტიპი ცხოვრობს
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- ანგარიშის ჩანაწერებს (`Account` და `AccountDetails`) ახლა აქვს სურვილისამებრ `uaid`
  ველი, რათა აპლიკაციებმა ისწავლონ იდენტიფიკატორი შეკვეთილი ჰეშირების გარეშე.
- ფარული ფუნქციის იდენტიფიკატორის პოლიტიკას შეუძლია დააკავშიროს თვითნებური ნორმალიზებული შენატანი
  (ტელეფონის ნომრები, ელფოსტა, ანგარიშის ნომრები, პარტნიორის სტრიქონები) `opaque:` ID-ებზე
  UAID სახელთა სივრცის ქვეშ. ჯაჭვის ნაწილები არის `IdentifierPolicy`,
  `IdentifierClaimRecord` და `opaque_id -> uaid` ინდექსი.
- Space Directory ინახავს `World::uaid_dataspaces` რუკას, რომელიც აკავშირებს თითოეულ UAID-ს
  მონაცემთა სივრცის ანგარიშებზე, რომლებიც მითითებულია აქტიური მანიფესტების მიერ. Torii ხელახლა იყენებს ამას
  რუკა `/portfolio` და `/uaids/*` API-ებისთვის.
- `POST /v1/accounts/onboard` აქვეყნებს ნაგულისხმევი Space Directory მანიფესტს
  გლობალური მონაცემთა სივრცე, როდესაც არცერთი არ არსებობს, ამიტომ UAID დაუყოვნებლივ იკვრება.
  საბორტო ორგანოებმა უნდა დაიცვან `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- ყველა SDK ავლენს დამხმარეებს UAID ლიტერალების კანონიკიზაციისთვის (მაგ.,
  `UaidLiteral` Android SDK-ში). დამხმარეები იღებენ ნედლეულ 64 თექვსმეტიან დიჯესტს
  (LSB=1) ან `uaid:<hex>` ლიტერალები და ხელახლა გამოიყენეთ იგივე Norito კოდეკები, ასე რომ
  დაიჯესტი ვერ გადაინაცვლებს ენებზე.

## 1.1 დამალული იდენტიფიკატორის პოლიტიკა

UAID-ები ახლა არის მეორე იდენტურობის ფენის წამყვანი:- გლობალური `IdentifierPolicyId` (`<kind>#<business_rule>`) განსაზღვრავს
  სახელთა სივრცე, საჯარო ვალდებულების მეტამონაცემები, გადამწყვეტის დადასტურების გასაღები და
  კანონიკური შეყვანის ნორმალიზაციის რეჟიმი (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, ან `AccountNumber`).
- პრეტენზია აკავშირებს ერთ მიღებულ `opaque:` იდენტიფიკატორს ზუსტად ერთ UAID-თან და ერთთან
  კანონიკური `AccountId` ამ პოლიტიკის მიხედვით, მაგრამ ჯაჭვი იღებს მხოლოდ
  პრეტენზია, როდესაც მას ახლავს ხელმოწერილი `IdentifierResolutionReceipt`.
- გარჩევადობა რჩება `resolve -> transfer` ნაკადად. Torii წყვეტს გაუმჭვირვალობას
  ამუშავებს და აბრუნებს კანონიკურ `AccountId`-ს; ტრანსფერები კვლავ მიზნად ისახავს
  კანონიკური ანგარიში და არა პირდაპირ `uaid:` ან `opaque:`.
- პოლიტიკას ახლა შეუძლია BFV შეყვანის დაშიფვრის პარამეტრების გამოქვეყნება
  `PolicyCommitment.public_parameters`. როდესაც არსებობს, Torii აქვეყნებს მათ რეკლამას
  `GET /v1/identifier-policies` და კლიენტებს შეუძლიათ წარადგინონ BFV-შეფუთული შენატანი
  ჩვეულებრივი ტექსტის ნაცვლად. დაპროგრამებული პოლიტიკა ახვევს BFV პარამეტრებს ა
  კანონიკური `BfvProgrammedPublicParameters` პაკეტი, რომელიც ასევე აქვეყნებს
  საჯარო `ram_fhe_profile`; მემკვიდრეობითი ნედლეული BFV დატვირთვები განახლებულია მასზე
  კანონიკური შეკვრა, როდესაც ვალდებულება აღდგება.
- იდენტიფიკატორის მარშრუტები გადის იმავე Torii წვდომის ნიშნით და განაკვეთის ლიმიტით
  ამოწმებს, როგორც სხვა აპის საბოლოო წერტილებს. ისინი არ არიან შემოვლითი გზა ნორმალურის გარშემო
  API პოლიტიკა.

## 1.2 ტერმინოლოგია

დასახელების გაყოფა მიზანმიმართულია:- `ram_lfe` არის გარე ფარული ფუნქციის აბსტრაქცია. ის მოიცავს პოლიტიკას
  რეგისტრაცია, ვალდებულებები, საჯარო მეტამონაცემები, შესრულების ქვითრები და
  გადამოწმების რეჟიმი.
- `BFV` არის Brakerski/Fan-Vercauteren ჰომორფული დაშიფვრის სქემა, რომელსაც იყენებს
  ზოგიერთი `ram_lfe` backends დაშიფრული შეყვანის შესაფასებლად.
- `ram_fhe_profile` არის BFV-სპეციფიკური მეტამონაცემები და არა მთლიანი მეორე სახელი
  თვისება. იგი აღწერს დაპროგრამებულ BFV აღსრულების მანქანას, რომელიც საფულეებს და
  ვერიფიკატორები უნდა იყოს მიზანმიმართული, როდესაც პოლიტიკა იყენებს დაპროგრამებულ ბექენდს.

კონკრეტულად:

- `RamLfeProgramPolicy` და `RamLfeExecutionReceipt` არის LFE ფენის ტიპები.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` და
  `BfvRamProgramProfile` არის FHE ფენის ტიპები.
- `HiddenRamFheProgram` და `HiddenRamFheInstruction` არის შიდა სახელები
  ფარული BFV პროგრამა, რომელიც შესრულებულია დაპროგრამებული ბექენდის მიერ. ისინი რჩებიან
  FHE მხარეს, რადგან ისინი აღწერენ დაშიფრული შესრულების მექანიზმს, ვიდრე
  გარე პოლიტიკა ან ქვითრის აბსტრაქცია.

## 1.3 ანგარიშის იდენტურობა მეტსახელების წინააღმდეგ

უნივერსალური ანგარიშის გაშვება არ ცვლის ანგარიშის კანონიკური იდენტობის მოდელს:- `AccountId` რჩება ანგარიშის კანონიკურ, დომენის გარეშე.
- `AccountAlias` მნიშვნელობები არის ცალკეული SNS შეკვრა ამ საგნის თავზე. ა
  დომენის კვალიფიცირებული მეტსახელი, როგორიცაა `merchant@banka.sbp` და dataspace-root მეტსახელი
  როგორიცაა `merchant@sbp` შეიძლება ორივე გადაჭრას იმავე კანონიკურ `AccountId`-ზე.
- კანონიკური ანგარიშის რეგისტრაცია ყოველთვის არის `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; არ არსებობს დომენით კვალიფიცირებული ან დომენით მატერიალიზებული
  რეგისტრაციის გზა.
- დომენის საკუთრება, მეტსახელის ნებართვები და დომენის მასშტაბის სხვა ქცევები ცოცხალია
  საკუთარ სახელმწიფოში და API-ებში და არა თავად ანგარიშის იდენტურობაში.
- საჯარო ანგარიშის ძებნა შემდეგნაირად ხდება ამ გაყოფა: ალიასის მოთხოვნები რჩება საჯარო, ხოლო
  კანონიკური ანგარიშის იდენტურობა რჩება სუფთა `AccountId`.

განხორციელების წესი ოპერატორებისთვის, SDK-ებისთვის და ტესტებისთვის: დაიწყეთ კანონიკურიდან
`AccountId`, შემდეგ დაამატეთ მეტსახელის იჯარა, მონაცემთა სივრცის/დომენის ნებართვები და ნებისმიერი
დომენის საკუთრებაში არსებული სახელმწიფო ცალკე. არ მოახდინოთ ყალბი ფსევდონიმით მიღებული ანგარიშის სინთეზირება
ან ველით რაიმე დაკავშირებული დომენის ველს ანგარიშის ჩანაწერებში მხოლოდ იმიტომ, რომ მეტსახელი ან
მარშრუტი ატარებს დომენის სეგმენტს.

მიმდინარე Torii მარშრუტები:| მარშრუტი | დანიშნულება |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | ჩამოთვლილია აქტიური და არააქტიური RAM-LFE პროგრამის პოლიტიკა, პლუს მათი საჯარო აღსრულების მეტამონაცემები, მათ შორის არჩევითი BFV `input_encryption` პარამეტრები და დაპროგრამებული სარეზერვო `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | იღებს ზუსტად ერთს `{ input_hex }`-დან ან `{ encrypted_input }`-დან და აბრუნებს მოქალაქეობის არმქონე `RamLfeExecutionReceipt` პლუს `{ output_hex, output_hash, receipt_hash }` არჩეული პროგრამისთვის. მიმდინარე Torii გაშვების დრო გასცემს ქვითრებს დაპროგრამებული BFV backend-ისთვის. |
| `POST /v1/ram-lfe/receipts/verify` | მოქალაქეობის გარეშე ამოწმებს `RamLfeExecutionReceipt` გამოქვეყნებულ ჯაჭვზე პროგრამის პოლიტიკას და სურვილისამებრ ამოწმებს, რომ აბონენტის მიერ მიწოდებული `output_hex` ემთხვევა `output_hash` ქვითარს. |
| `GET /v1/identifier-policies` | ჩამოთვლილია აქტიური და არააქტიური ფარული ფუნქციების პოლიტიკის სახელთა სივრცეები პლუს მათი საჯარო მეტამონაცემები, მათ შორის არჩევითი BFV `input_encryption` პარამეტრები, საჭირო `normalization` რეჟიმი დაშიფრული კლიენტის მხრიდან და `ram_fhe_profile` დაპროგრამებული BFV პოლიტიკისთვის. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | იღებს ზუსტად ერთს `{ input }` ან `{ encrypted_input }`-დან. Plaintext `input` არის ნორმალიზებული სერვერის მხრიდან; BFV `encrypted_input` უკვე უნდა იყოს ნორმალიზებული გამოქვეყნებული პოლიტიკის რეჟიმის მიხედვით. შემდეგ საბოლოო წერტილი გამოიმუშავებს `opaque:` სახელურს და აბრუნებს ხელმოწერილ ქვითარს, რომელიც `ClaimIdentifier`-ს შეუძლია წარადგინოს ჯაჭვზე, მათ შორის როგორც ნედლი `signature_payload_hex`, ასევე გაანალიზებული `signature_payload`. || `POST /v1/identifiers/resolve` | იღებს ზუსტად ერთს `{ input }` ან `{ encrypted_input }`-დან. Plaintext `input` არის ნორმალიზებული სერვერის მხრიდან; BFV `encrypted_input` უკვე უნდა იყოს ნორმალიზებული გამოქვეყნებული პოლიტიკის რეჟიმის მიხედვით. საბოლოო წერტილი ანაწილებს იდენტიფიკატორს `{ opaque_id, receipt_hash, uaid, account_id, signature }`-ში, როდესაც აქტიური პრეტენზია არსებობს და ასევე აბრუნებს კანონიკურ ხელმოწერილ დატვირთვას, როგორც `{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | ეძებს მუდმივ `IdentifierClaimRecord`-ს, რომელიც დაკავშირებულია ქვითრის დეტერმინისტულ ჰეშთან, რათა ოპერატორებმა და SDK-ებმა შეძლონ საკუთრების უფლების აუდიტი ან დიაგნოსტიკა განმეორებითი / შეუსაბამობის წარუმატებლობები სრული იდენტიფიკატორის ინდექსის სკანირების გარეშე. |

Torii-ის პროცესის დროს შესრულების დრო კონფიგურირებულია
`torii.ram_lfe.programs[*]`, გასაღებით `program_id`. იდენტიფიკატორი მარშრუტები ახლა
ხელახლა გამოიყენეთ იგივე RAM-LFE გაშვების დრო ცალკე `identifier_resolver`-ის ნაცვლად
კონფიგურაციის ზედაპირი.

ამჟამინდელი SDK მხარდაჭერა:- `normalizeIdentifierInput(value, normalization)` ემთხვევა ჟანგს
  კანონიკალიზატორები `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address` და `account_number`.
- `ToriiClient.listIdentifierPolicies()` ჩამოთვლის პოლიტიკის მეტამონაცემებს, BFV-ს ჩათვლით
  შეყვანის დაშიფვრის მეტამონაცემები, როდესაც პოლიტიკა აქვეყნებს მას, პლუს გაშიფრული
  BFV პარამეტრის ობიექტი `input_encryption_public_parameters_decoded`-ის საშუალებით.
  დაპროგრამებული პოლიტიკა ასევე ავლენს დეკოდირებულ `ram_fhe_profile`-ს. ეს სფეროა
  განზრახ BFV-ს სპექტში: ის საშუალებას აძლევს საფულეებს გადაამოწმონ მოსალოდნელი რეგისტრი
  რაოდენობა, ზოლის რაოდენობა, კანონიკიზაციის რეჟიმი და მინიმალური შიფრული ტექსტის მოდული
  დაპროგრამებული FHE backend კლიენტის მხრიდან შეყვანის დაშიფვრამდე.
- `getIdentifierBfvPublicParameters(policy)` და
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` დახმარება
  JS აბონენტები მოიხმარენ გამოქვეყნებულ BFV მეტამონაცემებს და ქმნიან პოლიტიკის გაცნობიერების მოთხოვნას
  ორგანოები პოლიტიკის id და ნორმალიზაციის წესების ხელახალი განხორციელების გარეშე.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` და
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` ახლა ნება
  JS საფულეები ქმნიან BFV Norito შიფრული ტექსტის სრულ კონვერტს ადგილობრივად
  გამოქვეყნებული პოლიტიკის პარამეტრები, წინასწარ აშენებული შიფრული ტექსტის ექვსკუთხედის გაგზავნის ნაცვლად.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  ამოხსნის ფარულ იდენტიფიკატორს და აბრუნებს ხელმოწერილი ქვითრის დატვირთვას,
  მათ შორის `receipt_hash`, `signature_payload_hex` და
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId, input |
  დაშიფრული შეყვანა })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` ამოწმებს დაბრუნებულს
  ქვითარი კლიენტის მხარეს პოლიტიკის გადამწყვეტი გასაღების წინააღმდეგ და`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` იღებს
  შენარჩუნებული საჩივრის ჩანაწერი შემდგომი აუდიტის/გამართვის ნაკადებისთვის.
- `IrohaSwift.ToriiClient` ახლა ამჟღავნებს `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  და `getIdentifierClaimByReceiptHash(_)`, პლუს
  `ToriiIdentifierNormalization` იმავე ტელეფონისთვის/ელ.ფოსტის/ანგარიშის ნომრისთვის
  კანონიკიზაციის რეჟიმები.
- `ToriiIdentifierLookupRequest` და
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` დამხმარეები უზრუნველყოფენ აკრეფილ Swift მოთხოვნის ზედაპირს
  ზარების გადაჭრა და პრეტენზია-მიღება, ხოლო Swift-ის პოლიტიკას ახლა შეუძლია BFV-ის მოპოვება
  შიფრული ტექსტი ადგილობრივად `encryptInput(...)` / `encryptedRequest(input:...)`-ით.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` ადასტურებს ამას
  ზედა დონის ქვითრის ველები ემთხვევა ხელმოწერილი დატვირთვას და ამოწმებს
  გადამწყვეტის ხელმოწერა კლიენტის მხარეს გაგზავნამდე.
- `HttpClientTransport` Android SDK-ში ახლა ვლინდება
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, შეყვანა,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(accountId, PolicyId,
  შეყვანა, დაშიფრულიInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  პლუს `IdentifierNormalization` იგივე კანონიკიზაციის წესებისთვის.
- `IdentifierResolveRequest` და
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` დამხმარეები უზრუნველყოფენ აკრეფილ Android მოთხოვნის ზედაპირს,
  ხოლო `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` ვიღებთ BFV შიფრული ტექსტის კონვერტს
  ადგილობრივად გამოქვეყნებული პოლიტიკის პარამეტრებიდან.
  `IdentifierResolutionReceipt.verifySignature(policy)` ამოწმებს დაბრუნებულს
  გადამწყვეტი ხელმოწერის კლიენტის მხარეს.

მიმდინარე ინსტრუქციის ნაკრები:- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (მიმღები; დაუმუშავებელი `opaque_id` პრეტენზიები უარყოფილია)
- `RevokeIdentifier`

`iroha_crypto::ram_lfe`-ში ახლა არსებობს სამი backend:

- ისტორიული ვალდებულებით შეკრული `HKDF-SHA3-512` PRF და
- BFV მხარდაჭერილი საიდუმლო აფინური შემფასებელი, რომელიც მოიხმარს BFV დაშიფრულ იდენტიფიკატორს
  სლოტები პირდაპირ. როდესაც `iroha_crypto` აგებულია ნაგულისხმევად
  `bfv-accel` ფუნქცია, BFV რგოლის გამრავლება იყენებს ზუსტ დეტერმინისტიკას
  CRT-NTT backend შიდა; ამ ფუნქციის გამორთვა უბრუნდება
  სკალარული სასკოლო წიგნის გზა იდენტური შედეგებით და
- BFV-ს მხარდაჭერილი საიდუმლო დაპროგრამებული შემფასებელი, რომელიც გამოიმუშავებს ინსტრუქციას
  RAM-ის სტილის შესრულების კვალი დაშიფრულ რეგისტრებსა და შიფრული ტექსტის მეხსიერებაზე
  ბილიკები გაუმჭვირვალე იდენტიფიკატორის და ქვითრის ჰეშის მიღებამდე. დაპროგრამებული
  backend ახლა მოითხოვს უფრო ძლიერ BFV მოდულის იატაკს, ვიდრე affine ბილიკი, და
  მისი საჯარო პარამეტრები გამოქვეყნებულია კანონიკურ პაკეტში, რომელიც მოიცავს
  RAM-FHE შესრულების პროფილი მოხმარებული საფულეებისა და ვერიფიკატორების მიერ.

აქ BFV ნიშნავს Brakerski/Fan-Vercauteren FHE სქემას, რომელიც განხორციელდა
`crates/iroha_crypto/src/fhe_bfv.rs`. ეს არის დაშიფრული შესრულების მექანიზმი
გამოიყენება აფინური და დაპროგრამებული ბექენდების მიერ და არა გარე დამალულის სახელი
ფუნქციის აბსტრაქცია.Torii იყენებს პოლიტიკის ვალდებულებით გამოქვეყნებულ backend-ს. როდესაც BFV backend
აქტიურია, უბრალო ტექსტის მოთხოვნები ნორმალიზდება, შემდეგ დაშიფრულია სერვერის მხრიდან
შეფასება. BFV `encrypted_input` მოთხოვნები affine backend-ისთვის შეფასებულია
პირდაპირ და უკვე უნდა იყოს ნორმალიზებული კლიენტის მხრიდან; დაპროგრამებული backend
ახდენს დაშიფრული შეყვანის კანონიკალიზაციას გადამწყვეტის დეტერმინისტულ BFV-ზე
კონვერტი საიდუმლო ოპერატიული მეხსიერების პროგრამის შესრულებამდე, ასე რომ, ქვითრის ჰეშები რჩება
სტაბილურია სემანტიკურად ეკვივალენტურ შიფრულ ტექსტებში.

## 2. UAID-ების გამომუშავება და გადამოწმება

UAID-ის მისაღებად სამი მხარდაჭერილი გზა არსებობს:

1. **წაიკითხეთ მსოფლიო სახელმწიფოს ან SDK მოდელებიდან.** ნებისმიერი `Account`/`AccountDetails`
   Torii-ის მეშვეობით მოთხოვნილი დატვირთვის ველი ახლა შევსებულია `uaid`, როდესაც
   მონაწილემ აირჩია უნივერსალური ანგარიშები.
2. **შეიკითხეთ UAID-ის რეესტრებში.** Torii ასახავს
   `GET /v1/space-directory/uaids/{uaid}`, რომელიც აბრუნებს მონაცემთა სივრცის კავშირებს
   და მანიფესტის მეტამონაცემები, რომელსაც Space Directory მასპინძელი რჩება (იხ
   `docs/space-directory.md` §3 დატვირთვის ნიმუშებისთვის).
3. **მიიღეთ იგი დეტერმინისტულად.** ახალი UAID-ების ოფლაინ ჩატვირთვისას, ჰეშ
   კანონიკური მონაწილე დათესეს Blake2b-256-ით და დააფიქსირეს შედეგი
   `uaid:`. ქვემოთ მოცემული ფრაგმენტი ასახავს დამხმარეს, რომელიც დოკუმენტირებულია
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```ყოველთვის შეინახეთ ლიტერალი მცირე ასოებით და დაარეგულირეთ სივრცე ჰეშირების წინ.
CLI დამხმარეები, როგორიცაა `iroha app space-directory manifest scaffold` და Android
`UaidLiteral` პარსერი იყენებს იგივე მორთვის წესებს, რათა მმართველობის მიმოხილვამ შეძლოს
გადაამოწმეთ მნიშვნელობები ad hoc სკრიპტების გარეშე.

## 3. UAID ჰოლდინგისა და მანიფესტების შემოწმება

დეტერმინისტული პორტფელის აგრეგატორი `iroha_core::nexus::portfolio`-ში
ასახავს ყველა აქტივს/მონაცემთა სივრცის წყვილს, რომელიც მიმართავს UAID-ს. ოპერატორები და SDK-ები
შეუძლია მონაცემების მოხმარება შემდეგი ზედაპირების მეშვეობით:

| ზედაპირი | გამოყენება |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | აბრუნებს მონაცემთა სივრცეს → აქტივს → ბალანსის შეჯამებებს; აღწერილია `docs/source/torii/portfolio_api.md`-ში. |
| `GET /v1/space-directory/uaids/{uaid}` | ჩამოთვლის მონაცემთა სივრცის ID-ებს + ანგარიშების ლიტერალებს, რომლებიც დაკავშირებულია UAID-თან. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | გთავაზობთ სრულ `AssetPermissionManifest` ისტორიას აუდიტებისთვის. |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI მალსახმობი, რომელიც ახვევს საკინძების ბოლო წერტილს და სურვილისამებრ წერს JSON დისკზე (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | იღებს მანიფესტის JSON პაკეტს მტკიცებულებების პაკეტებისთვის. |

CLI სესიის მაგალითი (Torii URL კონფიგურირებულია `torii_api_url`-ის მეშვეობით `iroha.json`-ში):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

შეინახეთ JSON სნეპშოტები მიმოხილვის დროს გამოყენებული მანიფესტის ჰეშის გვერდით; The
Space Directory დამკვირვებელი აღადგენს `uaid_dataspaces` რუკას, როდესაც ეს გამოჩნდება
გაააქტიურეთ, იწურება ან გააუქმეთ, ასე რომ, ეს კადრები დასამტკიცებლად ყველაზე სწრაფი გზაა
რა კავშირები იყო აქტიური მოცემულ ეპოქაში.## 4. გამოქვეყნების უნარი ვლინდება მტკიცებულებებით

გამოიყენეთ ქვემოთ მოცემული CLI ნაკადი, როდესაც ახალი შემწეობა გამოვა. ყოველი ნაბიჯი უნდა
მიწის ნაკვეთი მტკიცებულებათა პაკეტში, რომელიც ჩაწერილია მმართველობის ხელმოწერისთვის.

1. **დაშიფვრეთ მანიფესტი JSON**, რათა მიმომხილველებმა ადრე დაინახონ დეტერმინისტული ჰეში
   წარდგენა:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **გამოაქვეყნეთ შემწეობა** Norito დატვირთვის (`--manifest`) გამოყენებით ან
   JSON აღწერა (`--manifest-json`). ჩაწერეთ Torii/CLI ქვითარი პლუს
   `PublishSpaceDirectoryManifest` ინსტრუქციის ჰეში:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SapceDirectoryEvent მტკიცებულებების გადაღება.** გამოწერა
   `SpaceDirectoryEvent::ManifestActivated` და მოიცავს ღონისძიების დატვირთვას
   პაკეტი, რათა აუდიტორებმა დაადასტურონ, როდის მოხდა ცვლილება.

4. ** შექმენით აუდიტის ნაკრები ** აკავშირებს manifest-ს მის მონაცემთა სივრცის პროფილთან და
   ტელემეტრიული კაკვები:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **შეამოწმეთ შეკვრა Torii** (`bindings fetch` და `manifests fetch`) და
   დაარქივეთ ეს JSON ფაილები ზემოთ ჰეშით + პაკეტით.

მტკიცებულებების ჩამონათვალი:

- [ ] მანიფესტის ჰეში (`*.manifest.hash`) ხელმოწერილია ცვლილების დამმტკიცებლის მიერ.
- [ ] CLI/Torii ქვითარი გამოქვეყნების ზარისთვის (stdout ან `--json-out` არტეფაქტი).
- [ ] `SpaceDirectoryEvent` დატვირთვის დამადასტურებელი გააქტიურება.
- [ ] აუდიტის ნაკრების დირექტორია მონაცემთა სივრცის პროფილით, კაკვებით და მანიფესტის ასლით.
- [ ] Bindings + მანიფესტის სნეპშოტები მოტანილია Torii პოსტ-აქტივაციისგან.ეს ასახავს `docs/space-directory.md` §3.2 მოთხოვნებს SDK-ის მიცემისას
მფლობელებს ერთი გვერდი უნდა მიუთითონ გამოშვების მიმოხილვის დროს.

## 5. მარეგულირებელი/რეგიონული მანიფესტის შაბლონები

გამოიყენეთ in-repo მოწყობილობები, როგორც საწყისი წერტილები, როდესაც გამოვლინდება ხელოსნობის შესაძლებლობები
მარეგულირებელი ან რეგიონალური ზედამხედველებისთვის. ისინი აჩვენებენ, თუ როგორ უნდა მოხდეს დაშვების/უარის ფარგლები
წესები და აუხსენით პოლიტიკის შენიშვნებს, რომლებსაც მიმომხილველები მოელიან.

| მოწყობილობა | დანიშნულება | მაჩვენებლები |
|---------|--------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB აუდიტის არხი. | მხოლოდ წაკითხვის შეღავათები `compliance.audit::{stream_reports, request_snapshot}`-ისთვის საცალო გადარიცხვებზე უარყოფით-მოგებით, რათა მარეგულირებელი UAID-ები იყოს პასიური. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA ზედამხედველობის ჩიხი. | ამატებს შეზღუდული `cbdc.supervision.issue_stop_order` შემწეობას (PerDay ფანჯარა + `max_amount`) და აშკარა უარყოფას `force_liquidation`-ზე ორმაგი კონტროლის განსახორციელებლად. |

ამ მოწყობილობების კლონირებისას განაახლეთ:

1. `uaid` და `dataspace` ID, რომელიც ემთხვევა მონაწილესა და ზოლს, რომელსაც ჩართავთ.
2. `activation_epoch`/`expiry_epoch` ფანჯრები მართვის განრიგის მიხედვით.
3. `notes` ველები მარეგულირებლის პოლიტიკის მითითებით (MiCA სტატია, JFSA
   წრიული და ა.შ.).
4. დამხმარე ფანჯრები (`PerSlot`, `PerMinute`, `PerDay`) და სურვილისამებრ
   `max_amount` იფარება, ასე რომ SDK-ები ახორციელებენ იგივე ლიმიტებს, როგორც მასპინძელი.

## 6. მიგრაციის შენიშვნები SDK მომხმარებლებისთვისარსებული SDK ინტეგრაციები, რომლებზეც მითითებულია თითო დომენის ანგარიშის ID, უნდა გადავიდეს
ზემოთ აღწერილი UAID-ზე ორიენტირებული ზედაპირები. გამოიყენეთ ეს სია განახლებების დროს:

  ანგარიშის ID. Rust/JS/Swift/Android-ისთვის ეს ნიშნავს განახლებას უახლესზე
  სამუშაო სივრცის ყუთები ან Norito საკინძების რეგენერაცია.
- **API ზარები:** შეცვალეთ დომენის ფარგლების პორტფოლიო მოთხოვნებით
  `GET /v1/accounts/{uaid}/portfolio` და მანიფესტის/დაკავშირების ბოლო წერტილები.
  `GET /v1/accounts/{uaid}/portfolio` იღებს არასავალდებულო `asset_id` მოთხოვნას
  პარამეტრი, როდესაც საფულეებს სჭირდებათ მხოლოდ ერთი აქტივის მაგალითი. კლიენტების დამხმარეები ასეთი
  როგორც `ToriiClient.getUaidPortfolio` (JS) და Android
  `SpaceDirectoryClient` უკვე ახვევს ამ მარშრუტებს; უპირატესობა მიანიჭეთ მათ შეკვეთილზე
  HTTP კოდი.
- **ქეშირება და ტელემეტრია:** ქეში ჩანაწერები UAID-ით + მონაცემთა სივრცე ნედლეულის ნაცვლად
  ანგარიშის იდენტიფიკატორი და ასხივებენ ტელემეტრიას, რომელიც აჩვენებს UAID-ს პირდაპირი მნიშვნელობით, ასე რომ ოპერაციებს შეუძლიათ
  დაალაგეთ ჟურნალები Space Directory-ის მტკიცებულებებით.
- **შეცდომის დამუშავება:** ახალი საბოლოო წერტილები აბრუნებს UAID-ის გარჩევის მკაცრ შეცდომებს
  დოკუმენტირებულია `docs/source/torii/portfolio_api.md`-ში; ამოიღეთ ეს კოდები
  სიტყვასიტყვით, რათა დამხმარე გუნდებმა შეძლონ პრობლემების გადაჭრა რეპრო ნაბიჯების გარეშე.
- ** ტესტირება: ** გადაიტანეთ ზემოთ ნახსენები მოწყობილობები (პლუს თქვენი საკუთარი UAID მანიფესტები)
  SDK ტესტის კომპლექტებში Norito ორმხრივი მგზავრობისა და მანიფესტი შეფასებების დასამტკიცებლად
  ემთხვევა მასპინძლის განხორციელებას.

## 7. ლიტერატურა- `docs/space-directory.md` — ოპერატორის სათამაშო წიგნი სასიცოცხლო ციკლის უფრო ღრმა დეტალებით.
- `docs/source/torii/portfolio_api.md` — REST სქემა UAID პორტფოლიოსთვის და
  მანიფესტი საბოლოო წერტილები.
- `crates/iroha_cli/src/space_directory.rs` — CLI განხორციელება მითითებულია
  ამ სახელმძღვანელოს.
- `fixtures/space_directory/capability/*.manifest.json` — რეგულატორი, საცალო და
  CBDC მანიფესტის შაბლონები მზად არის კლონირებისთვის.