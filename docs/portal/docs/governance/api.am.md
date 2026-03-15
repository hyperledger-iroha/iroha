---
lang: am
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14dbd50875bebd8d9f8c9f03f85cb458c909c9da956a7a7048c9dae8c885b969
source_last_modified: "2026-01-22T16:26:46.496232+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

ሁኔታ፡ ከአስተዳደር ትግበራ ተግባራት ጋር አብሮ የሚሄድ ረቂቅ/ንድፍ። በመተግበር ጊዜ ቅርጾች ሊለወጡ ይችላሉ. ቆራጥነት እና የ RBAC ፖሊሲ መደበኛ ገደቦች ናቸው; Torii `authority` እና `private_key` ሲቀርብ ግብይቶችን መፈረም/ማስገባት ይችላል፣ይህ ካልሆነ ደንበኞች ገንብተው ለ`/transaction` ማቅረብ ይችላሉ።

አጠቃላይ እይታ
- ሁሉም የመጨረሻ ነጥቦች JSON ይመለሳሉ። ግብይትን ለሚፈጥሩ ፍሰቶች፣ ምላሾች `tx_instructions` ያካትታሉ - የአንድ ወይም ከዚያ በላይ የማስተማሪያ አጽሞች ስብስብ፡-
  - `wire_id`: ለመመሪያው ዓይነት የመመዝገቢያ መለያ
  - `payload_hex`: Norito የመጫኛ ባይት (ሄክስ)
- `authority` እና `private_key` ከቀረቡ (ወይም `private_key` በድምጽ መስጫ DTOs)፣ Torii ምልክት አድርጎ ግብይቱን አስረክቦ አሁንም `tx_instructions` ይመልሳል።
- ያለበለዚያ ደንበኞቻቸው ሥልጣናቸውን እና ቼይንሲይድ በመጠቀም SignedTransaction ይሰበስባሉ፣ ከዚያ ይፈርሙ እና ወደ `/transaction` ይለጥፉ።
- የኤስዲኬ ሽፋን፡-
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` ይመልሳል `GovernanceProposalResult` (የተለመደ ሁኔታ/አይነት መስኮች)፣ `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` ይመልሳል፣ Norito `ToriiClient.get_governance_locks_typed` ይመልሳል `GovernanceLocksResult`፣ `ToriiClient.get_governance_unlock_stats_typed` ይመልሳል `GovernanceUnlockStats`፣ እና `ToriiClient.list_governance_instances_typed` `ToriiClient.list_governance_instances_typed` ይመልሳል `GovernanceInstancesPage` በመላ ዩኤስ ላይ የተተየበው የአስተዳደር ዘይቤን ያስፈጽማል።
- ፓይዘን ቀላል ክብደት ያለው ደንበኛ (`iroha_torii_client`)፡ `ToriiClient.finalize_referendum` እና `ToriiClient.enact_proposal` መመለሻ የተተየበው `GovernanceInstructionDraft` ቅርቅቦች (Torii አጽም Prometheus ጥቅል መጠቅለል) ፍሰቶችን ያጠናቅቁ/አጽድቁ።
- JavaScript (`@iroha/iroha-js`): `ToriiClient` ንጣፎች ለፕሮፖዛል፣ ለሪፈረንዳ፣ ለከፍታዎች፣ ለመቆለፊያዎች፣ ለመክፈቻ ስታቲስቲክስ እና አሁን `listGovernanceInstances(namespace, options)` እና የምክር ቤቱ የመጨረሻ ነጥቦች (`getGovernanceCouncilCurrent`፣00000059X፣000000059X፣000000059X፣00060X `governancePersistCouncil`፣ `getGovernanceCouncilAudit`) ስለዚህ የ Node.js ደንበኞች `/v2/gov/instances/{ns}` ን በማንሳት በVRF የሚደገፉ የስራ ፍሰቶችን ከነባሩ የኮንትራት ምሳሌ ዝርዝር ጋር ማሽከርከር ይችላሉ።

የመጨረሻ ነጥቦች

- POST `/v2/gov/proposals/deploy-contract`
  - ጥያቄ (JSON)፡-
    {
      "namespace": "መተግበሪያዎች",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "… 64ሄክስ",
      "አቢ_ሀሽ"፡ "blake2b32:..." | "… 64ሄክስ",
      "አቢ_ቨርዥን": "1",
      "መስኮት": {"ዝቅተኛ": 12345, "ላይ": 12400},
      "ስልጣን": "i105...?",
      "የግል_ቁልፍ": "...?"
    }
  ምላሽ (JSON)፡-
    {"እሺ"፡ እውነት፡ "ፕሮፖሳል_መታወቂያ"፡ "…64hex"፣ "tx_instructions": [{ "wire_id": "…", "payload_hex": "..." }] }
  - ማረጋገጫ፡ አንጓዎች `abi_hash` ለቀረበው `abi_version` እና አለመዛመጃዎችን ውድቅ ያድርጉ። ለ `abi_version = "v1"`፣ የሚጠበቀው ዋጋ `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ነው።ኮንትራቶች ኤፒአይ (ተሰማራ)
- POST `/v2/contracts/deploy`
  ጥያቄ፡ {"ስልጣን"፡ "i105..."፣ "የግል_ቁልፍ"፡ "..."፣ "code_b64": "..." }
  - ባህሪ: `code_hash` ከ IVM ፕሮግራም አካል እና `abi_hash` ከ አርዕስት `abi_version` ያሰላል, ከዚያም `RegisterSmartContractCode` (የሚገለጥበት) እና I00NI704 ያቀርባል. `.to` ባይት) በ `authority` ስም።
  ምላሽ፡ {"እሺ"፡ እውነት፡ "code_hash_hex"፡ "..."፣ "አቢ_ሀሽ_ሄክስ"፡ "..." }
  - ተዛማጅ፡
    - GET `/v2/contracts/code/{code_hash}` → የተከማቸ አንጸባራቂ ይመልሳል
    - GET `/v2/contracts/code-bytes/{code_hash}` → `{ code_b64 }` ይመልሳል
