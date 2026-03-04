---
lang: am
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9308487dabef67df547ab0b5ed8cd2c7a186ed90e7bb06d03cff48ed441025
source_last_modified: "2026-01-22T14:45:01.359018+00:00"
translation_last_reviewed: 2026-02-07
title: Confidential Assets & ZK Transfers
description: Phase C blueprint for shielded circulation, registries, and operator controls.
slug: /nexus/confidential-assets
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# ሚስጥራዊ ንብረቶች እና የ ZK ማስተላለፍ ንድፍ

## ተነሳሽነት
- ግልጽ ስርጭትን ሳይቀይሩ ጎራዎች የግብይት ግላዊነትን እንዲጠብቁ መርጦ የመግባት የተከለለ የንብረት ፍሰት ያቅርቡ።
- ኦዲተሮችን እና ኦፕሬተሮችን ለወረዳዎች እና ምስጠራ መመዘኛዎች የህይወት ዑደት መቆጣጠሪያዎችን (ማግበር ፣ ማሽከርከር ፣ መሻር) ያቅርቡ።

## አስጊ ሞዴል
- አረጋጋጮች ሐቀኛ-ግን ጉጉ ናቸው፡ መግባባትን በታማኝነት ይፈጽማሉ ነገር ግን ደብተር/ግዛትን ለመመርመር ይሞክራሉ።
- የአውታረ መረብ ታዛቢዎች አግድ ውሂብ እና ሐሜት ግብይቶችን ያያሉ; የግል ሐሜት ጣቢያዎች ምንም ግምት.
- ከጥቅም ውጭ፡- ከደብተር ውጪ የትራፊክ ትንተና፣ የኳንተም ተቃዋሚዎች (በ PQ ፍኖተ ካርታው ስር ለብቻው ክትትል የሚደረግበት)፣ የመመዝገቢያ መገኘት ጥቃቶች።

## ንድፍ አጠቃላይ እይታ
- ንብረቶች አሁን ካሉት ግልጽ ሚዛኖች በተጨማሪ *የተከለለ ገንዳ* ሊያውጁ ይችላሉ። የተከለለ የደም ዝውውር በምስጠራ ቃል ኪዳኖች ይወከላል።
- ማስታወሻዎች `(asset_id, amount, recipient_view_key, blinding, rho)` በ:
  - ቃል ኪዳን: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, ከማስታወሻ ማዘዣ ነጻ.
  - የተመሰጠረ ጭነት፡ `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`።
- የግብይቶች ማጓጓዣ Norito-encoded `ConfidentialTransfer` ጭነት ጭነት
  - የህዝብ ግብዓቶች፡ Merkle መልህቅ፣ ወራሪዎች፣ አዲስ ግዴታዎች፣ የንብረት መታወቂያ፣ የወረዳ ስሪት።
  - ለተቀባዮች እና አማራጭ ኦዲተሮች የተመሰጠረ ክፍያ።
  - የዜሮ እውቀት ማረጋገጫ እሴትን መጠበቅን፣ ባለቤትነትን እና ፍቃድን ያረጋግጣል።
- የማረጋገጫ ቁልፎች እና የመለኪያ ስብስቦች የሚቆጣጠሩት በመመዝገቢያ መዝገብ ውስጥ በማንቃት መስኮቶች ነው; አንጓዎች ያልታወቁ ወይም የተሻሩ ግቤቶችን የሚጠቅሱ ማረጋገጫዎችን ለማረጋገጥ እምቢ ይላሉ።
- የስምምነት ራስጌዎች ንቁ ሚስጥራዊ ባህሪን ለመፍጨት ቃል ገብተዋል ስለዚህ ብሎኮች የሚቀበሉት መዝገብ እና መለኪያ ሁኔታ ሲዛመዱ ብቻ ነው።
- የማረጋገጫ ግንባታ የ Halo2 (Plonkish) ቁልል ያለ የታመነ ቅንብር ይጠቀማል; Groth16 ወይም ሌሎች የSNARK ተለዋጮች ሆን ተብሎ በቁ1 አይደገፉም።

### ቆራጥ ቋሚዎች

ሚስጥራዊ የማስታወሻ ፖስታዎች አሁን በ `fixtures/confidential/encrypted_payload_v1.json` ላይ ቀኖናዊ መሣሪያ ይዘው ይጓዛሉ። የመረጃ ቋቱ አወንታዊ v1 ኤንቨሎፕ እና አሉታዊ የተበላሹ ናሙናዎችን ስለሚይዝ ኤስዲኬዎች የመተንተን እኩልነትን ማረጋገጥ ይችላሉ። የ Rust ዳታ-ሞዴል ሙከራዎች (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) እና Swift suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) ሁለቱም መሳሪያውን በቀጥታ ይጭናሉ፣ ይህም Norito ኢንኮዲንግ፣ የስህተት ንጣፎች እና የሪግሬሽን ሽፋን ኮዴክ እየተሻሻለ ሲመጣ እንደሚቀጥሉ ዋስትና ይሰጣል።

