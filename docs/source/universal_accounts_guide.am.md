---
lang: am
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f972f8f82b7f4e89c1d48b0dbbc6eb5b73303e2fab0f580ab21e63990ba03af8
source_last_modified: "2026-03-27T19:05:17.617064+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ሁለንተናዊ መለያ መመሪያ

ይህ መመሪያ የ UAID (ሁለንተናዊ መለያ መታወቂያ) ልቀት መስፈርቶችን ያስወግዳል
የ Nexus የመንገድ ካርታ እና ጥቅል ወደ ኦፕሬተር + ኤስዲኬ ያተኮረ የእግር ጉዞ።
የ UAID አመጣጥን፣ ፖርትፎሊዮ/ገላጭ ፍተሻን፣ የተቆጣጣሪ አብነቶችን፣
እና ከእያንዳንዱ `iroha መተግበሪያ ቦታ-ማውጫ ዝርዝር መግለጫ ጋር አብሮ መሆን ያለበት ማስረጃ
print` run (roadmap reference: `roadmap.md:2209`)።

## 1. የ UAID ፈጣን ማጣቀሻ- UAIDs `uaid:<hex>` ቀጥተኛ ቃላት ሲሆኑ `<hex>` የ Blake2b-256 መፈጨት
  LSB ወደ `1` ተቀናብሯል። ቀኖናዊው ዓይነት ይኖራል
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- የመለያ መዝገቦች (`Account` እና `AccountDetails`) አሁን አማራጭ `uaid` ይይዛሉ
  መስክ ስለዚህ አፕሊኬሽኖች ያለ ሀሺንግ መለያውን መማር ይችላሉ።
- የተደበቀ ተግባር ለዪ ፖሊሲዎች የዘፈቀደ መደበኛ ግብዓቶችን ማሰር ይችላሉ።
  (ስልክ ቁጥሮች፣ ኢሜይሎች፣ የመለያ ቁጥሮች፣ የአጋር ሕብረቁምፊዎች) ወደ `opaque:` መታወቂያዎች
  በUAID የስም ቦታ። በሰንሰለት ላይ ያሉት ቁርጥራጮች `IdentifierPolicy`፣
  `IdentifierClaimRecord`፣ እና `opaque_id -> uaid` ኢንዴክስ።
- የስፔስ ማውጫ እያንዳንዱን UAID የሚያገናኝ `World::uaid_dataspaces` ካርታ ይይዛል
  በንቁ አንጸባራቂዎች ለተጠቀሱት የውሂብ ቦታ መለያዎች። Torii ያንን እንደገና ይጠቀማል
  ካርታ ለ`/portfolio` እና `/uaids/*` APIs።
- `POST /v1/accounts/onboard` ነባሪ የስፔስ ማውጫ መግለጫ ያትማል
  የአለምአቀፍ የመረጃ ቦታ ምንም በማይኖርበት ጊዜ፣ ስለዚህ UAID ወዲያውኑ ይታሰራል።
  ተሳፋሪ ባለስልጣናት `CanPublishSpaceDirectoryManifest{dataspace=0}` መያዝ አለባቸው።
- ሁሉም ኤስዲኬዎች የ UAID ቃል በቃል እንዲገልጹ ረዳቶችን ያጋልጣሉ (ለምሳሌ፣
  `UaidLiteral` በአንድሮይድ ኤስዲኬ)። ረዳቶቹ ጥሬ 64-ሄክስ መፍጨት ይቀበላሉ
  (LSB=1) ወይም `uaid:<hex>` ቀጥታ ቃላት እና ተመሳሳዩን Norito ኮዴኮችን እንደገና ተጠቀም
  መፍጨት በቋንቋዎች መንሸራተት አይችልም።

## 1.1 የተደበቁ መለያ መመሪያዎች

ዩአይዲዎች አሁን ለሁለተኛው የማንነት ንብርብር መልህቅ ናቸው፡-- ዓለም አቀፍ `IdentifierPolicyId` (`<kind>#<business_rule>`) ይገልጻል
  የስም ቦታ፣ የህዝብ ቁርጠኝነት ሜታዳታ፣ ፈቺ የማረጋገጫ ቁልፍ እና የ
  ቀኖናዊ ግቤት መደበኛ ሁነታ (`Exact`፣ `LowercaseTrimmed`፣
  `PhoneE164`፣ `EmailAddress`፣ ወይም `AccountNumber`)።
- የይገባኛል ጥያቄ አንድን `opaque:` መለያን ከአንድ UAID እና አንድ ጋር ያገናኛል
  ቀኖናዊ `AccountId` በዚያ ፖሊሲ መሠረት፣ ነገር ግን ሰንሰለቱ የሚቀበለው
  የይገባኛል ጥያቄ ከተፈረመ `IdentifierResolutionReceipt` ጋር ሲታጀብ።