- POST `/v2/contracts/instance`
  ጥያቄ፡ {"ስልጣን"፡ "i105..."፣ "የግል_ቁልፍ"፡ "..."፣ "ስም ቦታ": "መተግበሪያዎች"፣ "ኮንትራት_መታወቂያ"፡ "calc.v1"፣ "code_b64": "..." }
  - ባህሪ፡ የቀረበውን ባይትኮድ ያሰማራና ወዲያውኑ የ`(namespace, contract_id)` ካርታውን በ`ActivateContractInstance` ያንቀሳቅሰዋል።
  ምላሽ፡ {"እሺ"፡ እውነት፡ "ስም ቦታ"፡ "መተግበሪያዎች"፡ "ኮንትራት_መታወቂያ"፡ "calc.v1"፣ "code_hash_hex"፡ "…"፣ "abi_hash_hex": "..." }

ተለዋጭ አገልግሎት
- ፖስት `/v2/aliases/voprf/evaluate`
  - ጥያቄ፡ {"blindded_element_hex"፡ "..." }
  ምላሽ፡ {"የተገመገመ_element_hex"፡ "…128hex", "backend": "blake2b512-mock"}}
    - `backend` የግምገማውን አተገባበር ያንፀባርቃል። የአሁኑ ዋጋ፡ `blake2b512-mock`
  - ማስታወሻዎች፡ Blake2b512ን ከጎራ መለያየት `iroha.alias.voprf.mock.v1` ጋር የሚተገበር ቆራጥ የፌዝ ገምጋሚ። የምርት VOPRF የቧንቧ መስመር በIroha እስኪሰካ ድረስ ለሙከራ መሳሪያ ነው።
  - ስህተቶች፡ HTTP `400` በተበላሸ የሄክስ ግቤት ላይ። Torii Norito `ValidationFail::QueryFailed::Conversion` ፖስታ ከዲኮደር ስህተት መልእክት ጋር ይመልሳል።
- POST `/v2/aliases/resolve`
  - ጥያቄ፡ {"ተለዋጭ ስም"፡ "GB82 WEST 1234 5698 7654 32" }
  ምላሽ፡ {"ተለዋጭ ስም"፡ "GB82WEST12345698765432"፣ "መለያ_መታወቂያ"፡ "i105..."፣ "ኢንዴክስ"፡ 0፣ "ምንጭ"፡ "ኢሶ_ብሪጅ" }
  - ማስታወሻዎች፡ የ ISO ድልድይ አሂድ ጊዜ ዝግጅትን ይፈልጋል (`[iso_bridge.account_aliases]` በ `iroha_config`)። Torii ከመፈተሽ በፊት ነጭ ቦታን እና የላይኛውን መያዣ በመንጠቅ ተለዋጭ ስሞችን መደበኛ ያደርገዋል። ተለዋጭ ስም በማይኖርበት ጊዜ 404 እና 503 የ ISO bridge runtime ሲጠፋ ይመልሳል።
- POST `/v2/aliases/resolve_index`
  - ጥያቄ፡ {"ኢንዴክስ"፡ 0}
  ምላሽ፡ {"ኢንዴክስ"፡ 0፣ "ተለዋጭ ስም"፡ "GB82WEST12345698765432"፣ "መለያ_መታወቂያ"፡ "i105..."፣ "ምንጭ"፡ "iso_bridge"}
  - ማስታወሻዎች፡ የአሊያስ ኢንዴክሶች ከውቅረት ቅደም ተከተል (0-ተኮር) በወሰነው ሁኔታ ተመድበዋል። ለተለዋጭ ማረጋገጫ ክስተቶች የኦዲት መንገዶችን ለመገንባት ደንበኞች ከመስመር ውጭ ምላሾችን መሸጎጫ ማድረግ ይችላሉ።

የኮድ መጠን ካፕ
- ብጁ መለኪያ፡ `max_contract_code_bytes` (JSON u64)
  - በሰንሰለት የኮንትራት ኮድ ማከማቻ የሚፈቀደውን ከፍተኛ መጠን (በባይት) ይቆጣጠራል።
  - ነባሪ: 16 ሚቢ. የ `.to` ምስል ርዝመት በማይለዋወጥ ጥሰት ስህተት ከካፒታው ሲያልፍ አንጓዎች `RegisterSmartContractBytes`ን አይቀበሉም።
  - ኦፕሬተሮች `SetParameter(Custom)` ከ `id = "max_contract_code_bytes"` እና የቁጥር ጭነት ጋር በማስገባት ማስተካከል ይችላሉ።- POST `/v2/gov/ballots/zk`
  ጥያቄ፡- {"ሥልጣን"፡ "i105..."፣ "የግል_ቁልፍ"፡ "…?"፣ "ቼይን_መታወቂያ"፡ "..."፣ "ምርጫ_መታወቂያ"፡ "e1"፣ "ማስረጃ_b64"፡ "…", "ይፋዊ"፡ {…} }
  ምላሽ፡ {"እሺ"፡ እውነት፡ "ተቀባይነት ያለው"፡ እውነት፡ "tx_instructions"፡ [{…}] }
  - ማስታወሻዎች:
    - የወረዳው የህዝብ ግብአቶች `owner`፣ `owner`፣ `amount` እና `duration_blocks`ን ሲያካትቱ እና ማስረጃው ከተዋቀረው ቪኬ ጋር ሲገናኝ መስቀለኛ መንገዱ የአስተዳደር መቆለፊያን ይፈጥራል ወይም ያራዝመዋል ለ Sumeragi አቅጣጫ ተደብቆ ይቆያል (`unknown`); መጠኑ/የሚያበቃበት ጊዜ ብቻ ተዘምኗል። ድጋሚ-ድምጾች ነጠላ ናቸው፡ መጠኑ እና ጊዜው የሚያበቃው ብቻ ነው የሚጨምረው (መስቀለኛ መንገድ የሚመለከተው ከፍተኛ(መጠን፣ ቀዳሚ መጠን) እና ከፍተኛ(የሚያበቃበት፣ prev.expiry)) ነው።
    - ZK መጠኑን ለመቀነስ ወይም ጊዜው የሚያበቃበትን ድጋሚ የሰጡት የአገልጋይ ወገን ከ`BallotRejected` መመርመሪያዎች ጋር ውድቅ ተደርገዋል።
    - የኮንትራት አፈፃፀም `ZK_VOTE_VERIFY_BALLOT` መደወል አለበት ከመግባቱ በፊት `SubmitBallot`; አስተናጋጆች የአንድ-ምት መቀርቀሪያን ያስገድዳሉ።