ስዊፍት ኤስዲኬዎች አሁን የጋሻ መመሪያዎችን ያለ የጄሶን ሙጫ ማውጣት ይችላሉ፡ ግንባታ ሀ
`ShieldRequest` ባለ 32-ባይት ማስታወሻ ቁርጠኝነት፣ የተመሰጠረ ክፍያ እና የዴቢት ዲበ ዳታ፣
ከዚያ ለመፈረም እና ለማስተላለፍ `IrohaSDK.submit(shield:keypair:)` (ወይም `submitAndWait`) ይደውሉ
ግብይት በ `/v1/pipeline/transactions`. ረዳቱ የቁርጠኝነት ርዝመቶችን ያረጋግጣል ፣
ክሮች `ConfidentialEncryptedPayload` ወደ Norito ኢንኮደር፣ እና `zk::Shield` ያንጸባርቃል
አቀማመጥ ከዚህ በታች ተብራርቷል ስለዚህ የኪስ ቦርሳዎች ከዝገት ጋር በደረጃ መቆለፊያ ውስጥ እንዲቆዩ።## የስምምነት ቃል ኪዳኖች እና የችሎታ ጌቲንግ
- አግድ ራስጌዎች `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ያጋልጣሉ; ዳይጀስት በስምምነት ሃሽ ውስጥ ይሳተፋል እና ተቀባይነትን ለማገድ ከአካባቢው የመመዝገቢያ እይታ ጋር እኩል መሆን አለበት።
- አስተዳደር `next_conf_features` ከወደፊቱ `activation_height` ጋር በፕሮግራም በማዘጋጀት ማሻሻያ ማድረግ ይችላል; እስከዚያው ከፍታ ድረስ, እገዳዎች አምራቾች የቀደመውን የምግብ መፈጨት መቀጠል አለባቸው.
- አረጋጋጭ አንጓዎች ከ `confidential.enabled = true` እና `assume_valid = false` ጋር መሥራት አለባቸው። የጅምር ቼኮች ሁለቱም ሁኔታዎች ካልተሳካ ወይም የአካባቢ `conf_features` ልዩነት ከሆነ የማረጋገጫውን ስብስብ ለመቀላቀል ፈቃደኛ አይደሉም።
- P2P የመጨባበጥ ሜታዳታ አሁን `{ enabled, assume_valid, conf_features }` ያካትታል። የማይደገፉ ባህሪያትን የሚያስተዋውቁ እኩዮች በ`HandshakeConfidentialMismatch` ውድቅ ይደረጋሉ እና በጭራሽ ወደ ስምምነት ሽክርክር ውስጥ አይገቡም።
- አረጋጋጭ ያልሆኑ ታዛቢዎች `assume_valid = true` ሊያዘጋጁ ይችላሉ; ሚስጥራዊ ዴልታዎችን በጭፍን ይተገብራሉ ነገር ግን የጋራ መግባባት ደህንነት ላይ ተጽዕኖ አያሳርፉም።## የንብረት ፖሊሲዎች
- እያንዳንዱ የንብረት ፍቺ በፈጣሪ ወይም በአስተዳደር በኩል የተቀመጠውን `AssetConfidentialPolicy` ይይዛል፡
  - `TransparentOnly`: ነባሪ ሁነታ; ግልጽ መመሪያዎች ብቻ (`MintAsset`፣ `TransferAsset`፣ ወዘተ) የተፈቀዱ እና የተከለሉ ስራዎች ውድቅ ናቸው።
  - `ShieldedOnly`: ሁሉም መውጣት እና ማስተላለፎች ሚስጥራዊ መመሪያዎችን መጠቀም አለባቸው; `RevealConfidential` የተከለከለ ነው ስለዚህ ሚዛኖች በጭራሽ በይፋ አይታዩም።
  - `Convertible`: ያዢዎች ከታች ያለውን የበራ/ማጥፋት መመሪያዎችን በመጠቀም ግልጽ በሆነ እና በጋሻ ውክልና መካከል እሴትን ሊያንቀሳቅሱ ይችላሉ።
- ፖሊሲዎች የተገደበ ፈንዶችን ለመከላከል FSM ይከተላሉ፡-
  - `TransparentOnly → Convertible` (የተከለለ ገንዳ ወዲያውኑ ማንቃት).
  - `TransparentOnly → ShieldedOnly` (በመጠባበቅ ላይ ያለ የሽግግር እና የልወጣ መስኮት ያስፈልገዋል).
  - `Convertible → ShieldedOnly` (የተተገበረ ዝቅተኛ መዘግየት).
  - `ShieldedOnly → Convertible` (የተከለሉ ማስታወሻዎች ወጪ የሚቻሉ ሆነው እንዲቆዩ የፍልሰት እቅድ ያስፈልጋል)።
  - `ShieldedOnly → TransparentOnly` የተከለለ ገንዳው ባዶ ካልሆነ በስተቀር ወይም አስተዳደር አስደናቂ ማስታወሻዎችን የሚሸፍን ፍልሰትን ካልሰጠ በስተቀር የተከለከለ ነው።
- የአስተዳደር መመሪያዎች `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` በ `ScheduleConfidentialPolicyTransition` ISI በኩል ያስቀምጣቸዋል እና በ `CancelConfidentialPolicyTransition` የታቀዱ ለውጦችን ሊያስወግድ ይችላል። የሜምፑል ማረጋገጫ ምንም አይነት ግብይት ከሽግግሩ ቁመት ጋር እንደማይገናኝ እና የፖሊሲ ቼክ በመካከለኛው እገዳ ቢቀየር ማካተት እንደማይሳካ ያረጋግጣል።
- በመጠባበቅ ላይ ያሉ ሽግግሮች አዲስ ብሎክ ሲከፈት በራስ-ሰር ይተገበራሉ፡ የማገጃ ቁመቱ ወደ መለወጫ መስኮቱ ከገባ በኋላ (ለ`ShieldedOnly` ማሻሻያዎች) ወይም ፕሮግራም ወደተዘጋጀው `effective_height` ሲደርስ፣ የሩጫ ጊዜ ማሻሻያ `AssetConfidentialPolicy`፣ `zk.policy` ሜታውን ያድሳል፣ እና መግቢያውን ያጸዳል። የ `ShieldedOnly` ሽግግር ሲበስል ግልጽነት ያለው አቅርቦት የሚቆይ ከሆነ፣የሩጫ ጊዜው ለውጡን ያስወግደዋል እና ማስጠንቀቂያ ይመዘግባል፣የቀድሞው ሁነታ ሳይበላሽ ይቀራል።
- ማቀፊያ ቁልፎች `policy_transition_delay_blocks` እና `policy_transition_window_blocks` የኪስ ቦርሳዎች ማስታወሻዎችን በመቀየሪያው ዙሪያ እንዲቀይሩ ለማድረግ አነስተኛውን የማስታወቂያ እና የእፎይታ ጊዜ ያስገድዳሉ።
- `pending_transition.transition_id` እንደ ኦዲት እጀታ በእጥፍ ይጨምራል; ሽግግሮችን ሲያጠናቅቅ ወይም ሲሰርዝ አስተዳደር መጥቀስ አለበት ስለዚህ ኦፕሬተሮች የራምፕ ላይ/የማጥፋት ሪፖርቶችን ማዛመድ ይችላሉ።
- `policy_transition_window_blocks` ነባሪዎች ወደ 720 (≈12 ሰዓታት በ 60 ሴኮንድ የማገጃ ጊዜ)። አንጓዎች አጭር ማስታወቂያ የሚሞክሩ የአስተዳደር ጥያቄዎችን ያጨናሉ።
- ዘፍጥረት ይገለጣል እና CLI የወለል የአሁን እና በመጠባበቅ ላይ ያሉ ፖሊሲዎችን ይፈስሳል። የመግቢያ አመክንዮ እያንዳንዱ ሚስጥራዊ መመሪያ የተፈቀደ መሆኑን ለማረጋገጥ በአፈፃፀም ጊዜ ፖሊሲውን ያነባል።
- የፍልሰት ማረጋገጫ ዝርዝር - Milestone M0 የሚከታተለውን የማሻሻያ ዕቅድ ከዚህ በታች ያለውን "የስደት ቅደም ተከተል" ይመልከቱ።

#### በTorii በኩል ሽግግሮችን መከታተል

የኪስ ቦርሳዎች እና ኦዲተሮች ምርጫ `GET /v1/confidential/assets/{definition_id}/transitions` ለመመርመር
ንቁው `AssetConfidentialPolicy`. የJSON ክፍያ ሁልጊዜ ቀኖናዊውን ያካትታል
የንብረት መታወቂያ፣ የቅርብ ጊዜ የታየ የማገጃ ቁመት፣ የፖሊሲው `current_mode`፣ ይህ ሁነታ
በዚያ ከፍታ ላይ ውጤታማ (የልወጣ መስኮቶች `Convertible` ለጊዜው ሪፖርት ያደርጋሉ) እና
የሚጠበቀው `vk_set_hash`/Poseidon/Pedersen መለኪያ መለያዎች። አስተዳደር ሲፈጠር
ሽግግር በመጠባበቅ ላይ ነው ምላሹም የሚከተሉትን ያጠቃልላል- `transition_id` - የኦዲት እጀታ በ `ScheduleConfidentialPolicyTransition` ተመልሷል።
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` እና የተገኘው `window_open_height` (የኪስ ቦርሳዎች ያለበት እገዳ
  ለ ShieldedOnly መቁረጫዎች መለወጥ ይጀምሩ)።

ምሳሌ ምላሽ፡-

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

የ`404` ምላሽ ምንም ተዛማጅ የንብረት ፍቺ እንደሌለ ያሳያል። ሽግግር በማይኖርበት ጊዜ
የታቀደው የ`pending_transition` መስክ `null` ነው።

### ፖሊሲ ግዛት ማሽን| የአሁኑ ሁነታ | ቀጣይ ሁነታ | ቅድመ ሁኔታዎች | ውጤታማ-ቁመት አያያዝ | ማስታወሻ |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| ግልጽነት ብቻ | ሊለወጥ የሚችል | አስተዳደር አረጋጋጭ/መለኪያ መመዝገቢያ ግቤቶችን ነቅቷል። `ScheduleConfidentialPolicyTransition` በ `effective_height ≥ current_height + policy_transition_delay_blocks` አስገባ። | ሽግግር በትክክል በ `effective_height` ላይ ይሰራል; የተከለለ ገንዳ ወዲያውኑ ይገኛል።                   | ግልጽ ፍሰቶችን እየጠበቀ ምስጢራዊነትን ለማንቃት ነባሪ መንገድ።               |
| ግልጽነት ብቻ | በጋሻው ብቻ | ከላይ ካለው ጋር ተመሳሳይ፣ በተጨማሪም `policy_transition_window_blocks ≥ 1`።                                                         | የሩጫ ጊዜ በራስ-ሰር ወደ `Convertible` በ `effective_height - policy_transition_window_blocks` ያስገባል; ወደ `ShieldedOnly` በ `effective_height` ይገለበጣል። | ግልጽ መመሪያዎች ከመጥፋታቸው በፊት የሚወስን የልወጣ መስኮት ያቀርባል።   |
| ሊለወጥ የሚችል | በጋሻው ብቻ | ከ`effective_height ≥ current_height + policy_transition_delay_blocks` ጋር የታቀደ ሽግግር። አስተዳደር (`transparent_supply == 0`) በኦዲት ሜታዳታ በኩል ማረጋገጥ አለበት; Runtime ይህንን በማቋረጥ ላይ ያስፈጽማል። | ከላይ እንደተገለፀው ተመሳሳይ የመስኮት ትርጉም. በ `effective_height` ላይ ግልጽነት ያለው አቅርቦት ዜሮ ካልሆነ, ሽግግሩ በ `PolicyTransitionPrerequisiteFailed` ይቋረጣል. | ንብረቱን ወደ ሙሉ ሚስጥራዊ ስርጭት ይቆልፋል።                                     |
| በጋሻው ብቻ | ሊለወጥ የሚችል | የታቀደ ሽግግር; ምንም ገባሪ የአደጋ ጊዜ መውጣት የለም (`withdraw_height` አልተዋቀረም)።                                    | ግዛት በ `effective_height` ይገለበጣል; የታሸጉ ማስታወሻዎች ልክ እንደሆኑ በሚቆዩበት ጊዜ መወጣጫዎች እንደገና መከፈታቸውን አሳይ።                           | ለጥገና መስኮቶች ወይም ኦዲተር ግምገማዎች ጥቅም ላይ ይውላል።                                          |
| በጋሻው ብቻ | ግልጽነት ብቻ | አስተዳደር `shielded_supply == 0` ማረጋገጥ አለበት ወይም የተፈረመ `EmergencyUnshield` እቅድ (የኦዲተር ፊርማ ያስፈልጋል)። | Runtime `Convertible` መስኮት ከ `effective_height` ፊት ይከፍታል; በከፍታ ላይ ፣ ሚስጥራዊ መመሪያዎች ከባድ-ውድቀት እና ንብረቱ ወደ ግልፅ-ብቻ ሁነታ ይመለሳል። | የመጨረሻ ሪዞርት መውጣት። ማንኛውም ሚስጥራዊ ማስታወሻ በመስኮቱ ውስጥ የሚወጣ ከሆነ ሽግግር በራስ-ሰር ይሰርዛል። |
| ማንኛውም | ከአሁኑ ጋር ተመሳሳይ | `CancelConfidentialPolicyTransition` በመጠባበቅ ላይ ያለ ለውጥ ያጸዳል።                                                        | `pending_transition` ወዲያውኑ ተወግዷል።                                                                          | ሁኔታን ያቆያል; ለሙሉነት የሚታየው.                                             |ከላይ ያልተዘረዘሩ ሽግግሮች በአስተዳደር ግቤት ወቅት ውድቅ ይደረጋሉ። የአሂድ ጊዜ የታቀደ ሽግግርን ከመተግበሩ በፊት ቅድመ ሁኔታዎችን ይፈትሻል; ቅድመ ሁኔታዎች አለመሳካት ንብረቱን ወደ ቀድሞው ሁነታ ይገፋፋዋል እና `PolicyTransitionPrerequisiteFailed` በቴሌሜትሪ እና ዝግጅቶችን ያግዳል።