- ጥራት የ `resolve -> transfer` ፍሰት ይቀራል። Torii ግልጽ ያልሆነውን ይፈታል።
  ቀኖናዊውን `AccountId` በመያዝ ይመልሳል; ዝውውሮች አሁንም ዒላማው ናቸው
  ቀኖናዊ መለያ፣ በቀጥታ `uaid:` ወይም `opaque:` አይደለም።
- ፖሊሲዎች አሁን የBFV ግቤት-ምስጠራ መለኪያዎችን በዚህ በኩል ማተም ይችላሉ።
  `PolicyCommitment.public_parameters`. ባሉበት ጊዜ፣ Torii ያስተዋውቃቸዋል።
  `GET /v1/identifier-policies`፣ እና ደንበኞች BFV-የተጠቀለለ ግቤት ማስገባት ይችላሉ።
  ግልጽ በሆነ ጽሑፍ ፋንታ. በፕሮግራም የተቀመጡ ፖሊሲዎች የBFV መለኪያዎችን በ ሀ
  ቀኖናዊ `BfvProgrammedPublicParameters` ቅርቅብ እሱ ደግሞ የሚያሳትመው
  የህዝብ `ram_fhe_profile`; የቆዩ ጥሬ BFV ጭነቶች በዚያ ላይ ተሻሽለዋል።
  ቁርጠኝነት እንደገና ሲገነባ ቀኖናዊ ጥቅል።
- የመለያ መንገዶች የሚሄዱት በተመሳሳዩ Torii የመድረሻ ማስመሰያ እና የፍጥነት ገደብ ነው።
  እንደ ሌሎች መተግበሪያ የሚመለከቱ የመጨረሻ ነጥቦችን ይፈትሻል። እነሱ በተለመደው አካባቢ ማለፊያ አይደሉም
  የኤፒአይ ፖሊሲ።

## 1.2 ቃላት

የስያሜ ክፍፍል ሆን ተብሎ የተደረገ ነው፡-- `ram_lfe` ውጫዊ የተደበቀ ተግባር ረቂቅ ነው። ፖሊሲን ይሸፍናል።
  ምዝገባ፣ ቃል ኪዳኖች፣ የህዝብ ሜታዳታ፣ የአፈጻጸም ደረሰኞች እና
  የማረጋገጫ ሁነታ.
- `BFV` የ Brakerski/Fan-Vercauteren ሆሞሞርፊክ ምስጠራ ዘዴ ነው
  አንዳንድ `ram_lfe` የተመሰጠረ ግቤትን ለመገምገም የጀርባ ደጋፊዎች።
- `ram_fhe_profile` BFV-ተኮር ሜታዳታ ነው እንጂ ለጠቅላላው ሁለተኛ ስም አይደለም
  ባህሪ. በፕሮግራም የተያዘውን የ BFV ማስፈጸሚያ ማሽን የኪስ ቦርሳ እና
  ፖሊሲ በፕሮግራም የተያዘለትን የኋላ ክፍል ሲጠቀም አረጋጋጮች ማነጣጠር አለባቸው።

በተጨባጭ ሁኔታ፡-

- `RamLfeProgramPolicy` እና `RamLfeExecutionReceipt` LFE-ንብርብር ዓይነቶች ናቸው።
- `BfvParameters`፣ `BfvCiphertext`፣ `BfvProgrammedPublicParameters`፣ እና
  `BfvRamProgramProfile` FHE-ንብርብር ዓይነቶች ናቸው።
- `HiddenRamFheProgram` እና `HiddenRamFheInstruction` የውስጥ ስሞች ናቸው
  የተደበቀው የ BFV ፕሮግራም በፕሮግራም በተያዘው የኋላ ክፍል የተተገበረ። ላይ ይቆያሉ።
  የFHE ጎን ምክንያቱም ኢንክሪፕት የተደረገውን የማስፈጸሚያ ዘዴን ይገልጻሉ።
  የውጪው ፖሊሲ ወይም ደረሰኝ ረቂቅ.

## 1.3 የመለያ መታወቂያ ከተለዋጭ ስሞች ጋር

ሁለንተናዊ-መለያ መልቀቅ የቀኖናዊ መለያ መለያ ሞዴልን አይለውጠውም፡-- `AccountId` ቀኖናዊ፣ ጎራ የለሽ መለያ ርዕሰ ጉዳይ ሆኖ ይቆያል።
- `ScopedAccountId { account, domain }` ለዕይታዎች ግልጽ የሆነ የጎራ አውድ ነው።
  የጎራ አገናኝን የሚያመለክቱ ምዝገባዎች። ሁለተኛ ቀኖናዊ አይደለም።
  ማንነት.