- ፖስት `/v2/gov/ballots/plain`
  - ጥያቄ፡ {"ስልጣን"፡ "i105..."፣ "የግል_ቁልፍ"፡ "…?"፣ "ሰንሰለት_መታወቂያ"፡ "…"፣ "ሪፈረንደም_መታወቂያ"፡ "r1"፣ "ባለቤት"፡ "i105..."፣ "መጠን"፡ "1000"፣ "ቆይታ_ብሎኮች"፡ 6000፣ "አቅጣጫ"|አብዬ
  ምላሽ፡ {"እሺ"፡ እውነት፡ "ተቀባይነት ያለው"፡ እውነት፡ "tx_instructions"፡ [{…}] }
  - ማስታወሻዎች፡ ድጋሚ-ድምጾች የተራዘሙ ብቻ ናቸው - አዲስ የድምጽ መስጫ ወረቀቱ ያለውን የመቆለፊያ መጠን ወይም የአገልግሎት ጊዜው ሊቀንስ አይችልም። `owner` ከግብይቱ ባለስልጣን ጋር እኩል መሆን አለበት። ዝቅተኛው የቆይታ ጊዜ `conviction_step_blocks` ነው።

- POST `/v2/gov/finalize`
  ጥያቄ፡ {"የህዝበ ውሳኔ"፡ "r1"፣ "proposal_id": "…64hex"፣ "ስልጣን": "i105…?"፣ "የግል_ቁልፍ"፡ "…?" }
  ምላሽ፡ {"እሺ"፡ እውነት፡ "tx_instructions"፡ [{"wire_id"፡ "…FinalizeReferendum"፣ "payload_hex":"…"}] }
  በሰንሰለት ላይ ተጽእኖ (የአሁኑ ስካፎልድ)፡ የጸደቀ የማሰማራት ፕሮፖዛልን ማፅደቅ በ`code_hash` ከተጠበቀው `abi_hash` ጋር በትንሹ `ContractManifest` ያስገባል እና ፕሮፖዛሉ የፀደቀውን ያመላክታል። ለ`code_hash` የተለየ `abi_hash` ያለው አንጸባራቂ ካለ፣ ህጉ ተቀባይነት አላገኘም።
  - ማስታወሻዎች:
    - ለ ZK ምርጫዎች, የኮንትራት ዱካዎች `ZK_VOTE_VERIFY_TALLY` መደወል አለባቸው `FinalizeElection`; አስተናጋጆች የአንድ-ምት መቀርቀሪያን ያስገድዳሉ። `FinalizeReferendum` የምርጫው ጠቅላላ ድምር እስኪጠናቀቅ ድረስ የ ZK ሪፈረንድን ውድቅ አደረገው።
    - በራስ-ሰር ዝጋ በ `h_end` ልቀቶች የተፈቀደ/የተከለከለ ለሜዳ ሪፈረንዳ ብቻ; የ ZK ሪፈረንዳ የተጠናቀቀ ድምር እስኪቀርብ እና `FinalizeReferendum` እስኪፈጸም ድረስ ተዘግቷል።
    - የመመለሻ ቼኮች ማጽደቅ+ ውድቅ ብቻ ይጠቀማሉ። መታቀብ ለምርጫ አይቆጠርም።

- POST `/v2/gov/enact`
  ጥያቄ፡- {"ፕሮፖሳል_መታወቂያ"፡"…64hex"፣ "preimage_hash": "…64hex?"፣ "መስኮት"፡ {"ዝቅተኛ"፡ 0፣ "የላይ"፡ 0}?፣ "ስልጣን"፡ "i105…?"፣ "የግል_ቁልፍ": "…?" }
  ምላሽ፡ {"እሺ"፡ እውነት፡ "tx_instructions"፡ [{"wire_id"፡ "…EnactReferendum"፣ "payload_hex"፡"…"}] }
  - ማስታወሻዎች: Torii `authority`/`private_key` ሲቀርብ የተፈረመውን ግብይት ያቀርባል; አለበለዚያ ደንበኞች እንዲፈርሙ እና እንዲያቀርቡ አጽም ይመልሳል. ቅድመ እይታው አማራጭ እና በአሁኑ ጊዜ መረጃዊ ነው።- `/v2/gov/proposals/{id}` ያግኙ
  መንገድ `{id}`፡ ፕሮፖዛል መታወቂያ ሄክስ (64 ቻርልስ)
  ምላሽ፡ {"ተገኝ"፡ ቡል፡ "ፕሮፖዛል"፡ {… }? }

- `/v2/gov/locks/{rid}` ያግኙ
  - መንገድ `{rid}`: የሪፈረንደም መታወቂያ ሕብረቁምፊ
  ምላሽ፡ {"ተገኝ"፡ ቡል፣ "ሪፈረንደም_መታወቂያ"፡ "ሪድ"፣ "መቆለፊያዎች"፡ { … }? }

- `/v2/gov/council/current` ያግኙ
  - ምላሽ፡ {"epoch": N, "አባላት": [{"መለያ_መታወቂያ": "..." }, …] }
  - ማስታወሻዎች: የጸና ምክር ቤት በሚገኝበት ጊዜ ይመልሳል; ያለበለዚያ የተዋቀረውን የአክሲዮን ንብረት እና ገደቦችን በመጠቀም የሚወስን ውድቀትን ያመጣል (ቀጥታ የቪአርኤፍ ማረጋገጫዎች በሰንሰለት ላይ እስከሚቆዩ ድረስ የVRF ዝርዝርን ያሳያል)።

- POST `/v2/gov/council/derive-vrf` (ባህሪ፡ gov_vrf)
  - ጥያቄ፡ {"የኮሚቴ_መጠን"፡ 21፣ "epoch": 123? , "እጩዎች": [{ "መለያ_መታወቂያ": "…", "ተለዋዋጭ": "መደበኛ|ትንሽ", "pk_b64": "…", "ማስረጃ_b64": "…" }, …] }
  - ባህሪ፡ የእያንዳንዱን እጩ የቪአርኤፍ ማረጋገጫ ከ`chain_id`፣ `epoch`፣ እና የቅርብ ጊዜው የብሎክ ሃሽ ቢኮን በተገኘው ቀኖናዊ ግብአት ላይ ያረጋግጣል። በ ውፅዓት ባይት ዴስክ ከቲኬት ሰሪዎች ጋር መደርደር; ከፍተኛ `committee_size` አባላትን ይመልሳል። አይጸናም።
  - ምላሽ፡ {"epoch": N, "አባላት": [{"መለያ_መታወቂያ": "..." } …]፣ "ጠቅላላ_እጩዎች"፡ M፣ "የተረጋገጠ"፡ K }
  - ማስታወሻዎች፡ መደበኛ = pk በ G1፣ ማስረጃ በG2 (96 ባይት)። ትንሽ = pk በ G2፣ ማስረጃ በG1 (48 ባይት)። ግብዓቶች በጎራ የተከፈሉ ናቸው እና `chain_id` ያካትታሉ።

### የአስተዳደር ነባሪዎች (iroha_config `gov.*`)

በTorii የሚጠቀመው የምክር ቤት ውድቀት በ`iroha_config` በኩል ይለካል፡

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

ተመጣጣኝ አካባቢ ይሽረዋል፡

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` ምንም ምክር ቤት ሳይቆይ ወደ ኋላ የተመለሱ አባላትን ቁጥር ይቆጥባል፣ `parliament_term_blocks` ለዘር መገኛነት ጥቅም ላይ የሚውለውን የኢፖክ ርዝመት ይገልጻል `parliament_eligibility_asset_id` የእጩውን ስብስብ ሲገነባ የትኛው የንብረት ሒሳብ እንደሚቃኝ ይመርጣል።

የአስተዳደር ቪኬ ማረጋገጫ ማለፊያ የለውም፡ የድምጽ መስጫ ማረጋገጫ ሁል ጊዜ `Active` የማረጋገጫ ቁልፍ ከውስጥ ባይት ጋር ይፈልጋል፣ እና አከባቢዎች ማረጋገጫን ለመዝለል በሙከራ-ብቻ መቀየሪያዎች ላይ መተማመን የለባቸውም።

RBAC
- በሰንሰለት ላይ መፈጸም ፈቃዶችን ይፈልጋል፡-
  - ሀሳቦች: `CanProposeContractDeployment{ contract_id }`
  - ቦሎቶች: `CanSubmitGovernanceBallot{ referendum_id }`
  - አፈጻጸም: `CanEnactGovernance`
  - ምክር ቤት አስተዳደር (ወደፊት): `CanManageParliament`የተጠበቁ የስም ቦታዎች
- ብጁ መለኪያ `gov_protected_namespaces` (JSON array of strings) ወደ ተዘረዘሩ የስም ቦታዎች ለማሰማራት የመግቢያ መግቢያን ያስችላል።
- ደንበኞች የተጠበቁ የስም ቦታዎችን ለማሰማራት የግብይት ሜታዳታ ቁልፎችን ማካተት አለባቸው፡-
  - `gov_namespace`፡ የታለመው የስም ቦታ (ለምሳሌ፡ `"apps"`)
  - `gov_contract_id`: በስም ቦታ ውስጥ ያለው ምክንያታዊ የውል መታወቂያ
- `gov_manifest_approvers`፡ አማራጭ JSON የማረጋገጫ መለያ መታወቂያዎች ድርድር። የሌይን ሰነዱ ከአንድ በላይ ምልአተ ጉባኤ ሲያውጅ፣ መግቢያ የግብይቱን ባለስልጣን እና የተዘረዘሩትን ሂሳቦች የማብራራቱን ምልአተ ጉባኤ ለማርካት ይፈልጋል።
- ቴሌሜትሪ አጠቃላይ የመግቢያ ቆጣሪዎችን በ`governance_manifest_admission_total{result}` ያጋልጣል ስለዚህ ኦፕሬተሮች የተሳካላቸው እውቅናዎችን ከ `missing_manifest` ፣ `non_validator_authority` ፣ `quorum_rejected` ፣ `protected_namespace_rejected` ፣ እና `protected_namespace_rejected` ፣ እና I10018NIX
- ቴሌሜትሪ የማስፈጸሚያ መንገዱን በ`governance_manifest_quorum_total{outcome}` (እሴቶች `satisfied` / `rejected`) በኩል ስላለ ኦፕሬተሮች የጎደሉ ማፅደቆችን ኦዲት ማድረግ ይችላሉ።
- መስመሮች በመገለጫዎቻቸው ላይ የታተመውን የስም ቦታ ፍቃድ ዝርዝር ያስገድዳሉ። `gov_namespace`ን የሚያቀናብር ማንኛውም ግብይት `gov_contract_id` ማቅረብ አለበት፣ እና የስም ቦታው በማንፀባረቁ `protected_namespaces` ስብስብ ውስጥ መታየት አለበት። `RegisterSmartContractCode` ያለዚህ ሜታዳታ ማስገባት ጥበቃ ሲነቃ ውድቅ ይደረጋል።
- መግቢያ ለ tuple `(namespace, contract_id, code_hash, abi_hash)` የተረጋገጠ የአስተዳደር ፕሮፖዛል መኖሩን ያስፈጽማል; አለበለዚያ ማረጋገጫው ባልተፈቀደ ስህተት አይሳካም.

የአሂድ ጊዜ ማሻሻያ መንጠቆዎች
- የሌይን መግለጫዎች `hooks.runtime_upgrade` ወደ የአሂድ ጊዜ ማሻሻያ መመሪያዎችን (`ProposeRuntimeUpgrade`፣ `ActivateRuntimeUpgrade`፣ `CancelRuntimeUpgrade`) ሊያውጅ ይችላል።
- መንጠቆ ሜዳዎች;
  - `allow` (ቦል፣ ነባሪ `true`)፡ `false` ጊዜ፣ ሁሉም የአሂድ ማሻሻያ መመሪያዎች ውድቅ ናቸው።
  - `require_metadata` (ቦል፣ ነባሪ `false`)፡ በ`metadata_key` የተገለጸውን የግብይት ሜታዳታ ግቤት ያስፈልጋል።
  - `metadata_key` (ሕብረቁምፊ): የሜታዳታ ስም በ መንጠቆ ተፈጻሚ ነው። ሜታዳታ ሲያስፈልግ ወይም የፈቃድ ዝርዝር ሲኖር የ`gov_upgrade_id` ነባሪዎች።
  - `allowed_ids` (የሕብረቁምፊዎች ድርድር): አማራጭ የሜታዳታ እሴቶች ዝርዝር (ከተከረከመ በኋላ)። የቀረበው ዋጋ ካልተዘረዘረ ውድቅ ያደርጋል።
- መንጠቆው በሚገኝበት ጊዜ፣ ወረፋ መግባት ግብይቱ ወደ ወረፋው ከመግባቱ በፊት የሜታዳታ ፖሊሲን ያስፈጽማል። ከተፈቀደው ዝርዝር ውጪ ዲበ ውሂብ፣ ባዶ እሴቶች ወይም እሴቶች የሚጎድሉ የ`NotPermitted` ስህተት ይፈጥራሉ።
- ቴሌሜትሪ የማስፈጸሚያ ውጤቶችን በ`governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` ይከታተላል።
- መንጠቆውን የሚያረኩ ግብይቶች ሜታዳታ `gov_upgrade_id=<value>` (ወይም በአንጸባራቂ የተገለጸውን ቁልፍ) ከማንኛቸውም የአረጋጋጭ ማጽደቆች ጋር ማካተት አለባቸው።

ምቹ የመጨረሻ ነጥብ
- POST `/v2/gov/protected-namespaces` - `gov_protected_namespaces` በቀጥታ በመስቀለኛ መንገድ ላይ ይተገበራል።
  - ጥያቄ፡ {"ስም ቦታዎች"፡ ["መተግበሪያዎች"፣ "ስርዓት"]}
  ምላሽ፡ {"እሺ"፡ እውነት፡ "ተግባራዊ"፡ 1 }
  - ማስታወሻዎች: ለአስተዳዳሪ / ለሙከራ የታሰበ; ከተዋቀረ የኤፒአይ ማስመሰያ ያስፈልገዋል። ለማምረት፣ ከ`SetParameter(Custom)` ጋር የተፈረመ ግብይት ማስገባትን ይምረጡ።CLI አጋዦች
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - የስም ቦታውን የኮንትራት ምሳሌዎችን ያመጣል እና ያቋረጡ ቼኮች፡-
    - Torii ለእያንዳንዱ `code_hash` ባይትኮድ ያከማቻል፣ እና የእሱ Blake2b-32 መፍጨት ከ`code_hash` ጋር ይዛመዳል።
    - በ`/v2/contracts/code/{code_hash}` ሪፖርቶች ስር የተቀመጠው አንጸባራቂ ከ `code_hash` እና `abi_hash` እሴቶች ጋር የሚዛመድ።
    - መስቀለኛ መንገድ በሚጠቀምበት ተመሳሳይ ፕሮፖዛል-መታወቂያ የተገኘ የፀደቀ የአስተዳደር ፕሮፖዛል ለ`(namespace, contract_id, code_hash, abi_hash)` አለ።
  - የJSON ሪፖርት በአንድ ውል ከ`results[]` ጋር ያወጣል (ጉዳዮች፣ የሰነድ መግለጫ/ ኮድ/የፕሮፖዛል ማጠቃለያ) እና የአንድ መስመር ማጠቃለያ እስካልታፈነ ድረስ (`--no-summary`)።
  - የተጠበቁ የስም ቦታዎችን ለመመርመር ወይም በአስተዳደር ቁጥጥር ስር ያሉ የስራ ፍሰቶችን ለማረጋገጥ ጠቃሚ።
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - ስምምነቶችን ወደተጠበቁ የስም ቦታዎች ሲያስገቡ ጥቅም ላይ የዋለውን የJSON ሜታዳታ አጽም ያወጣል፣ ይህም አማራጭ `gov_manifest_approvers` የሰነድ ምልአተ ጉባኤ ሕጎችን ለማሟላት።
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — የመቆለፊያ ፍንጮች ያስፈልጋሉ `min_bond_amount > 0`፣ እና ማንኛውም የቀረበው ፍንጭ ስብስብ `owner`፣ `amount`፣ እና `duration_blocks` ማካተት አለበት።
  - የቀኖና መለያ መታወቂያዎችን ያረጋግጣል፣ የ32-ባይት መሻሪያ ፍንጮችን ቀኖናዊ ያደርጋል፣ እና ፍንጮቹን ወደ `public_inputs_json` (ከ`--public <path>` ጋር ለተጨማሪ መሻሮች) ያዋህዳል።
  - አጥፊው ​​ከማስረጃ ቁርጠኝነት (የህዝብ ግብአት) እና `domain_tag`፣ `chain_id`፣ እና `election_id` የተገኘ ነው። `--nullifier` በማረጃው ላይ የተረጋገጠው ሲቀርብ ነው።
  - የአንድ መስመር ማጠቃለያው አሁን ከኢ18NI00000208X የተገኘ ቆራጥ የሆነ `CastZkBallot` ከማንኛውም ዲኮድ የተደረጉ ፍንጮች (`owner`፣ `amount`፣ Prometheus፣Sumeragi)ይገኛል።
  - የ CLI ምላሾች `tx_instructions[]`ን ከ `payload_fingerprint_hex` እና ዲኮድ የተደረጉ መስኮችን ያብራራሉ ስለዚህ የታችኛው ዥረት መሳሪያ Norito ዲኮዲንግ ሳይተገበር አፅሙን ማረጋገጥ ይችላል።
  - የመቆለፊያ ፍንጮችን መስጠት መስቀለኛ መንገድ `LockCreated`/`LockExtended` ክስተቶችን ለ ZK ድምጽ መስጫ ወረቀቶች አንድ ጊዜ ወረዳው ተመሳሳይ እሴቶችን ሲያጋልጥ ያስችለዋል።
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` ቀኖናዊ I105 ቃል በቃል ይቀበላል; አማራጭ `@<domain>` ቅጥያ ፍንጮችን ብቻ ነው።
  - ተለዋጭ ስሞች `--lock-amount`/`--lock-duration-blocks` ለስክሪፕት እኩልነት የZK ባንዲራ ስሞችን ያንፀባርቃሉ።
  - የማጠቃለያ ውፅዓት መስተዋቶች `vote --mode zk` ኢንኮድ የተደረገ የመመሪያ የጣት አሻራ እና በሰው ሊነበቡ የሚችሉ የድምጽ መስጫ ቦታዎች (`owner`፣ Norito፣ `duration_blocks`፣`duration_blocks`፣`direction`)፣ፈጣን ማረጋገጫን በማቅረብ።የምሳሌዎች ዝርዝር
- GET `/v2/gov/instances/{ns}` - ለስም ቦታ ንቁ የውል ሁኔታዎችን ይዘረዝራል።
  - የጥያቄ መለኪያዎች;
    - `contains`: ማጣሪያ በ `contract_id` ንዑስ ሕብረቁምፊ (ጉዳይ-የሚነካ)
    - `hash_prefix`፡ በሄክስ ቅድመ ቅጥያ `code_hash_hex` (አነስተኛ ሆሄ) አጣራ
    - `offset` (ነባሪ 0)፣ `limit` (ነባሪ 100፣ ከፍተኛ 10_000)
    - `order`: አንዱ `cid_asc` (ነባሪ)፣ `cid_desc`፣ `hash_asc`፣ `hash_desc`
  ምላሽ፡ {"ስም ቦታ"፡ "ns"፡ "አብነት"፡ [{"contract_id"፡ "..."፣ "code_hash_hex": "..." }, …]፣ "ጠቅላላ"፡ N፣ "ማካካሻ"፡ n፣ "ገደብ"፡ m }
  - SDK አጋዥ፡ `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (ጃቫስክሪፕት) ወይም `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)።