### የስደት ቅደም ተከተል

2. ** ሽግግሩን ደረጃ፡** `ScheduleConfidentialPolicyTransition` ከ `effective_height` ጋር `policy_transition_delay_blocks` ያቅርቡ። ወደ `ShieldedOnly` ሲሄዱ የመቀየሪያ መስኮት ይግለጹ (`window ≥ policy_transition_window_blocks`)።
3. **የኦፕሬተር መመሪያን ያትሙ፡** የተመለሰውን `transition_id` ይቅረጹ እና የራምፕ ላይ/የራምፕ runbook ያሰራጩ። የመስኮቱን ክፍት ቁመት ለማወቅ Wallets እና ኦዲተሮች ለ`/v1/confidential/assets/{id}/transitions` ይመዝገቡ።
4. **የመስኮት ማስፈጸሚያ፡** መስኮቱ ሲከፈት የሩጫ ሰዓቱ ፖሊሲውን ወደ `Convertible` ይቀይራል፣ `PolicyTransitionWindowOpened { transition_id }` ያወጣል እና የሚጋጩ የአስተዳደር ጥያቄዎችን አለመቀበል ይጀምራል።
5. ** ማጠናቀቅ ወይም ማስወረድ፡** በ `effective_height`፣ የሩጫ ጊዜው የሽግግር ቅድመ ሁኔታዎችን ያረጋግጣል (ዜሮ ግልጽ አቅርቦት፣ የአደጋ ጊዜ ማቋረጥ፣ ወዘተ)። ስኬት ፖሊሲውን ወደተጠየቀው ሁነታ ይገለብጣል; አለመሳካቱ `PolicyTransitionPrerequisiteFailed` ያወጣል፣ በመጠባበቅ ላይ ያለውን ሽግግር ያጸዳል እና ፖሊሲው ሳይለወጥ ይተወዋል።
6. ** የመርሃግብር ማሻሻያዎች፡** ከተሳካ ሽግግር በኋላ፣ አስተዳደር የንብረት እቅድ ሥሪትን ያደናቅፋል (ለምሳሌ፣ `asset_definition.v2`) እና CLI Tooling ተከታታይ ሲገለጥ `confidential_policy` ያስፈልገዋል። የጄነሲስ ማሻሻያ ሰነዶች ኦፕሬተሮች አረጋጋጮችን እንደገና ከመጀመርዎ በፊት የፖሊሲ ቅንብሮችን እና የመመዝገቢያ የጣት አሻራዎችን እንዲያክሉ ያዛል።

በምስጢር የሚጀምሩ አዳዲስ አውታረ መረቦች በቀጥታ በዘፍጥረት ውስጥ የተፈለገውን ፖሊሲ ያመለክታሉ። የልወጣ መስኮቶች ቆራጥ ሆነው እንዲቆዩ እና የኪስ ቦርሳዎች ለማስተካከል ጊዜ እንዲኖራቸው ከጅምሩ በኋላ ሁነታዎችን ሲቀይሩ አሁንም ከላይ ያለውን የማረጋገጫ ዝርዝር ይከተላሉ።

### Norito አንጸባራቂ ስሪት እና ማግበር- የዘፍጥረት መግለጫዎች ለብጁ `confidential_registry_root` ቁልፍ `SetParameter` ማካተት አለባቸው። ክፍያው Norito JSON የሚዛመድ `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` ነው፡ ምንም አረጋጋጭ ግቤቶች በማይሰሩበት ጊዜ መስኩን (`null`) ውጣ፣ ያለበለዚያ ባለ 32-ባይት ሄክስ string (`0x…`) በ10010101010101010101010101010101010101010101032 ባይት የሄክስ string ያቅርቡ። አንጸባራቂ ውስጥ ተልኳል። መስቀለኛ መንገድ መለኪያው ከጠፋ ወይም ሃሽ ከተቀየረው መዝገብ ቤት ጋር ካልተስማማ ለመጀመር እምቢ ይላሉ።
- በሽቦ ላይ ያለው `ConfidentialFeatureDigest::conf_rules_version` አንጸባራቂውን የአቀማመጥ ሥሪቱን አካቷል። ለv1 አውታረ መረቦች `Some(1)` መቆየት አለበት እና `iroha_config::parameters::defaults::confidential::RULES_VERSION` እኩል ነው። ደንቡ ሲዳብር ቋሚውን ይንጠቁጡ፣ መገለጫዎችን ያድሱ እና ሁለትዮሾችን በደረጃ መቆለፊያ ያድርጉ። ስሪቶችን መቀላቀል አረጋጋጮች በ`ConfidentialFeatureDigestMismatch` ብሎኮችን ውድቅ እንዲያደርጉ ያደርጋል።
- ማግበር የመመዝገቢያ ማሻሻያዎችን፣ የህይወት ዑደት ለውጦችን እና የፖሊሲ ሽግግሮችን መጠቅለል አለበት ስለዚህ የምግብ መፍጫ ስርዓቱ ወጥነት ያለው ሆኖ እንዲቆይ ማድረግ።
  1. የታቀዱትን የመመዝገቢያ ሚውቴሽን (`Publish*`, `Set*Lifecycle`) ከመስመር ውጭ በሆነ የግዛት እይታ ውስጥ ይተግብሩ እና የድህረ-አክቲቬሽን መፍጨትን በ `compute_confidential_feature_digest` ያሰሉት።
  2. Emit `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` የተሰላውን ሃሽ በመጠቀም የዘገዩ እኩዮች መካከለኛ የመመዝገቢያ መመሪያዎችን ቢያመልጡም ትክክለኛውን የምግብ መፍጨት ሂደት መልሰው ማግኘት ይችላሉ።
  3. የ `ScheduleConfidentialPolicyTransition` መመሪያዎችን አክል. እያንዳንዱ መመሪያ በአስተዳደር የተሰጠ `transition_id` መጥቀስ አለበት; የሚረሳው በጊዜው ውድቅ እንደሚሆን ያሳያል።
  4. አንጸባራቂ ባይት፣ SHA-256 የጣት አሻራ እና በማግበሪያ እቅድ ውስጥ ጥቅም ላይ የዋለውን የምግብ መፍጨት ሂደት ይቆዩ። ኦፕሬተሮች ክፍፍሎችን ለማስቀረት አንጸባራቂውን ወደ ተግባር ከመግባታቸው በፊት ሶስቱን ቅርሶች ያረጋግጣሉ።