- የኤስኤንኤስ/የመለያ ተለዋጭ ስሞች በዚህ ርዕሰ ጉዳይ ላይ የተለያዩ ማሰሪያዎች ናቸው። ሀ
  እንደ `merchant@hbl.sbp` እና ዳታስፔስ-ስር ተለዋጭ ስም ያሉ ለጎራ ብቃት ያላቸው ተለዋጭ ስሞች
  እንደ `merchant@sbp` ያሉ ሁለቱም ወደ ተመሳሳይ ቀኖናዊ `AccountId` መፍታት ይችላሉ።
- `linked_domains` በተከማቸ የመለያ መዝገቦች ላይ ያለው ሁኔታ የተገኘው ከ
  መለያ-ጎራ ኢንዴክሶች. ለዛ በአሁኑ ጊዜ ተጨባጭ የሆኑ አገናኞችን ይገልጻል
  ርዕሰ ጉዳይ; የቀኖና መለያ አካል አይደለም።

ለኦፕሬተሮች፣ ኤስዲኬዎች እና ለሙከራዎች የትግበራ ህግ፡ ከቀኖናዊው ይጀምሩ
`AccountId`፣ በመቀጠል ተለዋጭ ስም ሊዝ፣ የውሂብ ቦታ/የጎራ ፈቃዶችን እና ግልጽነትን ያክሉ
የጎራ ማገናኛዎች በተናጠል. የሐሰት ጎራ-ወሰን ቀኖናዊ አያዋህዱ
መለያ ስም ወይም መንገድ የጎራ ክፍል ስላለው ብቻ።

የአሁኑ Torii መንገዶች፡| መስመር | ዓላማ |
|-------|--------|
| `GET /v1/ram-lfe/program-policies` | የነቃ እና የቦዘኑ RAM-LFE ፕሮግራም ፖሊሲዎች እና የእነርሱ ይፋዊ አፈጻጸም ዲበ ዳታ፣ አማራጭ BFV `input_encryption` መለኪያዎችን እና በፕሮግራም የተደገፈ `ram_fhe_profile` ይዘረዝራል። |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | በትክክል ከ`{ input_hex }` ወይም `{ encrypted_input }` አንዱን ተቀብሎ አገር አልባውን `RamLfeExecutionReceipt` እና `{ output_hex, output_hash, receipt_hash }`ን ለተመረጠው ፕሮግራም ይመልሳል። የአሁኑ Torii የአሂድ ጊዜ ደረሰኞችን በፕሮግራም ለተያዘው የBFV ጀርባ ይሰጣል። |
| `POST /v1/ram-lfe/receipts/verify` | ያለ ሀገር `RamLfeExecutionReceipt` በታተመው በሰንሰለት ፕሮግራም ፖሊሲ ላይ ያፀድቃል እና እንደአማራጭ በጠዋቂ ያቀረበው `output_hex` ከደረሰኙ `output_hash` ጋር እንደሚዛመድ ያረጋግጣል። |
| `GET /v1/identifier-policies` | የነቃ እና የቦዘኑ ድብቅ ተግባር የፖሊሲ የስም ቦታዎችን እና ይፋዊ ዲበ ዳታዎቻቸውን ይዘረዝራል፣ አማራጭ BFV `input_encryption` ግቤቶች፣ አስፈላጊ የሆነውን የ`normalization` ሁነታ ለተመሰጠረ ደንበኛ-ጎን ግብዓት እና `ram_fhe_profile` በፕሮግራም ለተያዙ የBFV ፖሊሲዎች። |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | በትክክል ከ `{ input }` ወይም `{ encrypted_input }` አንዱን ይቀበላል። Plaintext `input` መደበኛ አገልጋይ-ጎን ነው; BFV `encrypted_input` በታተመው የመመሪያ ሁኔታ መሰረት አስቀድሞ መደበኛ መሆን አለበት። የመጨረሻው ነጥብ የ `opaque:` መያዣን ያመጣል እና `ClaimIdentifier` በጥሬው `signature_payload_hex` እና የተተነተነውን `signature_payload` ጨምሮ በሰንሰለት ላይ ማስገባት የሚችለውን የተፈረመ ደረሰኝ ይመልሳል። || `POST /v1/identifiers/resolve` | በትክክል ከ `{ input }` ወይም `{ encrypted_input }` አንዱን ይቀበላል። Plaintext `input` መደበኛ አገልጋይ-ጎን ነው; BFV `encrypted_input` በታተመው የመመሪያ ሁኔታ መሰረት አስቀድሞ መደበኛ መሆን አለበት። የመጨረሻ ነጥቡ ንቁ የይገባኛል ጥያቄ በሚኖርበት ጊዜ መለያውን ወደ `{ opaque_id, receipt_hash, uaid, account_id, signature }` ይፈታዋል እና እንዲሁም ቀኖናዊ የተፈረመ ክፍያ እንደ `{ signature_payload_hex, signature_payload }` ይመልሳል። |
| `GET /v1/identifiers/receipts/{receipt_hash}` | ኦፕሬተሮች እና ኤስዲኬዎች የባለቤትነት መብት ይገባኛል ጥያቄ ኦዲት እንዲያደርጉ ወይም ሙሉ መለያ መረጃ ጠቋሚውን ሳይቃኙ የድጋሚ አጫውት/የማይዛመድ አለመሳካቶችን ለመመርመር የቀጠለውን `IdentifierClaimRecord` ከተለየ ደረሰኝ ሃሽ ጋር ተያይዟል። |