ክፈት ጠረግ (ኦፕሬተር/ኦዲት)
- `/v2/gov/unlocks/stats` ያግኙ
  - ምላሽ፡ {"ቁመት_የአሁኑ"፡ H፣ "ጊዜው_የተጠናቀቀ_መቆለፊያዎች"፡ n፣ "ማጣቀሻ_ጊዜው_ያለፈበት"፡ m፣ "የመጨረሻ_ጠረጋ ቁመት"፡ S }
  ማስታወሻዎች፡- `last_sweep_height` ጊዜው ያለፈባቸው መቆለፊያዎች ተጠርገው የቆዩበትን የቅርቡን የማገጃ ቁመት ያንፀባርቃል። `expired_locks_now` በ `expiry_height <= height_current` የመቆለፊያ መዝገቦችን በመቃኘት ይሰላል።
- POST `/v2/gov/ballots/zk-v1`
  - ጥያቄ (v1-ቅጥ DTO):
    {
      "ስልጣን": "i105...",
      "ሰንሰለት_መታወቂያ": "00000000-0000-0000-0000-00000000000",
      "የግል_ቁልፍ": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "ሥር_ፍንጭ"፡ "0x…64hex?"፣
      "ባለቤት": "i105…?"፣ // ቀኖናዊ አካውንትአይድ (I105 በጥሬው)
      "መጠን": "100?",
      "የቆይታ_ብሎኮች": 6000?,
      "አቅጣጫ": "አዬ|ናይ|መታቀብ?",
      " nullifier": "blake2b32:…64hex?"
    }
  ምላሽ፡ {"እሺ"፡ እውነት፡ "ተቀባይነት ያለው"፡ እውነት፡ "tx_instructions"፡ [{…}] }- POST `/v2/gov/ballots/zk-v1/ballot-proof` (ባህሪ፡ `zk-ballot`)
  - `BallotProof` JSON በቀጥታ ይቀበላል እና `CastZkBallot` አጽም ይመልሳል።
  - ጥያቄ፡-
    {
      "ስልጣን": "i105...",
      "ሰንሰለት_መታወቂያ": "00000000-0000-0000-0000-00000000000",
      "የግል_ቁልፍ": "...?",
      "election_id": "ref-1",
      "ምርጫ": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // ቤዝ64 የZK1 ወይም H2* መያዣ
        "root_hint"፡ ባዶ፣ // አማራጭ ባለ 32-ባይት ሄክስ ህብረቁምፊ (የብቁነት ስር)
        "ባለቤት"፡ ባዶ፣ // አማራጭ ቀኖናዊ AccountId (I105 በጥሬው)
        " nullifier"፡ ባዶ፣ // አማራጭ ባለ 32-ባይት የአስራስድስትዮሽ ሕብረቁምፊ (የማሻሻያ ፍንጭ)
        "መጠን": "100", // አማራጭ የመቆለፊያ መጠን ፍንጭ (አስርዮሽ ሕብረቁምፊ)
        "duration_blocks": 6000, // አማራጭ የመቆለፊያ ቆይታ ፍንጭ
        "direction": "አዎ" // አማራጭ አቅጣጫ ፍንጭ
      }
    }
  ምላሽ፡-
    {
      "እሺ": እውነት,
      "ተቀባይነት ያለው": እውነት,
      "ምክንያት": "የግብይት አጽም መገንባት",
      "tx_instructions": [
        {"wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - ማስታወሻዎች:
    - የአገልጋዩ ካርታ አማራጭ `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` ከድምጽ መስጫ 2500000255X `CastZkBallot`.
    - ለትምህርቱ ክፍያ የፖስታ ባይቶች እንደገና እንደ base64 ተቀምጠዋል።
    - ምላሹ `reason` Torii ድምጽ መስጫ ሲያቀርብ ወደ `submitted transaction` ይቀየራል።
    - ይህ የመጨረሻ ነጥብ የሚገኘው የ `zk-ballot` ባህሪ ሲነቃ ብቻ ነው።

የCastZkBallot ማረጋገጫ ዱካ
- `CastZkBallot` የቀረበውን ቤዝ64 ማረጋገጫ ኮድ ፈትቶ ባዶ ወይም የተበላሹ ጭነቶችን ውድቅ ያደርጋል (`BallotRejected` ከ `invalid or empty proof`)።
- `public_inputs_json` ከተሰጠ, የ JSON ነገር መሆን አለበት; እቃ ያልሆኑ ሸክሞች ውድቅ ናቸው።
- አስተናጋጁ የድምጽ መስጫ ቁልፉን ከሪፈረንደም (`vk_ballot`) ወይም የአስተዳደር ነባሪዎች ይፈታል እና መዝገቡ እንዲኖር ይጠይቃል፣ `Active` እና የመስመር ላይ ባይት ይይዛል።
- የተከማቹ የማረጋገጫ-ቁልፍ ባይቶች በ`hash_vk` እንደገና ታሽገዋል። ማንኛውም ቁርጠኝነት አለመመጣጠን ከማረጋገጡ በፊት አፈፃፀምን ያስወግዳል ከተበላሹ የመመዝገቢያ ግቤቶች (`BallotRejected` ከ `verifying key commitment mismatch` ጋር)።
- የማረጋገጫ ባይት በ `zk::verify_backend` በኩል ወደተመዘገበው ጀርባ ይላካል; ልክ ያልሆኑ የጽሑፍ ግልባጮች እንደ `BallotRejected` ከ `invalid proof` እና መመሪያው በቆራጥነት አልተሳካም።
- ማስረጃው የድምፅ መስጫ ቁርጠኝነትን እና የብቁነት ስር እንደ የህዝብ ግብአቶች ማጋለጥ አለበት; ሥሩ ከምርጫው `eligible_root` ጋር መዛመድ አለበት፣ እና የተገኘው nullifier ከማንኛውም ፍንጭ ጋር መዛመድ አለበት።
- የተሳካላቸው ማረጋገጫዎች `BallotAccepted` ያመነጫሉ; የተባዙ ውድቀቶች፣ የቆዩ ብቁነት ስሮች ወይም የተቆለፉ ሪግሬሽን ቀደም ሲል በዚህ ሰነድ ውስጥ የተገለጹትን ውድቅ ምክንያቶች ማፍራታቸውን ቀጥለዋል።

## አረጋጋጭ መጥፎ ባህሪ እና የጋራ መግባባት

### መጨፍጨፍ እና ማሰር የስራ ፍሰትአረጋጋጭ ፕሮቶኮሉን በጣሰ ቁጥር Norito-የተመሰጠረ `Evidence` ያወጣል። እያንዳንዱ የመጫኛ ጭነት በውስጠ-ማስታወሻ `EvidenceStore` ውስጥ ያርፋል እና ካልታየ በ WSV በሚደገፈው `consensus_evidence` ካርታ ውስጥ ገብቷል። ከ`sumeragi.npos.reconfig.evidence_horizon_blocks` (ነባሪ `7 200` ብሎኮች) የቆዩ መዛግብት ውድቅ ተደርገዋል ስለዚህ ማህደሩ እንደታሰረ ይቆያል፣ ነገር ግን ውድቀቱ ለኦፕሬተሮች ገብቷል። በአድማስ ውስጥ ያሉ ማስረጃዎች የጋራ ስምምነት ማስተናገጃ ደንብን (`mode_activation_height requires next_mode to be set in the same block`)፣ የማግበር መዘግየት (`sumeragi.npos.reconfig.activation_lag_blocks`፣ ነባሪ `1`) እና የመቁረጥ መዘግየትን (`sumeragi.npos.reconfig.slashing_delay_blocks`፣ ነባሪ I100000283X፣ ነባሪ I1828000000283X፣ ነባሪ I10280000000283X፣ ነባሪ I182800000000000000000 ይችላሉ

የታወቁ ጥፋቶች አንድ ለአንድ ወደ `EvidenceKind` ካርታ; አድልዎዎቹ የተረጋጉ እና በመረጃ ሞዴል የሚተገበሩ ናቸው፡-

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- ** DoublePrepare/DoubleCommit *** - አረጋጋጩ ለተመሳሳይ `(phase,height,view,epoch)` tuple የሚጋጩ ሃሾችን ፈርሟል።
- **ልክ ያልሆነQc** — ሰብሳቢው ቅርጹን የሚወስኑ ቼኮች (ለምሳሌ ባዶ ፈራሚ ቢትማፕ) ያልተሳካ የምስክር ወረቀት ተናግሯል።
- ** ልክ ያልሆነ ፕሮፖዛል *** - አንድ መሪ ​​መዋቅራዊ ማረጋገጫን (ለምሳሌ የተቆለፈውን ሰንሰለት ህግ የሚጥስ) ብሎክ አቅርቧል።
- ** ሳንሱር *** - የተፈረሙ የማስረከቢያ ደረሰኞች በጭራሽ ያልታሰበ/የተፈጸመ ግብይት ያሳያሉ።

የVRF ቅጣቶች ከ`activation_lag_blocks` በኋላ (ወንጀለኞች ታስረዋል) በኋላ ተፈጻሚ ይሆናሉ። የአስተዳደር ቅጣቱን ካልሰረዘው በስተቀር የስምምነት ቅነሳ ከ `slashing_delay_blocks` መስኮት በኋላ ብቻ ይተገበራል።

ኦፕሬተሮች እና መሳሪያዎች በሚከተለው መንገድ ክፍያ ጭነቶችን መፈተሽ እና እንደገና ማሰራጨት ይችላሉ።

- Torii፡ `GET /v2/sumeragi/evidence` እና `GET /v2/sumeragi/evidence/count`።
- CLI፡ `iroha ops sumeragi evidence list`፣ `… count`፣ እና `… submit --evidence-hex <payload>`።

አስተዳደር የማስረጃውን ባይት እንደ ቀኖናዊ ማረጋገጫ መያዝ አለበት፡-

1. ** ክፍያውን ይሰብስቡ *** ከማለቁ በፊት። ጥሬውን Norito ባይት ከፍታ/እይታ ዲበ ዳታ ጋር በማህደር ያስቀምጡ።
2. ** ካስፈለገ ይሰርዙ *** `CancelConsensusEvidencePenalty` ከማስረጃ ጭነት ጋር `slashing_delay_blocks` ከማለፉ በፊት; መዝገቡ `penalty_cancelled` እና `penalty_cancelled_at_height` ምልክት ተደርጎበታል፣ እና ምንም መቆራረጥ አይተገበርም።
3. ** ቅጣቱን ደረጃ ይስጡ** የሚከፈለውን ጭነት በሪፈረንደም ወይም በሱዶ መመሪያ (ለምሳሌ `Unregister::peer`) በማካተት። አፈፃፀም ክፍያውን እንደገና ያረጋግጣል; የተበላሸ ወይም የቆየ ማስረጃ በቆራጥነት ውድቅ ይደረጋል።
4. **የቀጣይ ቶፖሎጂን መርሐግብር ያውጡ** ስለዚህ አፀያፊው ወዲያውኑ እንደገና መቀላቀል አይችልም። የተለመደው ፍሰቶች ወረፋ `SetParameter(Sumeragi::NextMode)` እና `SetParameter(Sumeragi::ModeActivationHeight)` ከተዘመነው የስም ዝርዝር ጋር።
5. **የኦዲት ውጤቶች** በ`/v2/sumeragi/evidence` እና `/v2/sumeragi/status` በኩል ማስረጃው የላቀ መሆኑን ለማረጋገጥ እና አስተዳደር መወገድን ማውጣቱን ያረጋግጣል።

### የጋራ መግባባት ቅደም ተከተል

የጋራ መግባባት የወጪ አረጋጋጭ ስብስብ አዲሱ ስብስብ ሃሳብ ማቅረብ ከመጀመሩ በፊት የድንበሩን እገዳ እንደሚያጠናቅቅ ዋስትና ይሰጣል። የሩጫ ጊዜው ደንቡን በተጣመሩ መለኪያዎች ያስፈጽማል፡-- `SumeragiParameter::NextMode` እና `SumeragiParameter::ModeActivationHeight` **በተመሳሳይ ብሎክ** ውስጥ መፈፀም አለባቸው። `mode_activation_height` ማሻሻያውን ከተሸከመው የማገጃ ቁመት በጥብቅ የሚበልጥ መሆን አለበት፣ ይህም ቢያንስ የአንድ-ብሎክ መዘግየት ነው።
- `sumeragi.npos.reconfig.activation_lag_blocks` (ነባሪ `1`) ዜሮ መዘግየት እጅ-መጥፋትን የሚከላከል የውቅር ጠባቂ ነው፡
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ነባሪ `259200`) የአስተዳደር ስምምነት ከማመልከታቸው በፊት ቅጣቶችን እንዲሰርዝ ያደርጋል።

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- የሩጫ ጊዜ እና CLI ደረጃቸውን የጠበቁ መለኪያዎችን በ`/v2/sumeragi/params` እና `iroha --output-format text ops sumeragi params` በኩል ያጋልጣሉ፣ስለዚህ ኦፕሬተሮች የማግበሪያ ከፍታዎችን እና የማረጋገጫ ዝርዝሮችን ማረጋገጥ ይችላሉ።
- የአስተዳደር አውቶማቲክ ሁልጊዜ;
  1. በማስረጃ የተደገፈውን የማስወገድ (ወይም ወደነበረበት መመለስ) ውሳኔ ያጠናቅቁ።
  2. የክትትል መልሶ ማዋቀርን በ`mode_activation_height = h_current + activation_lag_blocks` ወረፋ ያድርጉ።
  3. `/v2/sumeragi/status` `effective_consensus_mode` እስኪገለበጥ በሚጠበቀው ከፍታ ላይ ይቆጣጠሩ።

አረጋጋጮችን የሚያዞር ወይም መጨፍጨፍን የሚተገበር ማንኛውም ስክሪፕት ** ዜሮ-ማግበርን መሞከር የለበትም ወይም የእጅ ማጥፊያ መለኪያዎችን መተው የለበትም። እንደነዚህ ያሉ ግብይቶች ውድቅ ይደረጋሉ እና አውታረ መረቡን በቀድሞው ሁነታ ይተዉታል.

## ቴሌሜትሪ ንጣፎች

- Prometheus ሜትሪክስ ወደ ውጭ መላክ የአስተዳደር እንቅስቃሴ፡-
  - `governance_proposals_status{status}` (መለኪያ) የፕሮፖዛል ቆጠራዎችን በሁኔታ ይከታተላል።
  - `governance_protected_namespace_total{outcome}` (ቆጣሪ) ጭማሪዎች የተከለለ የስም ቦታ መግቢያ ሲፈቅድ ወይም ውድቅ ሲያደርግ።
  - `governance_manifest_activations_total{event}` (ቆጣሪ) አንጸባራቂ ማስገባቶችን (`event="manifest_inserted"`) እና የስም ቦታ ማሰሪያዎችን (`event="instance_bound"`) ይመዘግባል።
- `/status` የውሳኔ ሃሳቡን ብዛት የሚያንፀባርቅ የ`governance` ነገርን ያጠቃልላል ፣የተከለለ የስም ቦታ ድምርን ሪፖርት ማድረግ እና የቅርብ ጊዜ አንጸባራቂ እንቅስቃሴዎችን (ስም ቦታ ፣ የውል መታወቂያ ፣ ኮድ/ABI ሃሽ ፣ የማገጃ ቁመት ፣ የማግበር ጊዜ ማህተም) ያጠቃልላል። ኦፕሬተሮች የዘመኑ ማኒፌክቶችን እና የተጠበቁ የስም ቦታ በሮች መተግበራቸውን ለማረጋገጥ ይህንን መስክ ሊጠይቁ ይችላሉ።
- A Grafana አብነት (`docs/source/grafana_governance_constraints.json`) እና
  ቴሌሜትሪ runbook በ `telemetry.md` ውስጥ ማንቂያዎችን ለተጣበቀ እንዴት ሽቦ ማድረግ እንደሚቻል ያሳያል
  ፕሮፖዛል፣ የጠፉ አንጸባራቂ ማነቃቂያዎች፣ ወይም ያልተጠበቀ የተጠበቀ-ስም ቦታ
  በአሂድ ጊዜ ማሻሻያዎች ወቅት አለመቀበል።