- ልቀቶች የዘገየ መቁረጥ ሲፈልጉ፣ የዒላማውን ቁመት በተጓዳኝ ብጁ መለኪያ (ለምሳሌ `custom.confidential_upgrade_activation_height`) ይመዝግቡ። ይህ ኦዲተሮች የመፍጨት ለውጡ ተግባራዊ ከመሆኑ በፊት አረጋጋጮች የማስታወቂያ መስኮቱን እንዳከበሩ በNorito የተመሰጠረ ማረጋገጫ ይሰጣል።## አረጋጋጭ እና መለኪያ የህይወት ዑደት
### ZK መዝገብ ቤት
- ደብተር `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ያከማቻል `proving_system` በአሁኑ ጊዜ በ `Halo2` ላይ ተስተካክሏል.
- `(circuit_id, version)` ጥንዶች በዓለም አቀፍ ደረጃ ልዩ ናቸው; መዝገቡ በወረዳ ሜታዳታ ለመፈለግ ሁለተኛ ደረጃ መረጃ ጠቋሚ ይይዛል። የተባዙ ጥንዶችን ለማስመዝገብ የተደረጉ ሙከራዎች በመግቢያው ወቅት ውድቅ ይደረጋሉ።
- `circuit_id` ባዶ ያልሆነ መሆን አለበት እና `public_inputs_schema_hash` መቅረብ አለበት (በተለምዶ የብሌክ2ብ-32 አረጋጋጭ ቀኖናዊ የህዝብ ግብአት ኢንኮዲንግ)። መግቢያ እነዚህን መስኮች የሚተዉ መዝገቦችን ውድቅ ያደርጋል።
- የአስተዳደር መመሪያዎች የሚከተሉትን ያካትታሉ:
  - `PUBLISH` የ `Proposed` ግቤት በሜታዳታ ብቻ ለመጨመር።
  - `ACTIVATE { vk_id, activation_height }` የመግቢያ ማግበርን በጊዜ ገደብ ለማስያዝ።
  - `DEPRECATE { vk_id, deprecation_height }` ማረጋገጫዎች መግቢያውን ሊጠቅሱ የሚችሉበትን የመጨረሻውን ቁመት ምልክት ለማድረግ።
  - ለድንገተኛ አደጋ `WITHDRAW { vk_id, withdraw_height }`; አዲስ ግቤቶች እስኪነቃቁ ድረስ የተጎዱት ንብረቶች ከተነሱት ከፍታ በኋላ ሚስጥራዊ ወጪን ያቆማሉ።
- ኦሪት ዘፍጥረት `confidential_registry_root` ብጁ ልኬት `vk_set_hash` ከገባሪ ግቤቶች ጋር የሚዛመድ በራስ-አወጣን ያሳያል። ማረጋገጫ መስቀለኛ መንገድ የጋራ መግባባትን ከመቀላቀሉ በፊት ይህንን የምግብ አሰራር ከአካባቢያዊ መዝገብ ቤት ሁኔታ ጋር በማነፃፀር ያረጋግጣል።
- አረጋጋጭ መመዝገብ ወይም ማዘመን `gas_schedule_id` ያስፈልገዋል። ማረጋገጫው የመመዝገቢያ መግባቱ `Active`፣ በ`(circuit_id, version)` ኢንዴክስ ውስጥ የሚገኝ መሆኑን እና የHalo2 ማረጋገጫዎች `OpenVerifyEnvelope` የ `circuit_id`፣ `vk_hash`፣4001X መሆኑን ያረጋግጣል። የመመዝገቢያ መዝገብ.

### የማረጋገጫ ቁልፎች
- የማረጋገጫ ቁልፎች ከመሪ ውጭ ይቆያሉ ነገር ግን በይዘት አድራሻ ለዪዎች (`pk_cid`፣ `pk_hash`፣ `pk_len`) ከአረጋጋጭ ዲበ ዳታ ጋር ታትመዋል።
- የኪስ ቦርሳ ኤስዲኬዎች የፒኬ መረጃን ያመጣሉ፣ hashesን ያረጋግጣሉ እና በአገር ውስጥ መሸጎጫ።

### Pedersen እና Poseidon መለኪያዎች
- የተለዩ መዝገቦች (`PedersenParams`፣ `PoseidonParams`) የመስታወት አረጋጋጭ የሕይወት ዑደት መቆጣጠሪያዎች፣ እያንዳንዳቸው `params_id`፣ የጄነሬተሮች/ቋሚዎች ሃሽ፣ ማግበር፣ መቋረጥ እና ከፍታ ማውጣት።

## ቆራጥ ማዘዣ እና አጥፊዎች
- እያንዳንዱ ንብረት `CommitmentTree` ከ `next_leaf_index` ጋር ይይዛል; በቆራጥነት ቁርጠኝነትን ያግዳል፡ ግብይቶችን በብሎክ ቅደም ተከተል ይደግማል። በእያንዳንዱ ግብይት ውስጥ የተከታታይ `output_idx` ወደ ላይ በመውጣት የተከለከሉ ውጤቶችን ይደግማል።
- `note_position` ከዛፍ ማካካሻዎች የተገኘ ነው ነገር ግን ** አይደለም ** የ nullifier አካል; በማስረጃ ምስክር ውስጥ የአባልነት መንገዶችን ብቻ ይመገባል።
- በተሃድሶዎች ስር የኑልፋየር መረጋጋት በ PRF ንድፍ የተረጋገጠ ነው; የPRF ግቤት `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`ን ያገናኛል፣እና መልህቆች ታሪካዊ የመርክል ሥሮችን በ`max_anchor_age_blocks` የተገደበ ነው።## የመመዝገቢያ ፍሰት
1. ** ሚስጥራዊ { የንብረት_መታወቂያ ፣ መጠን ፣ የተቀባይ_ፍንጭ }**
   - የንብረት ፖሊሲ `Convertible` ወይም `ShieldedOnly` ያስፈልገዋል; የመግቢያ ቼኮች የንብረት ባለስልጣን ፣ የአሁኑን `params_id` ፣ ናሙናዎች `rho` ፣ ቁርጠኝነትን ያወጣል ፣ የመርክል ዛፍን ያሻሽላል።
   - `ConfidentialEvent::Shielded` በአዲሱ ቁርጠኝነት፣ Merkle root delta እና የግብይት ጥሪ ሃሽ ለኦዲት ዱካዎች ያመነጫል።
2. **ማስተላለፎች ምስጢር {የእሴት_መታወቂያ፣ ማረጋገጫ፣ ወረዳ_መታወቂያ፣ ስሪት፣ ውድቀቶች፣ አዲስ_ተግባሮች፣ ኢንክ_ክፍያዎች፣ መልህቅ_ስር፣ ማስታወሻ}**
   - VM syscall የመመዝገቢያ መግቢያን በመጠቀም ማረጋገጫን ያረጋግጣል; አስተናጋጅ ጥቅም ላይ ያልዋሉ ወራሪዎችን፣ ቃል ኪዳኖች በቆራጥነት መያዛቸውን ያረጋግጣል፣ መልህቅ የቅርብ ጊዜ ነው።
   - Ledger የ`NullifierSet` ግቤቶችን ይመዘግባል፣የተመሰጠረ ክፍያ ለተቀባዮች/ኦዲተሮች ያከማቻል እና `ConfidentialEvent::Transferred` ማጠቃለያዎችን፣የታዘዙ ውጤቶችን፣የማረጋገጫ ሃሽ እና የመርክል ስርወዎችን ያወጣል።
3. **መገለጥ ሚስጥራዊ { የንብረት_መታወቂያ፣ ማረጋገጫ፣ ወረዳ_መታወቂያ፣ ስሪት፣ መሻሪያ፣ መጠን፣ የተቀባዩ_መለያ፣ መልህቅ_ስር }**
   - ለ `Convertible` ንብረቶች ብቻ የሚገኝ; ማስረጃው የማስታወሻ ዋጋን ከተገለፀው መጠን ጋር እኩል ያፀድቃል፣ የሂሳብ ደብተር ግልፅ ቀሪ ሂሳብን ይሰጣል፣ እና የጠፋውን ውድቅ ምልክት በማድረግ የተከለለውን ማስታወሻ ያቃጥላል።
   - `ConfidentialEvent::Unshielded` በሕዝብ መጠን፣ በጥቅም ላይ የሚውሉ ውድቀቶች፣ ማረጋገጫ ለዪዎች እና የግብይት ጥሪ ሃሽ ያወጣል።

## የውሂብ ሞዴል ተጨማሪዎች
- `ConfidentialConfig` (አዲስ የውቅር ክፍል) ከማስቻል ባንዲራ ጋር፣ `assume_valid`፣ ጋዝ/ገደብ እንቡጦች፣ መልህቅ መስኮት፣ አረጋጋጭ የኋላ።
- `ConfidentialNote`፣ `ConfidentialTransfer`፣ እና `ConfidentialMint` Norito ዕቅዶች ከግልጽ ባይት (`CONFIDENTIAL_ASSET_V1 = 0x01`) ጋር።
- `ConfidentialEncryptedPayload` ለXChaCha20-Poly1305 አቀማመጥ `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` ነባሪ በማድረግ የ AEAD ማስታወሻ ባይት በ `{ version, ephemeral_pubkey, nonce, ciphertext }` ይጠቀልላል።
- ቀኖናዊ ቁልፍ-ተለዋዋጭ ቬክተሮች በ `docs/source/confidential_key_vectors.json` ውስጥ ይኖራሉ; ሁለቱም CLI እና Torii የመጨረሻ ነጥብ በእነዚህ መጫዎቻዎች ላይ ይደገማሉ።
- `asset::AssetDefinition` ያገኘው `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ነው።
- `ZkAssetState` `(backend, name, commitment)` ለማስተላለፍ/ያልተከለከሉ አረጋጋጮች ማሰሪያውን ይቀጥላል። ማስፈጸሚያ የተጠቀሰው ወይም በመስመር ውስጥ የማረጋገጫ ቁልፍ ከተመዘገበው ቁርጠኝነት ጋር የማይጣጣም ማስረጃዎችን ውድቅ ያደርጋል።
- `CommitmentTree` (ከድንበር ማመሳከሪያ ነጥቦች ጋር በአንድ ንብረት) ፣ `NullifierSet` በ `(chain_id, asset_id, nullifier)` ፣ `ZkVerifierEntry` ፣ `PedersenParams` ፣ `PoseidonParams` የተከማቸ
- ሜምፑል ጊዜያዊ የ`NullifierIndex` እና `AnchorIndex` አወቃቀሮችን ቀደምት ብዜት ማወቂያ እና የመልህቅ ዕድሜ ፍተሻዎችን ያቆያል።
- Norito የመርሃግብር ዝመናዎች ለህዝብ ግብዓቶች ቀኖናዊ ቅደም ተከተልን ያካትታሉ; የዙር ጉዞ ሙከራዎች ኢንኮዲንግ ቆራጥነትን ያረጋግጣሉ።
- የተመሰጠረ የክፍያ ጭነት ዙር ጉዞዎች በክፍል ሙከራዎች (`crates/iroha_data_model/src/confidential.rs`) ተቆልፈዋል። ተከታይ የኪስ ቦርሳ ቬክተሮች ለኦዲተሮች ቀኖናዊ የ AEAD ግልባጮችን ያያይዙታል። `norito.md` ለኤንቨሎፑ በሽቦ ላይ ያለውን ራስጌ ሰነድ ያደርጋል።## IVM ውህደት & Syscall
- `VERIFY_CONFIDENTIAL_PROOF` syscall መቀበልን ያስተዋውቁ፡-
  - `circuit_id`፣ `version`፣ `scheme`፣ `public_inputs`፣ `proof`፣ እና ውጤቱ `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`።
  - Syscall አረጋጋጭ ሜታዳታ ከመዝገቡ ውስጥ ይጭናል፣ የመጠን/የጊዜ ገደቦችን ያስፈጽማል፣ የሚወስን ጋዝ ያስከፍላል፣ እና ማስረጃው ከተሳካ ብቻ ዴልታ ይተገበራል።