የTorii በሂደት ላይ ያለ የማስፈጸሚያ አሂድ ጊዜ በስር ተዋቅሯል።
`torii.ram_lfe.programs[*]`፣ በ`program_id` የተከፈተ። መለያው መንገዶች አሁን
ከተለየ `identifier_resolver` ይልቅ ያንን ተመሳሳይ RAM-LFE አሂድ ጊዜን እንደገና ይጠቀሙ
አዋቅር ወለል.

የአሁኑ የኤስዲኬ ድጋፍ፡-- `normalizeIdentifierInput(value, normalization)` ዝገቱ ጋር ይዛመዳል
  ቀኖናዎች ለ `exact`፣ `lowercase_trimmed`፣ `phone_e164`፣
  `email_address`፣ እና `account_number`።
- `ToriiClient.listIdentifierPolicies()` የፖሊሲ ሜታዳታ ይዘረዝራል፣ BFVን ጨምሮ
  የግቤት-ምስጠራ ሜታዳታ ፖሊሲው ሲያትመው እና ዲኮድ የተደረገ
  BFV መለኪያ ነገር በ`input_encryption_public_parameters_decoded` በኩል።
  የተነደፉ ፖሊሲዎች ዲኮድ የተደረገውን `ram_fhe_profile` ያጋልጣሉ። ያ መስክ ነው።
  ሆን ተብሎ BFV-scoped፡ የኪስ ቦርሳዎች የሚጠበቀውን መዝገብ እንዲያረጋግጡ ያስችላቸዋል
  ቆጠራ፣ የሌይን ቆጠራ፣ ቀኖናዊነት ሁነታ እና አነስተኛ የምስጥር ጽሑፍ ሞጁሎች ለ
  የደንበኛ-ጎን ግብዓት ከማመስጠር በፊት ፕሮግራም የተደረገው የFHE ጀርባ።
- `getIdentifierBfvPublicParameters(policy)` እና
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` እገዛ
  የጄኤስ ደዋዮች የታተመ BFV ሜታዳታ ይበላሉ እና ፖሊሲን የሚያውቅ ጥያቄን ይገንቡ
  የፖሊሲ-መታወቂያ እና የመደበኛነት ደንቦችን ሳይተገበሩ አካላት.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` እና
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` አሁን ፍቀድ
  የጄኤስ ቦርሳዎች ሙሉውን BFV Norito የምስጢር ጽሁፍ ፖስታ በአገር ውስጥ ከ
  ቀድሞ የተሰራ የምስጢር ጽሑፍ ሄክስ ከመርከብ ይልቅ የታተሙ የመመሪያ መለኪያዎች።
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  የተደበቀ መለያን ፈትቶ የተፈረመውን የክፍያ ደረሰኝ ይመልሳል፣
  `receipt_hash`፣ `signature_payload_hex`፣ እና ጨምሮ
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId፣ {policyId፣ input |
  ኢንክሪፕትድ ግቤት})` issues the signed receipt needed by `ClaimIdentifier`።
- `verifyIdentifierResolutionReceipt(receipt, policy)` የተመለሰውን ያረጋግጣል
  በደንበኛው በኩል ባለው የፖሊሲ መፍቻ ቁልፍ ላይ ደረሰኝ እና`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` ያመጣል
  ለበኋላ የኦዲት/የማረሚያ ፍሰቶች የይገባኛል ጥያቄ መዝገብ።
- `IrohaSwift.ToriiClient` አሁን `listIdentifierPolicies()` ያጋልጣል፣
  `resolveIdentifier(policyId:input:encryptedInputHex:)`፣
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`፣
  እና `getIdentifierClaimByReceiptHash(_)`, በተጨማሪም
  `ToriiIdentifierNormalization` ለተመሳሳይ ስልክ/ኢሜል/መለያ ቁጥር
  ቀኖናዊነት ሁነታዎች.
- `ToriiIdentifierLookupRequest` እና የ
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` ረዳቶች የተተየበው የስዊፍት ጥያቄ ወለል ለ
  መፍታት እና የይገባኛል ጥያቄ ደረሰኝ፣ እና የስዊፍት ፖሊሲዎች አሁን BFVን ማግኘት ይችላሉ።
  ምስጢራዊ ጽሑፍ በአገር ውስጥ በ`encryptInput(...)` / `encryptedRequest(input:...)`።
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` ያረጋግጣል
  የከፍተኛ ደረጃ ደረሰኝ መስኮች ከተፈረመው የክፍያ ጭነት ጋር ይዛመዳሉ እና ያረጋግጣል
  ከማቅረቡ በፊት ፈቺ ፊርማ ደንበኛ-ጎን.
- `HttpClientTransport` በአንድሮይድ ኤስዲኬ አሁን አጋልጧል
  `listIdentifierPolicies()`፣ `መፍትሄ መለያ(ፖሊሲአይድ፣ ግብዓት፣
  የተመሰጠረInputHex)`, `issueIdentifierClaimReceipt(accountId፣policyId፣
  ግብዓት፣ የተመሰጠረInputHex)`, and `getIdentifierClaimByReceiptHash(...)`፣
  በተጨማሪም `IdentifierNormalization` ለተመሳሳይ የቀኖና ደንቦች.
- `IdentifierResolveRequest` እና የ
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` ረዳቶች የተተየበው የአንድሮይድ ጥያቄ ወለል ይሰጣሉ፣
  `IdentifierPolicySummary.encryptInput(...)` / ሳለ
  `.encryptedRequestFromInput(...)` የBFV የምስጢር ጽሁፍ ፖስታ ነው የመጣው
  በአካባቢው ከታተሙ የፖሊሲ መለኪያዎች.
  `IdentifierResolutionReceipt.verifySignature(policy)` የተመለሰውን ያረጋግጣል
  ፈቺ ፊርማ ደንበኛ-ጎን.

የአሁኑ መመሪያ ስብስብ፡-- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (ደረሰኝ-የታሰረ፣ ጥሬ `opaque_id` የይገባኛል ጥያቄዎች ውድቅ ናቸው)
- `RevokeIdentifier`

አሁን በ `iroha_crypto::ram_lfe` ውስጥ ሶስት የጀርባ ጫፎች አሉ፡

- ታሪካዊ ቁርጠኝነት-የተሳሰረ `HKDF-SHA3-512` PRF, እና
- በBFV የሚደገፍ ሚስጥራዊ አፊን ገምጋሚ BFV-የተመሰጠረ መለያን የሚበላ
  ቦታዎች በቀጥታ. `iroha_crypto` በነባሪው ሲገነባ
  `bfv-accel` ባህሪ፣ BFV ቀለበት ማባዛት ትክክለኛ መወሰኛ ይጠቀማል
  CRT-NTT ከውስጥ ጀርባ; ያንን ባህሪ ማሰናከል ወደ
  ተመሳሳይ ውጤቶች ያሉት scalar የትምህርት መጽሐፍ መንገድ፣ እና
- በBFV የሚደገፍ ሚስጥራዊ ፕሮግራም ያለው ገምጋሚ በመመሪያ የሚመራ
  በተመሰጠሩ መዝገቦች እና በምስጥር ጽሑፍ ማህደረ ትውስታ ላይ የ RAM-style አፈፃፀም ዱካ
  ግልጽ ያልሆነ መለያ እና ደረሰኝ ሃሽ ከማውጣቱ በፊት መንገዶች። ፕሮግራሙን አዘጋጀ
  backend አሁን ከአፊን መንገድ የበለጠ ጠንካራ BFV ሞጁል ወለል ይፈልጋል
  ህዝባዊ መመዘኛዎቹ የሚታተሙት ቀኖናዊ ጥቅል ውስጥ ሲሆን ይህም ያካትታል
  RAM-FHE የማስፈጸሚያ መገለጫ በኪስ ቦርሳ እና አረጋጋጮች የሚበላ።

እዚህ BFV ማለት የብሬከርስኪ/ፋን-Vercauteren FHE እቅድ በ ውስጥ የተተገበረ ማለት ነው።
`crates/iroha_crypto/src/fhe_bfv.rs`. ኢንክሪፕት የተደረገው የማስፈጸሚያ ዘዴ ነው።
በአፊን እና በፕሮግራም የተደገፉ ጀርባዎች የሚጠቀሙት እንጂ የውጪው የተደበቀ ስም አይደለም።
የተግባር ረቂቅ.Torii በፖሊሲው ቁርጠኝነት የታተመውን የጀርባ ሽፋን ይጠቀማል። BFV ሲመለስ
ንቁ ነው፣ ግልጽ ያልሆኑ ጥያቄዎች መደበኛ ይሆናሉ ከዚያም በፊት የተመሰጠረ የአገልጋይ ጎን
ግምገማ. BFV `encrypted_input` የአፊን ጀርባ ጥያቄዎች ይገመገማሉ
በቀጥታ እና አስቀድሞ መደበኛ ደንበኛ-ጎን መሆን አለበት; በፕሮግራም የተያዘው ጀርባ
ኢንክሪፕት የተደረገ ግቤትን ወደ ፈቺው መወሰኛ BFV መልሶ ይመልሳል
ሚስጥራዊውን RAM ፕሮግራም ከመተግበሩ በፊት ኤንቨሎፕ ስለዚህ ደረሰኝ ሃሽ ይቀራል
በትርጓሜ አቻ የምስጥር ጽሑፎች ላይ የተረጋጋ።