- አስተናጋጁ የመርክል ስር ቅጽበተ-ፎቶዎችን እና የመሻር ሁኔታን ለማውጣት ተነባቢ-ብቻ `ConfidentialLedger` ባህሪን ያጋልጣል። Kotodama ቤተ መፃህፍት የምሥክርነት ማሰባሰቢያ ረዳቶችን እና የንድፍ ማረጋገጫዎችን ያቀርባል።
የማረጋገጫ ቋት አቀማመጥ እና የመመዝገቢያ መያዣዎችን ለማብራራት ጠቋሚ-ABI ሰነዶች ተዘምነዋል።

## የመስቀለኛ ችሎታ ድርድር
- የእጅ መጨባበጥ `feature_bits.confidential` ከ `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ጋር ያስተዋውቃል። የማረጋገጫ ተሳትፎ `confidential.enabled=true`፣ `assume_valid=false`፣ ተመሳሳይ አረጋጋጭ የኋለኛ ክፍል ለዪዎች እና ተዛማጅ የምግብ መፍጫ ዘዴዎችን ይፈልጋል። አለመመጣጠን በ `HandshakeConfidentialMismatch` መጨባበጥ ወድቋል።
- ኮንፊግ `assume_valid`ን ለተመልካች አንጓዎች ብቻ ይደግፋል፡ ሲሰናከል ሚስጥራዊ መመሪያዎችን ሲያጋጥመው ያለ ድንጋጤ ቆራጥ የሆነ `UnsupportedInstruction` ይሰጣል። ሲነቃ ታዛቢዎች ማረጋገጫዎችን ሳያረጋግጡ የታወጀ የስቴት ዴልታዎችን ይተገብራሉ።
- የአካባቢ አቅም ከተሰናከለ ሜምፑል ሚስጥራዊ ግብይቶችን ውድቅ ያደርጋል። የሐሜት ማጣሪያዎች ከችሎታው ጋር ሳይዛመዱ የተከለሉ ግብይቶችን ለእኩዮች ከመላክ ይቆጠባሉ እና ያልታወቁ የማረጋገጫ መታወቂያዎችን በመጠን ገደቦች ውስጥ እያስተላለፉ።

### የመግረዝ እና የማስወገጃ ማቆያ ፖሊሲ

ሚስጥራዊ ደብተሮች ማስታወሻ ትኩስነትን ለማረጋገጥ በቂ ታሪክ መያዝ አለባቸው
በአስተዳደር የሚመሩ ኦዲቶችን እንደገና ማጫወት። ነባሪው መመሪያ፣ በ
`ConfidentialLedger`፣ ነው፡-

- ** አጥፊ ማቆየት፡** ወጪ ማውረጃዎችን ለ * ቢያንስ* `730` ቀናት ያቆዩ (24)
  ወራቶች) ቁመትን ካሳለፉ በኋላ ወይም ረዘም ያለ ከሆነ በተቆጣጣሪው የታዘዘ መስኮት።
  ኦፕሬተሮች መስኮቱን በ `confidential.retention.nullifier_days` በኩል ማራዘም ይችላሉ።
  ከማቆያ መስኮቱ ያነሱ ነጣቂዎች በTorii መጠየቅ አለባቸው።
  ኦዲተሮች ድርብ ወጪ መቅረትን ማረጋገጥ ይችላሉ።
- ** መግረዝ ይገለጣል:** ግልጽነት ያለው መግሇጫ (`RevealConfidential`) መግረዝ
  ተያያዥ ማስታወሻዎች እገዳው ከተጠናቀቀ በኋላ ወዲያውኑ, ግን እ.ኤ.አ
  የተበላው nullifier ከላይ ላለው የማቆያ ህግ ተገዢ ነው። ከመግለጥ ጋር የተያያዘ
  ክስተቶች (`ConfidentialEvent::Unshielded`) የህዝብን መጠን፣ ተቀባይ፣
  እና ማስረጃ ሃሽ ስለዚህ ታሪካዊ ማሳያዎችን መልሶ መገንባት የተከረከመውን አይጠይቅም።
  ምስጢራዊ ጽሑፍ.
- **የፍተሻ ኬላዎች፡** የቁርጠኝነት ድንበሮች የሚሽከረከሩ የፍተሻ ቦታዎችን ይጠብቃሉ።
  ትልቁን የ `max_anchor_age_blocks` እና የማቆያ መስኮቱን ይሸፍናል. አንጓዎች
  የታመቁ የቆዩ የፍተሻ ነጥቦች በጊዜ ክፍተት ውስጥ ያሉት ሁሉም ውድቀቶች ካለቁ በኋላ ነው።
- ** የተዳከመ የምግብ መፈጨት ማስተካከያ፡** `HandshakeConfidentialMismatch` ከተነሳ
  ተንሳፋፊን ለማዋሃድ ኦፕሬተሮች (1) ያንን የሚያበላሹ መስኮቶችን ማረጋገጥ አለባቸው
  በክላስተር ላይ አሰልፍ፣ (2) `iroha_cli app confidential verify-ledger` ን አሂድ
  በተቀመጠው የማስወገጃ ስብስብ ላይ መፈጨትን እንደገና ማመንጨት እና (3) እንደገና ማሰማራት
  የታደሰ አንጸባራቂ። ያለጊዜው የተከረከመ ማንኛውም አስጸያፊዎች ወደነበሩበት መመለስ አለባቸው
  ወደ አውታረ መረቡ ከመቀላቀልዎ በፊት ቀዝቃዛ ማከማቻ።በኦፕሬሽኖች runbook ውስጥ የአካባቢያዊ መሻሮችን ሰነድ; የአስተዳደር ፖሊሲዎች ማራዘም
የማቆያ መስኮቱ የመስቀለኛ መንገድ ውቅረትን እና የማህደር ማከማቻ ዕቅዶችን ማዘመን አለበት።
መቆለፊያ.

### የማስወጣት እና መልሶ ማግኛ ፍሰት

1. በመደወል ወቅት፣ `IrohaNetwork` የማስታወቂያ ችሎታዎችን ያወዳድራል። ማንኛውም አለመዛመድ `HandshakeConfidentialMismatch` ከፍ ያደርገዋል; ግንኙነቱ ተዘግቷል እና እኩያው ወደ `Ready` ሳያስተዋውቅ በግኝት ወረፋ ውስጥ ይቆያል።
2. አለመሳካቱ በኔትዎርክ አገልግሎት ምዝግብ ማስታወሻ (ሪሞት ዲጀስት እና ጀርባን ጨምሮ) ይታያል፣ እና Sumeragi አቻውን ለፕሮፖዛል ወይም ድምጽ ለመስጠት መርሐግብር አይሰጥም።
3. ኦፕሬተሮች የማረጋገጫ መዝገቦችን እና የመለኪያ ስብስቦችን (Norito፣ Norito፣ `poseidon_params_id`) ወይም `next_conf_features`ን ከተስማማ Sumeragi ጋር በማስተካከል ያስተካክላሉ። የምግብ መፍጫው አንዴ ከተዛመደ የሚቀጥለው የእጅ መጨባበጥ በራስ-ሰር ይሳካል።
4. ያረጀ እኩያ ብሎክን (ለምሳሌ፣ በማህደር መልሶ ማጫወት) ማሰራጨት ከቻለ አረጋጋጮች በአውታረ መረቡ ላይ ወጥ የሆነ የመመዝገቢያ ሁኔታን በመያዝ በ`BlockRejectionReason::ConfidentialFeatureDigestMismatch` በቆራጥነት አይቀበሉም።

### ድጋሚ አጫውት ደህንነቱ የተጠበቀ የእጅ መጨባበጥ ፍሰት

1. እያንዳንዱ የወጪ ሙከራ ትኩስ ጫጫታ/X25519 ቁልፍ ቁሳቁስ ይመድባል። የተፈረመው የመጨባበጥ ጭነት (`handshake_signature_payload`) የአካባቢያዊ እና የርቀት ኢፍሜራል የህዝብ ቁልፎችን፣ Norito-የተቀየረ የማስታወቂያ ሶኬት አድራሻ እና—በ `handshake_chain_id` - ሰንሰለት መለያው ሲጠናቀር። መልእክቱ መስቀለኛ መንገድ ከመውጣቱ በፊት AEAD-የተመሰጠረ ነው።
2. ምላሽ ሰጪው የደመወዝ ጭነቱን በእኩያ/አካባቢያዊ ቁልፍ ትዕዛዝ ተቀልብሶ ያሰላል እና በ`HandshakeHelloV1` ውስጥ የተካተተውን የ Ed25519 ፊርማ ያረጋግጣል። ሁለቱም ጊዜያዊ ቁልፎች እና የማስታወቂያው አድራሻ የፊርማ ጎራ አካል በመሆናቸው የተቀረጸውን መልእክት በሌላ እኩያ ላይ እንደገና ማጫወት ወይም የቆየ ግንኙነትን መልሶ ማግኘት ማረጋገጥ በቆራጥነት አይሳካም።
3. ምስጢራዊ አቅም ባንዲራዎች እና `ConfidentialFeatureDigest` በ `HandshakeConfidentialMeta` ውስጥ ይጓዛሉ። ተቀባዩ tuple `{ enabled, assume_valid, verifier_backend, digest }` በአካባቢው ከተዋቀረ `ConfidentialHandshakeCaps` ጋር ያወዳድራል። መጓጓዣው ወደ `Ready` ከመሸጋገሩ በፊት ማንኛውም አለመዛመድ ከ`HandshakeConfidentialMismatch` ጋር ቀደም ብሎ ይወጣል።
4. ኦፕሬተሮች የምግብ መፍጫውን እንደገና ማስላት አለባቸው (በ`compute_confidential_feature_digest`) እና እንደገና ከመገናኘትዎ በፊት አንጓዎችን በተዘመኑት መዝገቦች/ፖሊሲዎች እንደገና ያስጀምሩ። የቆዩ መፈጨትን የሚያስተዋውቁ እኩዮች የእጅ መጨባበጥ አለመሳካታቸውን ቀጥለዋል፣ ይህም የቆየ ሁኔታ ወደ አረጋጋጭ ስብስብ ዳግም እንዳይገባ ይከለክላል።
5. የእጅ መጨባበጥ ስኬቶች እና ውድቀቶች ደረጃውን የጠበቀ `iroha_p2p::peer` ቆጣሪዎችን (`handshake_failure_count`፣ስህተት ታክሶኖሚ አጋዥዎች) ያዘምኑ እና የተዋቀሩ የምዝግብ ማስታወሻዎችን በርቀት የአቻ መታወቂያ እና የመፍጨት የጣት አሻራ ያመነጫሉ። በታቀደው ጊዜ የመልሶ ማጫወት ሙከራዎችን ወይም የተሳሳቱ ውቅሮችን ለማግኘት እነዚህን አመልካቾች ይከታተሉ።## ቁልፍ አስተዳደር እና ጭነቶች
- በየመለያ ቁልፍ የማውጫ ተዋረድ፡
  - `sk_spend` → `nk` (የማስሻሻ ቁልፍ)፣ `ivk` (መጪ መመልከቻ ቁልፍ)፣ `ovk` (የወጪ መመልከቻ ቁልፍ)፣ `fvk`።
- የተመሰጠረ የማስታወሻ ጭነት AEAD ከ ECDH-የተገኙ የጋራ ቁልፎች ጋር ይጠቀማሉ; አማራጭ የኦዲተር እይታ ቁልፎች በንብረት ፖሊሲ ከውጤቶች ጋር ሊጣበቁ ይችላሉ።
- CLI ተጨማሪዎች፡- `confidential create-keys`፣ `confidential send`፣ `confidential export-view-key`፣የኦዲተር ማስታዎሻዎችን ለመፍታት እና የ`iroha app zk envelope` ረዳት `iroha app zk envelope` ከመስመር ውጭ ኢንቬሎፕን ለመስራት። Torii በ `POST /v1/confidential/derive-keyset` በኩል ተመሳሳይ የመነሻ ፍሰት ያጋልጣል፣ ሁለቱንም ሄክስ እና ቤዝ64 ቅጾችን በመመለስ የኪስ ቦርሳ ቁልፍ ተዋረዶችን በፕሮግራማዊ መንገድ ማምጣት ይችላል።

## ጋዝ፣ ገደቦች እና የዶኤስ መቆጣጠሪያዎች
- የሚወስን የጋዝ መርሃ ግብር;
  - Halo2 (Plonkish): ቤዝ `250_000` ጋዝ + `2_000` ጋዝ በአንድ የሕዝብ ግብዓት.
  - `5` ጋዝ በማረጃ ባይት፣ ሲደመር ፐር- nullifier (`300`) እና የቃል ኪዳን (`500`) ክፍያዎች።
  - ኦፕሬተሮች እነዚህን ቋሚዎች በመስቀለኛ ውቅረት (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) ሊሽሯቸው ይችላሉ; ለውጦች በሚነሳበት ጊዜ ይሰራጫሉ ወይም የውቅር ንብርብሩ ትኩስ-እንደገና ሲጫን እና በክላስተር ላይ ወስኖ ሲተገበር።
- ከባድ ገደቦች (ሊዋቀሩ የሚችሉ ነባሪዎች)
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`፣ `max_commitments_per_tx = 8`፣ `max_confidential_ops_per_block = 256`።
- `verify_timeout_ms = 750`፣ `max_anchor_age_blocks = 10_000`። ከ`verify_timeout_ms` በላይ የሆኑ ማረጋገጫዎች መመሪያውን በቆራጥነት ያቋርጣሉ (የመንግስት ድምጽ መስጫ ወረቀቶች `proof verification exceeded timeout` ያወጣል፣ `VerifyProof` ስህተት ይመልሳል)።
- ተጨማሪ ኮታዎች መኖርን ያረጋግጣሉ፡- `max_proof_bytes_block`፣ `max_verify_calls_per_tx`፣ `max_verify_calls_per_block`፣ እና `max_public_inputs` የታሰሩ ብሎክ ግንበኞች። `reorg_depth_bound` (≥ `max_anchor_age_blocks`) የድንበር ፍተሻ ነጥብ ማቆየትን ይቆጣጠራል።
- የአሂድ ጊዜ ማስፈጸሚያ አሁን ከእነዚህ የግብይት ወይም በየብሎክ ገደቦች የሚበልጡ ግብይቶችን ውድቅ ያደርጋል፣ ቆራጥ የሆኑ `InvalidParameter` ስህተቶችን በማመንጨት እና የመመዝገቢያ ሁኔታ ሳይለወጥ ይተወዋል።
- የሜምፑል ቅድመ ማጣሪያዎች ሚስጥራዊ ግብይቶችን በ`vk_id`፣ የማረጋገጫ ርዝመት እና የመልህቅ ዕድሜ አረጋጋጩን ከመጥራቱ በፊት የሀብት አጠቃቀምን ገድቦ ይይዛል።
- ማረጋገጫ በጊዜ ማብቂያ ወይም የታሰረ ጥሰት ላይ በእርግጠኝነት ይቆማል; ግብይቶች በግልጽ ስህተቶች አይሳኩም። የሲምዲ ዳራዎች አማራጭ ናቸው ነገር ግን የጋዝ ሂሳብን አይቀይሩም።

### የካሊብሬሽን መሰረታዊ መስመሮች እና ተቀባይነት በሮች
- **የማጣቀሻ መድረኮች።** የመለኪያ ስራዎች ከዚህ በታች ያሉትን ሶስት የሃርድዌር መገለጫዎች መሸፈን አለባቸው። ሁሉንም መገለጫዎች ለመያዝ ያልቻሉ ሩጫዎች በግምገማ ወቅት ውድቅ ይደረጋሉ።| መገለጫ | አርክቴክቸር | ሲፒዩ / ምሳሌ | የአቀናባሪ ባንዲራዎች | ዓላማ |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) ወይም Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | ያለ ቬክተር ኢንትሪንሲክስ የወለል ዋጋዎችን ማቋቋም; የውድቀት ወጪ ሰንጠረዦችን ለማስተካከል ያገለግላል። |
  | `baseline-avx2` | `x86_64` | ኢንቴል Xeon ጎልድ 6430 (24c) | ነባሪ ልቀት | የ AVX2 መንገድን ያረጋግጣል; የሲምዲ ፍጥነት በገለልተኛ ጋዝ መቻቻል ውስጥ መቆየቱን ያረጋግጣል። |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | ነባሪ ልቀት | የNEON ጀርባ ቆራጥ እና ከ x86 መርሐግብሮች ጋር የተጣጣመ መሆኑን ያረጋግጣል። |