## 2. ዩኤአይዲዎችን ማግኘት እና ማረጋገጥ

UAID ለማግኘት ሶስት የሚደገፉ መንገዶች አሉ፡-

1. **ከዓለም ግዛት ወይም ከኤስዲኬ ሞዴሎች አንብበው።** ማንኛውም `Account`/`AccountDetails`
   በTorii በኩል የተጠየቀው ክፍያ አሁን የ `uaid` መስክ ሲሞላው ተሞልቷል።
   ተሳታፊው ወደ ሁለንተናዊ መለያዎች መርጧል።
2. ** የ UAID መዝገቦችን ይጠይቁ።** Torii ያጋልጣል።
   `GET /v1/space-directory/uaids/{uaid}` ይህም የውሂብ ቦታ ማሰሪያዎችን ይመልሳል
   እና የSpace Directory አስተናጋጁ እንደቀጠለ ሜታዳታ ያሳያል (ይመልከቱ
   `docs/space-directory.md` §3 ለክፍያ ናሙናዎች)።
3. **በመወሰን ያውጡት።** አዳዲስ UAIDዎችን ከመስመር ውጭ ሲያስነሱ ሃሽ
   ቀኖናዊው ተሳታፊ ዘር ከ Blake2b-256 ጋር እና ውጤቱን ቅድመ ቅጥያ ያድርጉ
   `uaid:`. ከታች ያለው ቅንጣቢ በሰነድ የተመለከተውን ረዳት ያሳያል
   `docs/space-directory.md` §3.3፡

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```ሁልጊዜ ቃል በቃል በትንሽ ፊደላት ያከማቹ እና ከመጥለፍዎ በፊት ነጭ ቦታን መደበኛ ያድርጉት።
እንደ `iroha app space-directory manifest scaffold` እና አንድሮይድ ያሉ የCLI ረዳቶች
`UaidLiteral` ተንታኝ የአስተዳደር ግምገማዎች እንዲችሉ ተመሳሳይ የመቁረጥ ህጎችን ይተገበራሉ
ያለማስታወቂያ ስክሪፕቶች እሴቶችን ፈትሽ።

## 3. የ UAID ይዞታዎችን እና መግለጫዎችን መመርመር

በ`iroha_core::nexus::portfolio` ውስጥ ያለው የሚወስነው ፖርትፎሊዮ ሰብሳቢ
UAIDን የሚያጣቅሱትን እያንዳንዱን የንብረት/የዳታ ቦታ ጥንዶችን ይዘረጋል። ኦፕሬተሮች እና ኤስዲኬዎች
ውሂቡን በሚከተሉት ወለሎች መጠቀም ይችላል

| ወለል | አጠቃቀም |
|--------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | የውሂብ ቦታ → ንብረት → ቀሪ ማጠቃለያዎችን ይመልሳል; በ `docs/source/torii/portfolio_api.md` ውስጥ ተገልጿል. |
| `GET /v1/space-directory/uaids/{uaid}` | ከUAID ጋር የተሳሰሩ የውሂብ ቦታ መታወቂያዎችን + የመለያ ቃል በቃል ይዘረዝራል። |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | ለኦዲት ሙሉ የ`AssetPermissionManifest` ታሪክ ያቀርባል። |
| `iroha app space-directory bindings fetch --uaid <literal>` | የ CLI አቋራጭ የማሰሪያውን የመጨረሻ ነጥብ ያጠቃለለ እና እንደ አማራጭ JSON ወደ ዲስክ (`--json-out`) ይጽፋል። |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | ለማረጃ ጥቅሎች የሰነድ ሰነዱን JSON ጥቅል ያመጣል። |

ምሳሌ CLI ክፍለ ጊዜ (Torii URL በ `torii_api_url` በ `iroha.json` ተዋቅሯል)

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

በግምገማ ወቅት ጥቅም ላይ ከዋለው አንጸባራቂ ሃሽ ጎን የJSON ቅጽበተ-ፎቶዎችን ያከማቹ። የ
የስፔስ ማውጫ ተመልካች በሚገለጥበት ጊዜ የ`uaid_dataspaces` ካርታውን እንደገና ይገነባል።
ያግብሩ፣ ጊዜው ያበቃል ወይም ይሽሩ፣ ስለዚህ እነዚህ ቅጽበተ-ፎቶዎች ለማረጋገጥ ፈጣኑ መንገድ ናቸው።
በተወሰነ ዘመን ውስጥ ምን ማያያዣዎች ንቁ ነበሩ ።

## 4. የማተም ችሎታ ከማስረጃ ጋር ይገለጻል።

አዲስ አበል በሚለቀቅበት ጊዜ ሁሉ ከዚህ በታች ያለውን የCLI ፍሰት ይጠቀሙ። እያንዳንዱ እርምጃ መሆን አለበት
ለአስተዳደር መፈረም በተመዘገበው የማስረጃ ጥቅል ውስጥ መሬት።

1. **አንጸባራቂውን JSON** ገምግሞ ገምጋሚዎች ወሳኙን ሃሽ ከዚህ በፊት እንዲያዩ ያድርጉ
   ማስረከብ፡

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **በአይ18NT00000002X ክፍያ (`--manifest`) ወይም በመጠቀም አበል ያትሙ**
   የJSON መግለጫ (`--manifest-json`)። የTorii/CLI ደረሰኝ ይቅዱ
   የ `PublishSpaceDirectoryManifest` መመሪያ ሃሽ፡-

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **የSpaceDirectoryEvent ማስረጃን ይያዙ።** ይመዝገቡ
   `SpaceDirectoryEvent::ManifestActivated` እና የክስተት ክፍያ ጭነትን ያካትቱ
   ለውጡ ሲያርፍ ኦዲተሮች ማረጋገጥ እንዲችሉ ጥቅል።

4. **የኦዲት ቅርቅብ ይፍጠሩ** ማኒፌክተሩን ከመረጃ ቦታ መገለጫው ጋር በማያያዝ እና
   ቴሌሜትሪ መንጠቆዎች;

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. ** ማሰሪያዎችን በTorii** (`bindings fetch` እና `manifests fetch`) ያረጋግጡ እና
   እነዚያን የJSON ፋይሎች ከላይ ባለው hash + bundle በማህደር ያስቀምጡ።

የማስረጃ ማረጋገጫ ዝርዝር፡-

- [ ] አንጸባራቂ ሃሽ (`*.manifest.hash`) በለውጥ አጽዳቂ የተፈረመ።
- [ ] CLI/Torii ደረሰኝ ለህትመት ጥሪ (stdout ወይም `--json-out` artefact)።
- [ ] `SpaceDirectoryEvent` ክፍያ ማግበርን ያረጋግጣል።
- [ ] የኦዲት ጥቅል ማውጫ ከዳታ ቦታ መገለጫ፣ መንጠቆዎች እና አንጸባራቂ ቅጂ ጋር።
- [ ] ማያያዣዎች + አንጸባራቂ ቅጽበተ-ፎቶዎች ከ ​​Torii ድህረ ማግበር የተገኙ።ይህ ኤስዲኬ በሚሰጥበት ጊዜ በ`docs/space-directory.md` §3.2 ውስጥ ያሉትን መስፈርቶች ያንጸባርቃል
በመልቀቂያ ግምገማዎች ወቅት ለመጠቆም አንድ ገጽ ባለቤቶች።

## 5. ተቆጣጣሪ/ክልላዊ መግለጫ አብነቶች

የመስራት ችሎታ በሚገለጥበት ጊዜ የውስጠ-ግንባታ መሳሪያዎችን እንደ መነሻ ይጠቀሙ
ለተቆጣጣሪዎች ወይም የክልል ተቆጣጣሪዎች. እንዴት መፍቀድ/መከልከል እንደሚቻል ያሳያሉ
ደንቦች እና ገምጋሚዎች የሚጠብቁትን የፖሊሲ ማስታወሻዎች ያብራሩ.

| ቋሚ | ዓላማ | ዋና ዋና ዜናዎች |
|--------|--------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB የኦዲት ምግብ። | የ `compliance.audit::{stream_reports, request_snapshot}` ተነባቢ-ብቻ አበል በችርቻሮ ዝውውሮች ላይ ከካድ-አሸናፊዎች ጋር የቁጥጥር UAID ዎች ተገብሮ ለማቆየት። |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA የክትትል መስመር። | የ `cbdc.supervision.issue_stop_order` አበል (በቀን መስኮት + `max_amount`) እና በ`force_liquidation` ላይ ድርብ መቆጣጠሪያዎችን ለማስፈጸም ግልጽ የሆነ መካድ ይጨምራል። |

እነዚህን መገልገያዎች በሚዘጉበት ጊዜ ያዘምኑ፦

1. `uaid` እና `dataspace` መታወቂያዎች እርስዎ ከሚያነቁት ተሳታፊ እና መስመር ጋር የሚዛመዱ።
2. በአስተዳደር መርሃ ግብር ላይ በመመስረት `activation_epoch`/`expiry_epoch` መስኮቶች.
3. `notes` መስኮች ከተቆጣጣሪው የፖሊሲ ማጣቀሻዎች ጋር (MiCA article፣ JFSA)
   ክብ ወዘተ)።
4. የአበል መስኮቶች (`PerSlot`፣ `PerMinute`፣ `PerDay`) እና አማራጭ
   `max_amount` caps ስለዚህ ኤስዲኬዎች እንደ አስተናጋጁ ተመሳሳይ ገደቦችን ያስፈጽማሉ።

## 6. የስደት ማስታወሻዎች ለኤስዲኬ ተጠቃሚዎችየጎራ መለያ መታወቂያዎችን ያጣቀሱ የኤስዲኬ ውህደቶች ወደ መሰደድ አለባቸው
ከላይ የተገለጹት የ UAID ማዕከሎች። በማሻሻያዎች ጊዜ ይህንን የማረጋገጫ ዝርዝር ይጠቀሙ፡-

  የመለያ መታወቂያዎች. ለ Rust/JS/Swift/Android ይህ ማለት ወደ የቅርብ ጊዜው ማሻሻል ማለት ነው።
  የስራ ቦታ ሳጥኖች ወይም Norito ማሰሪያዎችን በማደስ ላይ።
- ** የኤፒአይ ጥሪዎች፡** በጎራ የተቀመጡ የፖርትፎሊዮ መጠይቆችን ይተኩ
  `GET /v1/accounts/{uaid}/portfolio` እና አንጸባራቂ/የማሰሪያው የመጨረሻ ነጥቦች።
  `GET /v1/accounts/{uaid}/portfolio` አማራጭ `asset_id` ጥያቄ ይቀበላል
  የኪስ ቦርሳዎች አንድ የንብረት ምሳሌ ብቻ ሲፈልጉ መለኪያ። የደንበኛ ረዳቶች እንደ
  እንደ `ToriiClient.getUaidPortfolio` (JS) እና አንድሮይድ
  `SpaceDirectoryClient` እነዚህን መንገዶች አስቀድሞ ጠቅልሎታል; ከመጥፎ ይመርጧቸው
  የኤችቲቲፒ ኮድ
- ** መሸጎጫ እና ቴሌሜትሪ፡** መሸጎጫ በ UAID + የውሂብ ቦታ ከጥሬ ይልቅ
  የመለያ መታወቂያዎች እና የ UAID ቃል በቃል ኦፕሬሽኖችን በማሳየት ቴሌሜትሪ ያመነጫሉ።
  ምዝግብ ማስታወሻዎችን ከ Space Directory ማስረጃ ጋር አሰልፍ።
** የስህተት አያያዝ:** አዲስ የመጨረሻ ነጥቦች ጥብቅ የ UAID የመተንተን ስህተቶችን ይመለሳሉ
  በ `docs/source/torii/portfolio_api.md` ውስጥ ተመዝግቧል; እነዚያን ኮዶች ወለል አድርገው
  ቃል በቃል ስለዚህ የድጋፍ ቡድኖች ጉዳዮችን ያለ ምንም እርምጃዎች መለየት ይችላሉ።
- ** ሙከራ: ** ከላይ የተጠቀሱትን እቃዎች (የእራስዎ የ UAID መግለጫዎች ጨምሮ) ሽቦ ያድርጉ
  የ Norito የዙር ጉዞዎችን እና አንጸባራቂ ግምገማዎችን ለማረጋገጥ ወደ ኤስዲኬ የሙከራ ስብስቦች
  ከአስተናጋጁ አተገባበር ጋር ይጣጣሙ.

## 7. ማጣቀሻዎች- `docs/space-directory.md` - ከጠለቀ የህይወት ዑደት ዝርዝር ጋር የኦፕሬተር መጫወቻ መጽሐፍ።
- `docs/source/torii/portfolio_api.md` - ለ UAID ፖርትፎሊዮ የ REST እቅድ እና
  የመጨረሻ ነጥቦችን አንጸባራቂ።
- `crates/iroha_cli/src/space_directory.rs` - የ CLI ትግበራ በ ውስጥ ተጠቅሷል
  ይህ መመሪያ.
- `fixtures/space_directory/capability/*.manifest.json` - ተቆጣጣሪ፣ ችርቻሮ እና
  CBDC አንጸባራቂ አብነቶች ለክሎኒንግ ዝግጁ ናቸው።