- ** የቤንችማርክ ማሰሪያ።** ሁሉም የጋዝ ልኬት ሪፖርቶች በሚከተሉት መሆን አለባቸው፡-
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` የሚወስነውን አካል ለማረጋገጥ።
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` በማንኛውም ጊዜ ቪኤም ኦፕኮድ ወጪዎች ሲቀየሩ።

- ** ቋሚ የዘፈቀደነት።** አግዳሚ ወንበሮችን ከመሮጥዎ በፊት `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` ወደ ውጭ ይላኩ ስለዚህ `iroha_test_samples::gen_account_in` ወደ መወሰኛ `KeyPair::from_seed` መንገድ ይቀየራል። መታጠቂያው `IROHA_CONF_GAS_SEED_ACTIVE=…` አንድ ጊዜ ያትማል; ተለዋዋጭው ከጠፋ፣ ግምገማው ውድቀት አለበት። ማንኛውም አዲስ የካሊብሬሽን መገልገያዎች ረዳት የዘፈቀደነትን ሲያስተዋውቁ ይህንን env var ማክበር መቀጠል አለባቸው።

- **ውጤት ቀረጻ።**
  - የመስፈርት ማጠቃለያዎችን (`target/criterion/**/raw.csv`) ለእያንዳንዱ መገለጫ ወደ ተለቀቀው አርቲፊኬት ይስቀሉ።
  - የተገኙ መለኪያዎችን (`ns/op`፣ `gas/op`፣ `ns/gas`) በ [ሚስጥራዊ ጋዝ ካሊብሬሽን መዝገብ](./confidential-gas-calibration) ከጂት ቁርጠኝነት እና ከጥቅም ላይ የዋለው የማጠናከሪያ ስሪት ጋር ያከማቹ።
  - የመጨረሻዎቹን ሁለት መነሻዎች በአንድ መገለጫ ይያዙ; አዲሱ ሪፖርት አንዴ ከተረጋገጠ የቆዩ ቅጽበተ-ፎቶዎችን ሰርዝ።

- ** ተቀባይነት መቻቻል።**
  - በ `baseline-simd-neutral` እና `baseline-avx2` መካከል ያለው የጋዝ ዴልታዎች ≤ ± 1.5% መቆየት አለባቸው።
  - በ`baseline-simd-neutral` እና `baseline-neon` መካከል ያለው የጋዝ ዴልታዎች ≤ ± 2.0% መቆየት አለባቸው።
  - ከእነዚህ ገደቦች የሚያልፍ የመለኪያ ሀሳቦች የመርሐግብር ማስተካከያዎችን ወይም አለመግባባቱን እና ቅነሳውን የሚያብራራ RFC ያስፈልጋቸዋል።

- ** የማረጋገጫ ዝርዝርን ይገምግሙ።** አስረካቢዎች ለሚከተሉት ተጠያቂዎች ናቸው፡-
  - `uname -a`፣ `/proc/cpuinfo` ቅንጭብጦች (ሞዴል፣ እርከን) እና `rustc -Vv`ን በማካካሻ መዝገብ ውስጥ።
  - በማረጋገጥ ላይ `IROHA_CONF_GAS_SEED` በቤንች ውፅዓት ውስጥ አስተጋባ (አግዳሚ ወንበሮቹ ንቁውን ዘር ያትማሉ)።
  - የልብ ምት ሰሪ እና ሚስጥራዊ አረጋጋጭ ባህሪ ባንዲራዎችን መስታወት ማምረት ማረጋገጥ (`--features confidential,telemetry` ባንኮችን በቴሌሜትሪ ሲሮጡ)።

## ማዋቀር እና ክወናዎች
- `iroha_config` ትርፍ `[confidential]` ክፍል:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- ቴሌሜትሪ አጠቃላይ መለኪያዎችን ያመነጫል፡- `confidential_proof_verified`፣ `confidential_verifier_latency_ms`፣ `confidential_proof_bytes_total`፣ `confidential_nullifier_spent`፣ `confidential_commitments_appended`፣ Kotodama፣20000 ግልጽ መረጃ.
- RPC ወለል;
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## የሙከራ ስልት
- ቆራጥነት፡ በዘፈቀደ በብሎኮች ውስጥ የሚደረግ የግብይት ልውውጥ ተመሳሳይ የመርክል ሥሮችን እና የጥፋት ስብስቦችን ይሰጣል።
- Reorg resilience: መልቲ-ብሎክ reorgs ከመልህቆች ጋር አስመስለው; አጥፊዎች የተረጋጋ እና የቆዩ መልህቆች ውድቅ ናቸው።
- የጋዝ መለዋወጦች፡- ተመሳሳይ የጋዝ አጠቃቀምን በሲምዲ እና ያለሲምዲ ማጣደፍ ያረጋግጡ።
- የድንበር ሙከራ፡ በመጠን/በጋዝ ጣራዎች ላይ ያሉ ማስረጃዎች፣ ከፍተኛ የመግቢያ/ውጪ ቆጠራዎች፣ የጊዜ ማብቂያ ማስፈጸሚያ።
- የህይወት ዑደት፡ የአስተዳደር ስራዎች ለአረጋጋጭ እና መለኪያ ማግበር/ማጥፋት፣ የማሽከርከር ወጪ ሙከራዎች።
- ፖሊሲ FSM፡ የተፈቀዱ/የተከለከሉ ሽግግሮች፣ በመጠባበቅ ላይ ያሉ የሽግግር መዘግየቶች፣ እና ውጤታማ በሆኑ ከፍታዎች ዙሪያ የሜምፑል ውድቅ ማድረግ።
- የአደጋ ጊዜ ምዝገባ፡ የአደጋ ጊዜ ማውጣት የተጎዱ ንብረቶችን በ `withdraw_height` ያቆማል እና ከዚያ በኋላ ማረጋገጫዎችን ውድቅ ያደርጋል።
- የችሎታ መግቢያ: አረጋጋጮች ከ `conf_features` እገዳዎች ጋር የማይዛመዱ; `assume_valid=true` ያላቸው ታዛቢዎች የጋራ መግባባት ላይ ተጽእኖ ሳያደርጉ ይቀጥላሉ.
- የስቴት አቻነት፡ አረጋጋጭ/ሙሉ/ተመልካች አንጓዎች በቀኖናዊው ሰንሰለት ላይ አንድ አይነት የግዛት ሥር ይፈጥራሉ።
- አሉታዊ ማጭበርበር፡ የተበላሹ ማስረጃዎች፣ ከመጠን በላይ የተጫኑ ሸክሞች እና የጥፋት ግጭቶች በቆራጥነት ውድቅ ያደርጋሉ።

#አስደናቂ ስራ
- የቤንችማርክ Halo2 መለኪያ ስብስቦች (የወረዳ መጠን፣ የመፈለጊያ ስልት) እና ውጤቱን በካሊብሬሽን መጫወቻ ደብተር ውስጥ ይመዝግቡ ስለዚህም የጋዝ/የጊዜ ማብቂያ ነባሪዎች ከሚቀጥለው `confidential_assets_calibration.md` እድሳት ጋር መዘመን ይችላሉ።
- የአስተዳደር ረቂቅ ከፈረመ በኋላ የተፈቀደውን የስራ ፍሰት ወደ Torii በማገናኘት የኦዲተርን ይፋ የማውጣት ፖሊሲዎችን እና ተያያዥ መራጭ እይታዎችን ያጠናቅቁ።
- የኤስዲኬ ፈጻሚዎችን የፖስታ ፎርማት በመመዝገብ የባለብዙ ተቀባይ ውጤቶችን እና የታሸጉ ማስታወሻዎችን ለመሸፈን የምሥክር ምስጠራ ዕቅዱን ያራዝሙ።
- የወረዳዎች ፣ መዝገቦች እና የመለኪያ-ማሽከርከር ሂደቶችን የውጭ ደህንነት ግምገማ ኮሚሽን እና ግኝቶቹን ከውስጥ ኦዲት ሪፖርቶች ቀጥሎ በማህደር ያስቀምጡ።
- የኦዲተር ወጪ ማስታረቅ ኤፒአይዎችን ይግለጹ እና የኪስ ቦርሳ አቅራቢዎች ተመሳሳይ የማረጋገጫ ትርጉሞችን መተግበር እንዲችሉ የእይታ ቁልፍ ወሰን መመሪያን ያትሙ።## የትግበራ ደረጃ
1. ** ደረጃ M0 - ማቆም-መርከቦችን ማጠናከር ***
   - ✅ Nullifier መነጨው አሁን በፖሲዶን PRF ንድፍ (`nk`፣ `rho`፣ `asset_id`፣ `chain_id`) በመመዝገቢያ ዝማኔዎች ውስጥ የሚተገበር ቆራጥ ቁርጠኝነትን ይከተላል።
   - ✅ አፈፃፀሙ የማረጋገጫ መጠን ካፕ እና በአንድ ግብይት/በአንድ-ማገድ ሚስጥራዊ ኮታዎችን ያስፈጽማል፣ከበጀት ​​በላይ የሆኑ ግብይቶችን ከመወሰኛ ስህተቶች ጋር ውድቅ ያደርጋል።
   - ✅ P2P የእጅ መጨባበጥ `ConfidentialFeatureDigest` ያስተዋውቃል (የጀርባ ዳጀስት + መዝገብ የጣት አሻራዎች) እና በ`HandshakeConfidentialMismatch` በኩል አለመዛመጃዎችን በመወሰን አልተሳካም።
   - ✅ በምስጢር የማስፈጸሚያ መንገዶች ላይ ድንጋጤን ያስወግዱ እና የአቅም ማዛመጃ ሳይሆኑ ለአንጓዎች የሚጫወቱትን ሚና ይጨምሩ።
   - ⚪ የማረጋገጫ ጊዜ ማብቂያ በጀቶችን ያስፈጽሙ እና ለድንበር ፍተሻ ቦታዎች ጥልቅ ድንበሮችን ያሻሽሉ።
     - ✅ የማረጋገጫ ጊዜ ማብቂያ በጀቶች ተፈፃሚ ሆነዋል; ከ `verify_timeout_ms` በላይ የሆኑ ማረጋገጫዎች አሁን በትክክል ወድቀዋል።
     - ✅ የድንበር ማመሳከሪያ ነጥቦች አሁን `reorg_depth_bound`ን ያከብራሉ፣ ከተዋቀረው መስኮት በላይ የቆዩ የፍተሻ ነጥቦችን በመቁረጥ ቆራጥ የሆኑ ቅጽበተ-ፎቶዎችን እየጠበቁ።
   - `AssetConfidentialPolicy`፣ የፖሊሲ ኤፍኤስኤም እና የማስፈጸሚያ በሮች ለአዝሙድ/ማስተላለፊያ/መግለጥ መመሪያዎችን ያስተዋውቁ።
   - በብሎክ ራስጌዎች ውስጥ `conf_features` ግባ እና የአረጋጋጭ ተሳትፎን እምቢ ማለት መዝገብ/መለኪያ ሲለያይ።
2. ** ደረጃ M1 - መዝገቦች እና መለኪያዎች ***
   - መሬት `ZkVerifierEntry`፣ `PedersenParams`፣ እና `PoseidonParams` መዝገብ ቤቶች ከአስተዳደር ኦፕ፣ የዘፍጥረት መልህቅ እና መሸጎጫ አስተዳደር ጋር።
   - የመመዝገቢያ ፍለጋዎች፣ የጋዝ መርሐግብር መታወቂያዎች፣ schema hashing እና የመጠን ቼኮችን ለመፈለግ ሽቦ ሲስካል።
   - የተመሰጠረ የመክፈያ ፎርማት v1፣ የኪስ ቦርሳ ቁልፍ ምንጭ ቬክተሮችን እና CLI ለሚስጥራዊ ቁልፍ አስተዳደር ድጋፍ።
3. ** ደረጃ M2 - ጋዝ እና አፈፃፀም ***
   - የሚወስን የጋዝ መርሐግብር፣ በብሎክ ቆጣሪዎች እና የቤንችማርክ ማሰሪያዎችን በቴሌሜትሪ ይተግብሩ (የቆይታ ጊዜን፣ የማረጋገጫ መጠኖችን፣ የሜምፑል ውድቅዎችን ያረጋግጡ)።
   - Harden CommitmentTree የፍተሻ ነጥቦች፣ የLRU ጭነት እና የባለብዙ ንብረት የስራ ጫናዎች የኑልፋይር ኢንዴክሶች።
4. ** ደረጃ M3 - ማሽከርከር እና የኪስ ቦርሳ መሳሪያ **
   - ባለብዙ-መለኪያ እና ባለብዙ-ስሪት ማረጋገጫ መቀበልን አንቃ; ከሽግግር runbooks ጋር በአስተዳደር የሚመራ ማግበር/መቀነስን ይደግፉ።
   - የኪስ ቦርሳ ኤስዲኬ/CLI የፍልሰት ፍሰቶችን፣የኦዲተርን የስራ ፍሰቶችን እና የወጪ ማስታረቂያ መሳሪያዎችን ያቅርቡ።
5. **ደረጃ M4 - ኦዲት እና ኦፕስ**
   - የኦዲተር ቁልፍ የስራ ፍሰቶችን፣ የተመረጠ ገላጭ ኤፒአይዎችን እና የስራ ማስኬጃ መጽሐፍትን ያቅርቡ።
   - የውጭ ምስጠራ/የደህንነት ግምገማን መርሐግብር አውጥተህ ግኝቶችን በ`status.md` አትም።

ለብሎክቼይን አውታረመረብ ቆራጥ የማስፈጸሚያ ዋስትናዎችን ለመጠበቅ እያንዳንዱ ደረጃ የመንገድ ካርታ ዋና ዋና ደረጃዎችን እና ተዛማጅ ሙከራዎችን ያሻሽላል።