---
lang: my
direction: ltr
source: docs/source/governance_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eea277d4aae6a7b29b5be539ef9d8e63948ccdd89a152de5af4a3cb357fe543a
source_last_modified: "2026-01-22T16:26:46.569356+00:00"
translation_last_reviewed: 2026-02-07
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
---

အခြေအနေ- အုပ်ချုပ်မှု အကောင်အထည်ဖော်ရေး လုပ်ငန်းများကို ပူးတွဲဆောင်ရွက်ရန် မူကြမ်း/ပုံကြမ်း။ အကောင်အထည်ဖော်နေစဉ်အတွင်း ပုံစံများ ပြောင်းလဲနိုင်သည်။ Determinism နှင့် RBAC မူဝါဒများသည် စံကန့်သတ်ချက်များဖြစ်သည်။ Torii သည် `authority` နှင့် `private_key` ကို ပံ့ပိုးပေးသောအခါတွင် ငွေပေးငွေယူများကို လက်မှတ်ထိုး/တင်သွင်းနိုင်သည်၊ သို့မဟုတ်ပါက clients များသည် `/transaction` သို့ တည်ဆောက်ပြီး တင်ပြနိုင်သည်။

အရေးကြီးသည်- ကျွန်ုပ်တို့သည် အမြဲတမ်းကောင်စီ သို့မဟုတ် "ပုံသေ" အုပ်ချုပ်မှုစာရင်းကို ပေးပို့ခြင်းမပြုပါ။ ဘောက်စ်အတွင်းမှ၊ ကောင်စီသည် အလွတ်/ဆိုင်းငံ့နေသည့် အခြေအနေသို့ ပြန်ပို့ပေးသည် သို့မဟုတ် ဖွင့်ထားသည့်အခါတွင် သတ်မှတ်ထားသော ကန့်သတ်ဘောင်များ (အစုရှယ်ယာပိုင်ဆိုင်မှု၊ သက်တမ်း၊ ကော်မတီအရွယ်အစား) တို့မှ အဆုံးအဖြတ်ပေးသော ဆုတ်ယုတ်မှုတစ်ခုကို ရယူနိုင်သည်။ အော်ပရေတာများသည် အုပ်ချုပ်မှုလမ်းကြောင်းများမှတစ်ဆင့် ၎င်းတို့၏ကိုယ်ပိုင်စာရင်းကို ဆက်လက်ထားရှိရမည်ဖြစ်ပါသည်။ ဤသိုလှောင်ခန်းတွင် လျှို့ဝှက်သော့၊ သို့မဟုတ် အခွင့်ထူးခံကောင်စီအကောင့်တွင် ပေါင်းထည့်ထားသော multisig၊ လျှို့ဝှက်သော့၊ သို့မဟုတ် အခွင့်ထူးခံကောင်စီအကောင့်မရှိပါ။

ခြုံငုံသုံးသပ်ချက်
- အဆုံးမှတ်များအားလုံးသည် JSON ကို ပြန်ပေးသည်။ အရောင်းအ၀ယ်-ထုတ်လုပ်သည့် စီးဆင်းမှုများအတွက်၊ တုံ့ပြန်မှုများတွင် `tx_instructions` — တစ်ခု သို့မဟုတ် တစ်ခုထက်ပိုသော ညွှန်ကြားချက်အရိုးစုများ၏ array တစ်ခုပါဝင်သည်-
  - `wire_id`- ညွှန်ကြားချက်အမျိုးအစားအတွက် မှတ်ပုံတင်စနစ်
  - `payload_hex`: Norito payload bytes (hex)
- အကယ်၍ `authority` နှင့် `private_key` (သို့မဟုတ် မဲ DTO များပေါ်တွင် `private_key`) ကို Torii ဆိုင်းဘုတ်များတင်ပြီး ငွေပေးငွေယူတင်ပြပြီး `tx_instructions` ကို ဆက်လက်ပေးပို့ပါ။
- မဟုတ်ပါက၊ clients များသည် ၎င်းတို့၏ အခွင့်အာဏာနှင့် chain_id ကို အသုံးပြု၍ SignedTransaction ကိုစုပေါင်းပြီး `/transaction` သို့ လက်မှတ်ထိုးပြီး POST လုပ်ပါ။
- SDK လွှမ်းခြုံမှု-
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` သည် `GovernanceProposalResult` (ပုံမှန်အခြေအနေ/အမျိုးအစားအကွက်များ), `ToriiClient.get_governance_referendum_typed` ပြန်ပေးသည် `GovernanceReferendumResult`, Prometheus, Prometheus `ToriiClient.get_governance_locks_typed` သည် `GovernanceLocksResult` ကို ပြန်ပေးသည်၊ `ToriiClient.get_governance_unlock_stats_typed` သည် `GovernanceUnlockStats` ကို ပြန်ပေးသည်၊ နှင့် `ToriiClient.list_governance_instances_typed` သည် `GovernanceInstancesPage` ကို ပြန်ပေးသည်၊ REUTERS တွင် စာရိုက်ဝင်ရောက်ခွင့်ကို အုပ်ချုပ်မှုနမူနာများဖြင့် ပြဌာန်းထားသည်။
- Python ပေါ့ပါးသောကလိုင်းယင့် (`iroha_torii_client`): `ToriiClient.finalize_referendum` နှင့် `ToriiClient.enact_proposal` ပြန်ရိုက်ထားသော `GovernanceInstructionDraft` အစုအဝေးများ (Torii အရိုးစု I1805008X ကို ရှောင်ရှားခြင်း)၊ scripts များသည် Finalize/ Enact flows ကိုရေးဖွဲ့သည်။
- JavaScript (`@iroha/iroha-js`): `ToriiClient` သည် အဆိုပြုချက်များ၊ ရည်ညွှန်းချက်၊ မှတ်တမ်းများ၊ သော့ခတ်မှုများ၊ လော့ခ်ဖွင့်ခြင်း ကိန်းဂဏန်းများနှင့် ယခု `listGovernanceInstances(namespace, options)` နှင့် council endpoints (Sumeragi) `governancePersistCouncil`၊ `getGovernanceCouncilAudit`) ထို့ကြောင့် Node.js ဖောက်သည်များသည် `/v1/gov/instances/{ns}` ကို paginate နိုင်ပြီး VRF ကျောထောက်နောက်ခံပြုထားသော အလုပ်အသွားအလာများကို လက်ရှိစာချုပ်-ဥပမာစာရင်းနှင့်အတူ မောင်းနှင်နိုင်သည်။ `governanceFinalizeReferendumTyped` နှင့် `governanceEnactProposalTyped` သည် ဖွဲ့စည်းတည်ဆောက်ထားသော မူကြမ်းကို အမြဲပြန်ပေးခြင်းဖြင့် Python helpers များကို ထင်ဟပ်စေသည် (Torii မှ `204 No Content` တွင် automation 8000000X မတိုင်မီ NI000000000000000000 တွင် NI0000000000 မတိုင်မီ automation ခွဲထွက်ခြင်းမှ `204 No Content` မတိုင်မီ automation ခွဲထွက်ခြင်းမပြုမီ အစပျိုးသည်။ ယခု `getGovernanceLocksTyped` သည် `404 Not Found` တုံ့ပြန်မှုများကို `{found: false, locks: {}, referendum_id: <id>}` သို့ ပုံမှန်ဖြစ်အောင် ပြုလုပ်ပေးသောကြောင့် JS ခေါ်ဆိုသူများသည် ဆန္ဒခံယူပွဲတစ်ခုတွင် သော့မရှိသည့်အခါ Python helper ကဲ့သို့ ပုံစံတူရလဒ်ကို ရရှိမည်ဖြစ်သည်။

အဆုံးမှတ်များ- POST `/v1/gov/proposals/deploy-contract`
  - တောင်းဆိုချက် (JSON):
    {
      "namespace": "အက်ပ်များ",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:…" | "…64 hex"၊
      "abi_hash": "blake2b32:…" | "…64 hex"၊
      "abi_version": "1",
      "window": { "lower": 12345၊ "upper": 12400 }၊
      "authority": "ih58...?"၊
      "private_key": "…?"
    }
  - တုံ့ပြန်မှု (JSON):
    { "ok": true၊ "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
  - အတည်ပြုခြင်း- ပေးထားသော `abi_version` အတွက် nodes များကို canonicalise `abi_hash` နှင့် မကိုက်ညီမှုများကို ငြင်းပယ်ပါ။ `abi_version = "v1"` အတွက် မျှော်မှန်းတန်ဖိုးမှာ `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ဖြစ်သည်။

စာချုပ်များ API (အသုံးချရန်)
- POST `/v1/contracts/deploy`
  - တောင်းဆိုချက်- { "authority": "ih58...", "private_key": "…", "code_b64": "…" }
  - အပြုအမူ- IVM ပရိုဂရမ်ကိုယ်ထည်မှ `code_hash` ကို တွက်ချက်ပြီး `abi_version` ခေါင်းစီးမှ `abi_hash` ၊ ထို့နောက် `RegisterSmartContractCode` (manifest) နှင့် I10080NI `.to` bytes) `authority` ကိုယ်စား။
  - တုံ့ပြန်ချက်- { "ok": true, "code_hash_hex": "…", "abi_hash_hex": "…" }
  ဆက်စပ်-
    - `/v1/contracts/code/{code_hash}` → သိမ်းဆည်းထားသော မန်နီးဖက်စ်ကို ပြန်ရယူပါ။
    - `/v1/contracts/code-bytes/{code_hash}` ကိုရယူပါ → `{ code_b64 }` ကို ပြန်ပေးသည်
- POST `/v1/contracts/instance`
  - တောင်းဆိုချက်- { "authority": "ih58...", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "…" }
  - အပြုအမူ- ပံ့ပိုးပေးထားသော bytecode ကို အသုံးချပြီး `ActivateContractInstance` မှတစ်ဆင့် `(namespace, contract_id)` မြေပုံကို ချက်ချင်း အသက်သွင်းသည်။
  - တုံ့ပြန်မှု- { "ok": true၊ "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "…", "abi_hash_hex": "…" }

Alias ဝန်ဆောင်မှု
- POST `/v1/aliases/voprf/evaluate`
  - တောင်းဆိုချက်- { "blinded_element_hex": "…" }
  - တုံ့ပြန်ချက်- { "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }
    - `backend` သည် အကဲဖြတ်သူ၏ အကောင်အထည်ဖော်မှုကို ထင်ဟပ်စေသည်။ ကာလတန်ဖိုး- `blake2b512-mock`။
  - မှတ်စုများ- ဒိုမိန်းခွဲထုတ်ခြင်း `iroha.alias.voprf.mock.v1` ဖြင့် Blake2b512 ကိုအသုံးပြုသည့် ခွဲခြားသတ်မှတ်ပုံသဏ္ဍန်အကဲဖြတ်သူ။ ထုတ်လုပ်မှု VOPRF ပိုက်လိုင်းကို Iroha မှတဆင့် ကြိုးမသွယ်မချင်း စမ်းသပ်ကိရိယာအတွက် ရည်ရွယ်သည်။
  - အမှားအယွင်းများ- ပုံစံမမှန်သော hex ထည့်သွင်းမှုတွင် HTTP `400`။ Torii သည် Norito `ValidationFail::QueryFailed::Conversion` စာအိတ်ကို ဒီကုဒ်ဒါအမှား မက်ဆေ့ဂျ်ဖြင့် ပြန်ပေးသည်။
- POST `/v1/aliases/resolve`
  - တောင်းဆိုချက်- { "alias": "GB82 WEST 1234 5698 7654 32" }
  - တုံ့ပြန်မှု- { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - မှတ်ချက်များ- ISO တံတားဖွင့်ချိန် အဆင့်သတ်မှတ်မှု (`[iso_bridge.account_aliases]` တွင် `iroha_config`) လိုအပ်သည်။ Torii သည် ရှာဖွေမှုမပြုမီ အဖြူကွက်နှင့် အပေါ်ဖုံးကို ဖယ်ထုတ်ခြင်းဖြင့် နာမည်များကို ပုံမှန်ဖြစ်စေသည်။ alias မရှိတော့သည့်အခါ 404 နှင့် ISO တံတားဖွင့်ချိန်ကို ပိတ်ထားသောအခါ 503 ကို ပြန်ပေးသည်။
- POST `/v1/aliases/resolve_index`
  - တောင်းဆိုချက်- { "index": 0 }
  - တုံ့ပြန်မှု- { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - မှတ်ချက်- Alias indices များကို configuration order (0-based) မှ ဆုံးဖြတ်သတ်မှတ်ပေးပါသည်။ ဖောက်သည်များသည် အတုအယောင် သက်သေခံချက်ဖြစ်ရပ်များအတွက် စာရင်းစစ်လမ်းကြောင်းများတည်ဆောက်ရန် အော့ဖ်လိုင်းတွင် တုံ့ပြန်မှုများကို ကက်ရှ်လုပ်နိုင်ပါသည်။Code Size Cap
- စိတ်ကြိုက်ဘောင်- `max_contract_code_bytes` (JSON u64)
  - ကွင်းဆက်စာချုပ်ကုဒ်သိုလှောင်မှုအတွက် အများဆုံးခွင့်ပြုထားသောအရွယ်အစား (ဘိုက်များ) ကို ထိန်းချုပ်သည်။
  - ပုံသေ- 16 MiB `.to` ပုံအလျားသည် အဖုံးကို ကျော်လွန်နေသောအခါ Nodes များသည် `RegisterSmartContractBytes` ကို ငြင်းဆိုသည်။
  - အော်ပရေတာများသည် `SetParameter(Custom)` ကို `id = "max_contract_code_bytes"` နှင့် ဂဏန်းပေးချေမှုတစ်ခုတင်ပြခြင်းဖြင့် ချိန်ညှိနိုင်သည်။

- POST `/v1/gov/ballots/zk`
  - တောင်းဆိုချက်- { "authority": "ih58...", "private_key": "…?", "chain_id": "…", "election_id": "e1", "proof_b64": "…", " public": {…} }
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }
  - မှတ်စုများ
    - ဆားကစ်၏ အများသူငှာ သွင်းအားစုများတွင် `owner`၊ `amount`၊ နှင့် `duration_blocks` ပါ၀င်ပြီး အထောက်အထားသည် configured VK ကို ဆန့်ကျင်သောအခါ၊ node သည် Sumeragi အတွက် အုပ်ချုပ်မှုလော့ခ်တစ်ခုကို ဖန်တီး သို့မဟုတ် တိုးချဲ့သည် ဦးတည်ချက်သည် အရိပ်အမြွက်မပြပါက (`unknown`) ကို ဝှက်ထားဆဲဖြစ်သည်။ ပမာဏ/သက်တမ်း ကုန်ဆုံးမှုကိုသာ အပ်ဒိတ်လုပ်ထားသည်။ ပြန်လည်မဲများသည် မိုနိုတိုနစ်ဖြစ်သည်- ပမာဏနှင့် သက်တမ်းကုန်ဆုံးမှုသည်သာ တိုးလာသည် (node ​​သည် အမြင့်ဆုံး(amount, prev.amount) နှင့် max(expiry, prev.expiry)))။
    - မည်သည့်သော့ခတ်မှုအရိပ်အမြွက်ကိုမဆိုပေးသောအခါ၊ မဲသည် `owner`၊ `amount`၊ နှင့် `duration_blocks` ပေးရမည်။ တစ်စိတ်တစ်ပိုင်း အရိပ်အမြွက်များကို ပယ်ချပါသည်။ `min_bond_amount > 0` တွင် လော့ခ်ချရန် အရိပ်အမြွက်များ လိုအပ်ပါသည်။
    - ပမာဏ သို့မဟုတ် သက်တမ်းကုန်ဆုံးရန် ကြိုးပမ်းသော ZK ပြန်လည်မဲများသည် `BallotRejected` အဖြေရှာခြင်းများဖြင့် ဆာဗာဘက်တွင် ပယ်ချပါသည်။
    - စာချုပ်အကောင်အထည်ဖော်မှုတွင် `SubmitBallot` ကိုမခေါ်ယူမီ `ZK_VOTE_VERIFY_BALLOT` ကိုခေါ်ဆိုရပါမည်။ အိမ်ရှင်များသည် တစ်ချက်ချက် လက်ကိုင်ကို တွန်းအားပေးသည်။

- POST `/v1/gov/ballots/plain`
  - တောင်းဆိုချက်- { "authority": "ih58...", "private_key": "…?", "chain_id": "…", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "yetain"}|A
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }
  - မှတ်စုများ- ပြန်လည်မဲပေးခြင်းသည် သက်တမ်းတိုးခြင်းသာဖြစ်သည် — မဲအသစ်သည် ရှိပြီးသားလော့ခ်၏ပမာဏ သို့မဟုတ် သက်တမ်းကုန်ဆုံးမှုကို လျှော့ချ၍မရပါ။ `owner` သည် ငွေပေးငွေယူလုပ်ပိုင်ခွင့်အာဏာနှင့် တူညီရပါမည်။ အနည်းဆုံးကြာချိန်သည် `conviction_step_blocks` ဖြစ်သည်။- POST `/v1/gov/finalize`
  - တောင်းဆိုချက်- { "referendum_id": "r1", "proposal_id": "…64hex", "authority": "ih58...?", "private_key": "…?" }
  - တုံ့ပြန်ချက်- { "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "…" }] }
  - ကွင်းဆက်အကျိုးသက်ရောက်မှု (လက်ရှိ scaffold): အတည်ပြုထားသော ဖြန့်ကျက်အဆိုပြုချက်ကို အတည်ပြုခြင်းသည် `code_hash` ဖြင့် သော့ခတ်ထားသော အနည်းဆုံး `code_hash` ဖြင့် သော့ခတ်ထားသော `abi_hash` ကို ထည့်သွင်းပြီး အဆိုပြုချက်ကို အတည်ပြုကြောင်း အမှတ်အသားပြုပါသည်။ အခြား `abi_hash` နှင့် `code_hash` အတွက် ထင်ရှားချက်တစ်ခု ရှိနှင့်ပြီးဖြစ်ပါက၊ အတည်ပြုချက်ကို ပယ်ချပါသည်။
  - မှတ်စုများ
    - ZK ရွေးကောက်ပွဲများအတွက်၊ စာချုပ်လမ်းကြောင်းများသည် `FinalizeElection` ကိုမလုပ်ဆောင်မီ `ZK_VOTE_VERIFY_TALLY` သို့ခေါ်ဆိုရပါမည်။ အိမ်ရှင်များသည် တစ်ချက်ချက် လက်ကိုင်ကို တွန်းအားပေးသည်။ `FinalizeReferendum` သည် ZK ၏ ဆန္ဒခံယူမှုကို ပယ်ချသည် ။
    - `h_end` တွင် အလိုအလျောက်ပိတ်သည် ရိုးရှင်းသောဆန္ဒခံယူခြင်းအတွက်သာ အတည်ပြု/ပယ်ချခံရသည် ။ ZK ၏ဆန္ဒခံယူချက်ကို အပြီးသတ်စာရင်းတင်သွင်းပြီး `FinalizeReferendum` ကို အပြီးသတ်သည်အထိ ပိတ်ထားသည်။
    - မဲစာရင်းစစ်ဆေးမှုများသည် approve+reject ကိုသာအသုံးပြုသည်။ မဲဆန္ဒရှင်စာရင်းတွင် ရှောင်ရန် မပါဝင်ပါ။

- POST `/v1/gov/enact`
  - တောင်းဆိုချက်- { "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58...?", "private_key": "…?" }
  - တုံ့ပြန်ချက်- { "ok": true၊ "tx_instructions": [{ "wire_id": "…EnactReferendum", "payload_hex": "…" }] }
  - မှတ်စုများ- Torii သည် `authority`/`private_key` ကို ပံ့ပိုးပေးသောအခါတွင် လက်မှတ်ရေးထိုးထားသော ငွေပေးငွေယူကို တင်သွင်းပါသည်။ မဟုတ်ပါက ၎င်းသည် ဖောက်သည်များအား လက်မှတ်ရေးထိုးပြီး တင်ပြရန် အရိုးစုကို ပြန်ပေးသည်။ အကြိုဓာတ်ပုံသည် ရွေးချယ်နိုင်ပြီး လက်ရှိတွင် အချက်အလက်ဖြစ်သည်။

- `/v1/gov/proposals/{id}` ရယူပါ။
  - လမ်းကြောင်း `{id}`- အဆိုပြုချက် id hex (64 လုံး)
  - တုံ့ပြန်ချက်- { "found": bool, "proposal": { … }? }

- `/v1/gov/locks/{rid}` ရယူပါ။
  - လမ်းကြောင်း `{rid}`- ဆန္ဒခံယူပွဲ id စာကြောင်း
  - တုံ့ပြန်ချက်- { "found": bool, "referendum_id": "rid", "locks": { … }? }

- `/v1/gov/council/current` ရယူပါ။
  - တုံ့ပြန်ချက်- { "epoch": N၊ "အဖွဲ့ဝင်များ"- [{ "account_id": "…" }, …] }
  - မှတ်စုများ- လက်ရှိရှိနေသည့် ကောင်စီကို ပြန်ပေးသည်။ သို့မဟုတ်ပါက သတ်မှတ်ထားသော အစုရှယ်ယာပိုင်ဆိုင်မှုနှင့် ကန့်သတ်ချက်များကို အသုံးပြု၍ အဆုံးအဖြတ်ပေးသည့် ဆုတ်ယုတ်မှုတစ်ခု (VRF သတ်မှတ်ချက်ကို မှန်ချပ်များပေါ်မှ VRF အထောက်အထားများ ဆက်လက်တည်ရှိနေသည်အထိ)။

- POST `/v1/gov/council/derive-vrf` (အင်္ဂါရပ်- gov_vrf)
  - တောင်းဆိုချက်- { "committee_size": 21၊ "epoch": 123? , "လျှောက်ထားသူများ"- [{ "account_id": "…", "မူကွဲ"- "ပုံမှန်|အသေးစား", "pk_b64": "…", "proof_b64": "…" }, …] }
  - အပြုအမူ- `chain_id`၊ `epoch` နှင့် နောက်ဆုံးဘလောက် hash beacon တို့မှ ဆင်းသက်လာသော canonical input ကိုဆန့်ကျင်သော ကိုယ်စားလှယ်တစ်ဦးစီ၏ VRF သက်သေကို စစ်ဆေးပါ။ Tiebreakers ဖြင့် အထွက် bytes desc ဖြင့် စီပါ။ ထိပ်တန်း `committee_size` အဖွဲ့ဝင်များကို ပြန်ပေးသည်။ မတည်မြဲပါဘူး။
  - တုံ့ပြန်ချက်- { "epoch": N၊ "အဖွဲ့ဝင်များ"- [{ "account_id": "…" } …], "total_candidates": M, "verified": K }
  - မှတ်စုများ- G1 တွင် ပုံမှန် = pk၊ G2 (96 bytes) ဖြင့် အထောက်အထား။ Small = G2 တွင် pk၊ G1 (48 bytes) ဖြင့် အထောက်အထား။ ထည့်သွင်းမှုများကို ဒိုမိန်း-ခြားထားပြီး `chain_id` ပါဝင်သည်။

### အုပ်ချုပ်မှုပုံသေများ (iroha_config `gov.*`)

ဆက်ရှိနေသောစာရင်းစာရင်းမရှိသောအခါ Torii မှအသုံးပြုသော council fallback ကို `iroha_config` မှတဆင့်ကန့်သတ်ထားသည်-```toml
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
  voting_asset_id = "xor#sora"         # governance bond asset (Sora Nexus default)
  min_bond_amount = 150                # smallest units of voting_asset_id
  bond_escrow_account = "ih58..."
  slash_receiver_account = "ih58..."
  slash_double_vote_bps = 0            # percentage (basis points) to slash on double-vote attempts
  slash_invalid_proof_bps = 0          # percentage (basis points) to slash on invalid ballot proofs
  slash_ineligible_proof_bps = 0       # percentage (basis points) to slash on stale/invalid eligibility proofs
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

တူညီသောပတ်ဝန်းကျင်ကို အစားထိုးသည်-

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_VOTING_ASSET_ID=xor#sora
GOV_MIN_BOND_AMOUNT=150
GOV_BOND_ESCROW_ACCOUNT=ih58...
GOV_SLASH_RECEIVER_ACCOUNT=ih58...
GOV_SLASH_DOUBLE_VOTE_BPS=2500
GOV_SLASH_INVALID_PROOF_BPS=5000
GOV_SLASH_INELIGIBLE_PROOF_BPS=1500
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

Sora Nexus မူရင်း- `voting_asset_id` ၏ `min_bond_amount` ၏ မဲများကို လော့ခ်ချသည်
escrow အကောင့်ကို စီစဉ်သတ်မှတ်ထားသည်။ မဲများကျလာသည့်အခါ သော့ခတ်မှုများကို ဖန်တီး သို့မဟုတ် သက်တမ်းတိုးသည်။
သက်တမ်းကုန်ဆုံးရက်တွင်ထုတ်ပြန်ခဲ့သည် နှောင်ကြိုးဘဝစက်ဝိုင်းကို `governance_bond_events_total` မှတဆင့် ထုတ်လွှတ်သည်။
တယ်လီမီတာ (သော့ခတ်_ဖန်တီးထား|သော့ခတ်_တိုးချဲ့|သော့ခတ်ပြီးသော့ဖွင့်ထား|သော့ခတ်_ခုတ်ထစ်|သော့ခတ်_ပြန်လည်ပြင်ဆင်ထားသည်)။

`parliament_committee_size` သည် ကောင်စီကိုဆက်လက်မတည်မြဲသောအခါတွင် ပြန်ပေးသည့်အဖွဲ့ဝင်အရေအတွက်ကို ဖုံးအုပ်ထားသည်၊ `parliament_term_blocks` သည် မျိုးစေ့ဆင်းသက်မှုအတွက်အသုံးပြုသည့်အချိန်ကာလကိုသတ်မှတ်သည် (`epoch = floor(height / term_blocks)`)၊ `parliament_min_stake` သည် အနိမ့်ဆုံးနှင့် အနိမ့်ဆုံးယူနစ်များအဖြစ် သတ်မှတ်ပြဋ္ဌာန်းသည် `parliament_eligibility_asset_id` သည် ကိုယ်စားလှယ်လောင်းသတ်မှတ်မှုကို တည်ဆောက်သည့်အခါ မည်သည့်ပိုင်ဆိုင်မှုလက်ကျန်ကို စကင်ဖတ်သည်ကို ရွေးသည်။

Governance VK အတည်ပြုခြင်းတွင် ရှောင်ကွင်းမရှိပါ- မဲပြားအတည်ပြုခြင်းတွင် Inline bytes ပါသော `Active` ဖြင့် အတည်ပြုသောကီးကို အမြဲတမ်းလိုအပ်ပြီး အတည်ပြုခြင်းအား ကျော်ရန် ပတ်ဝန်းကျင်များသည် စမ်းသပ်မှု-သီးသန့်ခလုတ်များပေါ်တွင် အားမကိုးရပါ။

RBAC
- On-chain execution သည် ခွင့်ပြုချက်များ လိုအပ်သည်-
  - အဆိုပြုချက်များ- `CanProposeContractDeployment{ contract_id }`
  - မဲများ- `CanSubmitGovernanceBallot{ referendum_id }`
  - အတည်ပြုချက်- `CanEnactGovernance`
  - ဖြတ်တောက်ခြင်း/အယူခံမှု- `CanSlashGovernanceLock{ referendum_id }`၊ `CanRestituteGovernanceLock{ referendum_id }`
  - ကောင်စီစီမံခန့်ခွဲမှု (အနာဂတ်): `CanManageParliament`
- ခုတ်ထစ်ခြင်း/အယူခံမှုများ-
  - နှစ်ချက်-မဲ/တရားမ၀င်/အရည်အချင်းမပြည့်မီသောမဲများသည် ငွေချေးစာချုပ် escrow နှင့် ကိုက်ညီသော ပြင်ဆင်သတ်မှတ်ထားသော မျဉ်းစောင်းရာခိုင်နှုန်းများကို သက်ရောက်သည်၊ ရန်ပုံငွေများကို `slash_receiver_account` သို့ပြောင်းခြင်း၊ မျဉ်းစောင်းဖြတ်ပိုင်းစာရင်းကို အပ်ဒိတ်လုပ်ခြင်း၊ နှင့် `LockSlashed` ရိုက်ထည့်ထားသော ဖြစ်ရပ်များ (အကြောင်းရင်း + ဦးတည်ရာ + မှတ်စု)။
  - လက်စွဲစာအုပ် `SlashGovernanceLock`/`RestituteGovernanceLock` ညွှန်ကြားချက်များသည် အော်ပရေတာမှ မောင်းနှင်သော ပြစ်ဒဏ်များနှင့် အယူခံဝင်မှုများကို ပံ့ပိုးပေးသည်။ ပြန်လည်အပ်နှံခြင်းကို မှတ်တမ်းတင်ထားသော မျဉ်းစောင်းများဖြင့် ဖုံးအုပ်ထားပြီး၊ ငွေချေးစာချုပ်တွင် ရန်ပုံငွေများ ပြန်လည်ရယူသည်၊ လယ်ဂျာကို အပ်ဒိတ်လုပ်ကာ `LockRestituted` ကို သက်တမ်းကုန်ဆုံးသည့်အချိန်အထိ သော့ခတ်မှုကို ဆက်လက်လုပ်ဆောင်နေချိန်တွင် ထုတ်လွှတ်ပါသည်။ကာကွယ်ထားသော အမည်နေရာများ
- စိတ်ကြိုက်ကန့်သတ်ဘောင် `gov_protected_namespaces` (JSON အခင်းအကျင်းများ) သည် စာရင်းသွင်းထားသော namespace များအတွင်း ဖြန့်ကျက်ရန်အတွက် ဝင်ခွင့်တံခါးကို ဖွင့်ပေးသည်။
- ဖောက်သည်များသည် ကာကွယ်ထားသော namespaces ကို ပစ်မှတ်ထား၍ ဖြန့်ကျက်ရန်အတွက် ငွေပေးငွေယူ မက်တာဒေတာသော့များ ပါဝင်ရမည်-
  - `gov_namespace`- ပစ်မှတ်အမည်နေရာ (ဥပမာ၊ `"apps"`)
  - `gov_contract_id`- namespace အတွင်းရှိ ယုတ္တိကျသော စာချုပ် ID
- `gov_manifest_approvers`- ih58... အကောင့် ID များ၏ ရွေးချယ်နိုင်သော JSON အခင်းအကျင်း။ လမ်းကြောတစ်ခုက တစ်ခုထက်ပိုကြီးသော quorum ကိုကြေညာသောအခါ၊ ဝင်ခွင့်သည် ထင်ရှားသောအထမြောက်မှုကို ကျေနပ်စေရန် ငွေပေးငွေယူလုပ်ပိုင်ခွင့်အာဏာနှင့် စာရင်းသွင်းထားသောအကောင့်များ လိုအပ်ပါသည်။
- Telemetry သည် `governance_manifest_admission_total{result}` မှတစ်ဆင့် လုံး၀ ဝင်ခွင့်ကောင်တာကို ဖော်ထုတ်ပေးသောကြောင့် အော်ပရေတာများသည် အောင်မြင်သော ဝန်ခံချက်များကို `missing_manifest`၊ `non_ih58..._authority`၊ `quorum_rejected`၊ Sumeragi နှင့် `protected_namespace_rejected` နှင့် လမ်းကြောင်းများကို ခွဲခြားနိုင်ပါသည်။
- Telemetry သည် `governance_manifest_quorum_total{outcome}` (တန်ဖိုးများ `satisfied` / `rejected`) မှတစ်ဆင့် ပြဋ္ဌာန်းထားသောလမ်းကြောင်းကို ဖုံးအုပ်ပေးသောကြောင့် အော်ပရေတာများသည် ပျောက်ဆုံးနေသော အတည်ပြုချက်များကို စစ်ဆေးနိုင်ပါသည်။
- Lanes များသည် ၎င်းတို့၏ manifests များတွင် ထုတ်ပြန်ထားသော namespace ခွင့်ပြုစာရင်းကို တွန်းအားပေးသည်။ `gov_namespace` ကို သတ်မှတ်ပေးသည့် မည်သည့် လွှဲပြောင်းမှုမဆို `gov_contract_id` ကို ပေးဆောင်ရမည် ဖြစ်ပြီး namespace သည် manifest ၏ `protected_namespaces` set တွင် ပေါ်လာရပါမည်။ အကာအကွယ်ကိုဖွင့်ထားသောအခါ `RegisterSmartContractCode` တွင် ဤမက်တာဒေတာမပါဘဲ တင်ပြမှုများကို ပယ်ချပါသည်။
- tuple `(namespace, contract_id, code_hash, abi_hash)` အတွက် အတည်ပြုထားသော အုပ်ချုပ်မှု အဆိုပြုချက်တစ်ခု ရှိနေကြောင်း ဝင်ခွင့် ပြဋ္ဌာန်းထားသည်။ မဟုတ်ပါက ခွင့်မပြုသော အမှားတစ်ခုဖြင့် အတည်ပြုခြင်း မအောင်မြင်ပါ။

Runtime Upgrade Hooks
- Lane manifests သည် runtime upgrade ညွှန်ကြားချက်များ (`ProposeRuntimeUpgrade`၊ `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) ကိုဂိတ်ပေါက်ရန် `hooks.runtime_upgrade` ကြေငြာနိုင်ပါသည်။
- ချိတ်ကွက်များ
  - `allow` (bool၊ မူရင်း `true`): `false` တွင်၊ runtime-upgrade ညွှန်ကြားချက်များအားလုံးကို ပယ်ချပါသည်။
  - `require_metadata` (bool၊ မူရင်း `false`): `metadata_key` မှ သတ်မှတ်ထားသော ငွေပေးငွေယူ မက်တာဒေတာ ထည့်သွင်းမှု လိုအပ်ပါသည်။
  - `metadata_key` (string)- ချိတ်ဖြင့် ပြဋ္ဌာန်းထားသော မက်တာဒေတာအမည်။ မက်တာဒေတာလိုအပ်သည့်အခါ သို့မဟုတ် ခွင့်ပြုစာရင်းတစ်ခုရှိနေသည့်အခါ မူရင်းပုံစံသည် `gov_upgrade_id` ဖြစ်သည်။
  - `allowed_ids` (စာကြောင်းများ၏ အခင်းအကျင်း)- မက်တာဒေတာတန်ဖိုးများ၏ ရွေးချယ်ခွင့်စာရင်း (ချုံ့ပြီးနောက်)။ ပေးထားသည့်တန်ဖိုးကို စာရင်းမသွင်းသည့်အခါ ငြင်းပယ်သည်။
- အချိတ်အဆက်ရှိနေသောအခါ၊ ငွေပေးငွေယူတန်းစီထဲသို့မဝင်မီ တန်းစီခြင်းဝင်ခွင့်သည် မက်တာဒေတာမူဝါဒကို ပြဋ္ဌာန်းသည်။ ခွင့်ပြုစာရင်းပြင်ပရှိ မက်တာဒေတာ၊ အလွတ်တန်ဖိုးများ သို့မဟုတ် တန်ဖိုးများသည် အဆုံးအဖြတ်ပေးသော `NotPermitted` အမှားအယွင်းကို ဖြစ်ပေါ်စေသည်။
- Telemetry သည် `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` မှတစ်ဆင့် ပြဋ္ဌာန်းထားသော ရလဒ်များကို ခြေရာခံသည်။
- ချိတ်ကို ကျေနပ်စေသော ငွေပေးငွေယူများတွင် မက်တာဒေတာ `gov_upgrade_id=<value>` (သို့မဟုတ် manifest-သတ်မှတ်သော့) နှင့်အတူ manifest quorum မှလိုအပ်သော မည်သည့် ih58... အတည်ပြုချက်များ ပါဝင်ရမည်။

အဆင်ပြေမှုအဆုံးမှတ်
- POST `/v1/gov/protected-namespaces` — `gov_protected_namespaces` ကို node ပေါ်တွင် တိုက်ရိုက်သက်ရောက်သည်။
  - တောင်းဆိုချက်- { "namespaces"- ["apps", "system"] }
  - တုံ့ပြန်ချက်- { "ok": true, "applied": 1 }
  - မှတ်စုများ- admin/testing အတွက် ရည်ရွယ်ပါသည်။ ပြင်ဆင်သတ်မှတ်ပါက API တိုကင်လိုအပ်သည်။ ထုတ်လုပ်မှုအတွက်၊ `SetParameter(Custom)` ဖြင့် လက်မှတ်ရေးထိုးထားသော အရောင်းအ၀ယ်ကို တင်သွင်းခြင်းကို ပိုနှစ်သက်သည်။CLI အကူအညီပေးသူများ
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - namespace နှင့် cross-checks အတွက် စာချုပ်ဥပမာများကို ထုတ်ယူသည်-
    - Torii သည် `code_hash` တစ်ခုစီအတွက် bytecode ကို သိမ်းဆည်းထားပြီး ၎င်း၏ Blake2b-32 အညွှန်းသည် `code_hash` နှင့် ကိုက်ညီပါသည်။
    - `/v1/contracts/code/{code_hash}` အောက်တွင် သိမ်းဆည်းထားသော မန်နီးဖက်စ်သည် `code_hash` နှင့် `abi_hash` တန်ဖိုးများနှင့် ကိုက်ညီသော အစီရင်ခံစာများ။
    - တူညီသော proposal-id ကို node အသုံးပြုမှုများကို ဟက်ချခြင်းဖြင့် ဆင်းသက်လာသကဲ့သို့ `(namespace, contract_id, code_hash, abi_hash)` အတွက် အတည်ပြုပြဌာန်းထားသော အုပ်ချုပ်မှု အဆိုပြုချက်တစ်ခု ရှိပါသည်။
  - စာချုပ်တစ်ခုလျှင် `results[]` ဖြင့် JSON အစီရင်ခံစာ (ပြဿနာများ၊ သရုပ်ဖော်ခြင်း/ကုဒ်/အဆိုပြုချက် အနှစ်ချုပ်များ) နှင့် (`--no-summary`) တို့ကို ဖိနှိပ်ထားခြင်းမရှိပါက တစ်ကြောင်းတစ်ကြောင်း အကျဉ်းချုပ်ကို ထုတ်ပေးပါသည်။
  - ကာကွယ်ထားသော namespace များကို စစ်ဆေးခြင်း သို့မဟုတ် အုပ်ချုပ်မှုထိန်းချုပ်ထားသော အလုပ်အသွားအလာများကို စိစစ်ခြင်းအတွက် အသုံးဝင်သည်။
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - manifest quorum စည်းမျဉ်းများကို ကျေနပ်စေရန် ရွေးချယ်နိုင်သော `gov_manifest_approvers` အပါအဝင် ကာကွယ်ထားသော namespaces များသို့ ဖြန့်ကျက်ထည့်သွင်းရာတွင် အသုံးပြုသည့် JSON မက်တာဒေတာအရိုးစုကို ထုတ်လွှတ်သည်။
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Canonical account ids များကို တရားဝင်စေပြီး 32-byte nullifier အရိပ်အမြွက်များကို canonicalize လုပ်ကာ အရိပ်အမြွက်များကို `public_inputs_json` (နောက်ထပ်ထပ်ဆောင်းမှုများအတွက် `--public <path>` နှင့်) ပေါင်းထည့်သည်။
  - nullifier ကို အထောက်အထား ကတိကဝတ် (အများပြည်သူ ထည့်သွင်းမှု) နှင့် `domain_tag`၊ `chain_id` နှင့် `election_id` တို့မှ ဆင်းသက်လာပါသည်။ `--nullifier` ကို ပေးသွင်းသည့်အခါ အထောက်အထားနှင့် ကိုက်ညီကြောင်း အတည်ပြုထားသည်။
  - တစ်ကြောင်းချင်းအကျဉ်းချုပ်သည် ယခုအခါတွင် ကုဒ်လုပ်ထားသော `CastZkBallot` မှ ဆင်းသက်လာသော အဆုံးအဖြတ်ပေးသည့် `fingerprint=<hex>` ကို ဖော်ပြနေပါသည်။
  - CLI တုံ့ပြန်မှုများသည် `tx_instructions[]` နှင့် `payload_fingerprint_hex` အပေါင်းကို ကုဒ်လုပ်ထားသော အကွက်များဖြင့် မှတ်သားထားသောကြောင့် ရေအောက်တူးလ်လုပ်ခြင်းဖြင့် Norito ကို ထပ်မွမ်းမံခြင်းမပြုဘဲ အရိုးစုကို အတည်ပြုနိုင်သည်။
  - သော့ခတ်မှု အရိပ်အမြွက်ပေးသည့်အခါ၊ ZK မဲများသည် `owner`၊ `amount` နှင့် `duration_blocks` တို့ကို ပံ့ပိုးပေးရမည်။ တစ်စိတ်တစ်ပိုင်း အရိပ်အမြွက်များကို ပယ်ချပါသည်။ `min_bond_amount > 0` တွင် လော့ခ်ချရန် အရိပ်အမြွက်များ လိုအပ်ပါသည်။ လမ်းညွှန်ချက်သည် ရွေးချယ်ခွင့်ရှိနေဆဲဖြစ်ပြီး အရိပ်အမြွက်အဖြစ်သာ သဘောထားသည်။
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` သည် canonical IH58 စာလုံးများကို လက်ခံသည် ။ ရွေးချယ်နိုင်သော `@<domain>` ၏ နောက်ဆက်တွဲများသည် လမ်းပြခြင်းအရိပ်အမြွက်များသာဖြစ်သည်။
  - Aliases `--lock-amount`/`--lock-duration-blocks` သည် scripting parity အတွက် ZK အလံအမည်များကို ထင်ဟပ်စေသည်။
  - ကုဒ်လုပ်ထားသော ညွှန်ကြားချက်လက်ဗွေနှင့် လူသားဖတ်နိုင်သောမဲကွက်များ (`owner`၊ `amount`၊ `duration_blocks`၊ `direction`) အပါအဝင် ကုဒ်လုပ်ထားသော လက်ဗွေနှင့် လူသားဖတ်နိုင်သော မဲပုံးများအပါအဝင် အကျဉ်းချုပ် အထွက်မှန်များကို `duration_blocks`၊သာဓကများစာရင်း
- `/v1/gov/instances/{ns}` ကိုရယူပါ — namespace အတွက် တက်ကြွသော စာချုပ်ဖြစ်ရပ်များကို စာရင်းပြုစုပါ။
  - Query params များ-
    - `contains`- `contract_id` (စာလုံးအသေး-အကဲဆတ်) ၏ အခွဲဖြင့် စစ်ထုတ်ပါ
    - `hash_prefix`- `code_hash_hex` ၏ hex ရှေ့ဆက်ဖြင့် စစ်ထုတ်ခြင်း
    - `offset` (မူလ 0)၊ `limit` (မူလ 100၊ အများဆုံး 10_000)
    - `order`- `cid_asc` (မူရင်း)၊ `cid_desc`၊ `hash_asc`၊ `hash_desc`
  - တုံ့ပြန်မှု- { "namespace": "ns", "instances": [{ "contract_id": "…", "code_hash_hex": "…" }, …], "total": N, "offset": n, "limit": m }
  - SDK အကူအညီပေးသူ- `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) သို့မဟုတ် `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)။

Sweep ကို လော့ခ်ဖွင့်ပါ (အော်ပရေတာ/စာရင်းစစ်)
- `/v1/gov/unlocks/stats` ရယူပါ။
  - တုံ့ပြန်မှု- { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - မှတ်စုများ- `last_sweep_height` သည် သက်တမ်းလွန်သော့ခလောက်များကို ဖယ်ရှားပြီး ဆက်လက်တည်မြဲနေသည့် လတ်တလော ဘလောက်အမြင့်ကို ထင်ဟပ်စေသည်။ `expired_locks_now` ကို `expiry_height <= height_current` ဖြင့် လော့ခ်ချခြင်းမှတ်တမ်းများကို စကင်န်ဖတ်ခြင်းဖြင့်တွက်ချက်ပါသည်။
- POST `/v1/gov/ballots/zk-v1`
  - တောင်းဆိုချက် (v1-စတိုင် DTO):
    {
      "authority": "ih58..."၊
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?"၊
      "election_id": "ကိုးကား-၁",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ="၊
      "root_hint": "0x…64hex?"၊
      "ပိုင်ရှင်"- "ih58…", // canonical AccountId (IH58 ပကတိ; ရွေးချယ်နိုင်သော @domain အရိပ်အမြွက်)
      "ပမာဏ": "100?"၊
      "duration_blocks": 6000?၊
      "ဦးတည်ချက်"- "အေး|နေ|နေသလား?"၊
      "nullifier": "blake2b32:…64hex?"
    }
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }- ပို့စ် `/v1/gov/ballots/zk-v1/ballot-proof` (အင်္ဂါရပ်- `zk-ballot`)
  - `BallotProof` JSON ကို တိုက်ရိုက်လက်ခံပြီး `CastZkBallot` အရိုးစုကို ပြန်ပေးသည်။
  - တောင်းဆိုချက်-
    {
      "authority": "ih58..."၊
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?"၊
      "election_id": "ကိုးကား-1",
      "မဲ"- {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ="၊ ZK1 သို့မဟုတ် H2* ကွန်တိန်နာ၏ // base64
        "root_hint": null၊ // ရွေးချယ်နိုင်သော 32-byte hex string (အရည်အချင်းပြည့်မီမှု အမြစ်)
        "ပိုင်ရှင်"- null၊ // ရွေးချယ်နိုင်သော canonical AccountId (IH58 ပကတိ၊ ရွေးချယ်နိုင်သော @domain အရိပ်အမြွက်)
        "nullifier"- null၊ // ရွေးချယ်နိုင်သော 32-byte hex string (nullifier အရိပ်အမြွက်)
        "ပမာဏ"- "100", // ရွေးချယ်နိုင်သော လော့ခ်ပမာဏ အရိပ်အမြွက် (ဒဿမ စာတန်း)
        "duration_blocks": 6000၊ // ရွေးချယ်နိုင်သောသော့ခတ်ကြာချိန် အရိပ်အမြွက်
        "direction": "အေး" // ရွေးချယ်နိုင်သော ဦးတည်ချက် အရိပ်အမြွက်
      }
    }
  - တုံ့ပြန်မှု-
    {
      "ok": မှန်ပါတယ်၊
      "လက်ခံသည်": မှန်သည်၊
      "အကြောင်းပြချက်": "ငွေပေးငွေယူအရိုးစုတည်ဆောက်ခြင်း",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "…" }
      ]
    }
  - မှတ်စုများ
    - `private_key` ကို ပံ့ပိုးပေးသောအခါ၊ Torii သည် လက်မှတ်ထိုးထားသော ငွေလွှဲပို့မှုကို တင်သွင်းပြီး `reason` ကို `submitted transaction` သို့ သတ်မှတ်သည်။
    - ဆာဗာသည် ရွေးချယ်နိုင်သော `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/Prometheus အတွက် မဲပုံးမှ `nullifier` မှ I1800000180X `CastZkBallot`။
    - စာအိတ်ဘိုက်များကို ညွှန်ကြားချက်ပေးဆောင်မှုအတွက် base64 အဖြစ် ပြန်လည်ကုဒ်လုပ်ထားပါသည်။
    - `zk-ballot` အင်္ဂါရပ်ကို ဖွင့်ထားသောအခါမှသာ ဤအဆုံးမှတ်ကို ရရှိနိုင်သည်။

CastZkBallot အတည်ပြုခြင်းလမ်းကြောင်း
- `CastZkBallot` သည် ပံ့ပိုးပေးထားသော base64 အထောက်အထားကို ကုဒ်လုပ်ပြီး ဗလာ သို့မဟုတ် ပုံပျက်နေသော payloads (`BallotRejected` with `invalid or empty proof`) ကို ငြင်းပယ်သည်။
- အကယ်၍ `public_inputs_json` ကို ထောက်ပံ့ပေးပါက၊ ၎င်းသည် JSON အရာဝတ္ထုတစ်ခု ဖြစ်ရပါမည်။ အရာဝတ္ထုမဟုတ်သော payload များကို ငြင်းပယ်သည်။
- အိမ်ရှင်သည် ဆန္ဒခံယူပွဲ (`vk_ballot`) သို့မဟုတ် အုပ်ချုပ်မှုပုံသေများမှ မဲစိစစ်သည့်သော့ကို ဖြေရှင်းပေးပြီး မှတ်တမ်းတည်ရှိရန် လိုအပ်သည်၊ `Active` ဖြစ်ရန်နှင့် inline bytes သယ်ဆောင်ပါ။
- သိမ်းဆည်းထားသော အတည်ပြုသော့ဘိုက်များကို `hash_vk` ဖြင့် ပြန်လည် ဟက်ခ်လုပ်ထားပါသည်။ ကတိကဝတ်များ မကိုက်ညီပါက ဖျက်လိုက်သော registry entries များကို ကာကွယ်ရန် စိစစ်ခြင်းမပြုမီ ကတိကဝတ်များ ပျက်သွားသည် (`BallotRejected` with `verifying key commitment mismatch`)။
- အထောက်အထားဘိုက်များကို `zk::verify_backend` မှတစ်ဆင့် စာရင်းသွင်းထားသော နောက်ခံသို့ ပေးပို့သည်။ `invalid proof` ဖြင့် `BallotRejected` အဖြစ် မမှန်ကန်သော စာသားများ ပေါ်လာပြီး ညွှန်ကြားချက်သည် တိကျစွာ ပျက်ကွက်ပါသည်။
- အထောက်အထားသည် အများသူငှာ သွင်းအားစုများအဖြစ် မဲကတိကဝတ်နှင့် အရည်အချင်းပြည့်မီမှု အရင်းမြစ်ကို ဖော်ထုတ်ရမည်။ အမြစ်သည် ရွေးကောက်ပွဲ၏ `eligible_root` နှင့် ကိုက်ညီရမည်ဖြစ်ပြီး ဆင်းသက်လာသော nullifier သည် ပေးထားသည့် မည်သည့်အရိပ်အမြွက်နှင့်မဆို ကိုက်ညီရပါမည်။
- အောင်မြင်သောအထောက်အထားများသည် `BallotAccepted` ကိုထုတ်လွှတ်သည်။ ပွားနေသော nullifiers၊ ဟောင်းနွမ်းနေသော အရည်အသွေးပြည့်မီမှု အမြစ်များ သို့မဟုတ် လော့ခ်ချခြင်း ဆုတ်ယုတ်မှုများသည် ဤစာတမ်းတွင် ဖော်ပြထားသော ရှိပြီးသား ငြင်းပယ်ခြင်းဆိုင်ရာ အကြောင်းပြချက်များကို ဆက်လက်ဖြစ်ပေါ်စေပါသည်။

## မမှန်မကန်ပြုမူမှုနှင့် ပူးတွဲသဘောဆန္ဒကို အတည်ပြုသည်။

### ခုတ်ထစ်ခြင်းနှင့် ထောင်ချခြင်းလုပ်ငန်းih58... ပရိုတိုကောကို ချိုးဖောက်သည့်အခါတိုင်း Norito-encoded `Evidence` payload တစ်ခုစီသည် in-memory `EvidenceStore` တွင်ရှိပြီး၊ မမြင်ရပါက WSV ကျောထောက်နောက်ခံပြုထားသော `consensus_evidence` မြေပုံတွင် ရုပ်လုံးပေါ်လာပါသည်။ `sumeragi.npos.reconfig.evidence_horizon_blocks` (မူလ `7200` ဘလောက်များ) ထက်ဟောင်းသော မှတ်တမ်းများကို ပယ်ချသည် ထို့ကြောင့် မော်ကွန်းကို ကန့်သတ်ထားသော်လည်း ငြင်းပယ်ခြင်းကို အော်ပရေတာများအတွက် မှတ်တမ်းတင်ထားသည်။ မိုးကုပ်စက်ဝိုင်းအတွင်းမှ အထောက်အထားများသည် ပူးတွဲသဘောတူညီချက် (`mode_activation_height requires next_mode to be set in the same block`)၊ စတင်ခြင်းနှောင့်နှေးခြင်း (`sumeragi.npos.reconfig.activation_lag_blocks`၊ မူရင်း `1`) နှင့် မျဉ်းစောင်းနှောင့်နှေးမှု (`sumeragi.npos.reconfig.slashing_delay_blocks`၊ 7000 default `sumeragi.npos.reconfig.slashing_delay_blocks`)၊ မလျှောက်ထားမီ ပြစ်ဒဏ်များကို ပယ်ဖျက်ပါ။

အသိအမှတ်ပြုထားသော ပြစ်မှုများသည် `EvidenceKind` သို့ တစ်ပုံမှတစ်ပုံ ပုံဖော်ထားသည်။ ခွဲခြားဆက်ဆံသူများသည် ဒေတာပုံစံဖြင့် တည်ငြိမ်ပြီး ကျင့်သုံးသည်-

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

- **DoublePrepare/DoubleCommit** — the ih58... တူညီသော `(phase,height,view,epoch)` tuple အတွက် ကွဲလွဲနေသော hashe များကို ရေးထိုးထားသည်။
- **InvalidQc** — စုစည်းမှုတစ်ခုသည် အဆုံးအဖြတ်စစ်ဆေးမှုများ မအောင်မြင်သော ပုံသဏ္ဍာန်၏ QC တစ်ခုအား အတင်းအဖျင်းပြောခဲ့သည် (ဥပမာ၊ ဗလာနက္ခတ်ဘစ်မြေပုံ)။
- **InvalidProposal** — ခေါင်းဆောင်တစ်ဦးသည် ဖွဲ့စည်းတည်ဆောက်ပုံဆိုင်ရာ စစ်ဆေးအတည်ပြုခြင်းမှ ပျက်ကွက်သောပိတ်ဆို့တစ်ခု (ဥပမာ၊ သော့ခတ်ထားသောကွင်းဆက်စည်းမျဉ်းကို ချိုးဖောက်သည်)။
- **ဆင်ဆာဖြတ်တောက်ခြင်း** — လက်မှတ်ရေးထိုးထားသော တင်သွင်းမှုပြေစာများသည် အဆိုပြုခြင်း/ကတိက၀တ် မပြုဖူးသော အရောင်းအ၀ယ်တစ်ခုကို ပြသသည်။

VRF ပြစ်ဒဏ်များသည် `activation_lag_blocks` (ပြစ်မှုကျူးလွန်သူများအား ထောင်သွင်းအကျဉ်းချခြင်း) ပြီးနောက် အလိုအလျောက် ပြဋ္ဌာန်းပါသည်။ အုပ်ချုပ်ရေးမှ ပြစ်ဒဏ်ကို မပယ်ဖျက်ပါက `slashing_delay_blocks` ဝင်းဒိုးပြီးနောက်မှသာ အများသဘောတူချက်ကို ဖြတ်တောက်ခြင်းကို အသုံးပြုပါသည်။

အော်ပရေတာများနှင့် ကိရိယာတန်ဆာပလာများသည် ဝန်ဆောင်ခများကို စစ်ဆေးပြီး ပြန်လည်ထုတ်လွှင့်နိုင်သည်-

- Torii: `GET /v1/sumeragi/evidence` နှင့် `GET /v1/sumeragi/evidence/count`။
- CLI- `iroha ops sumeragi evidence list`၊ `… count` နှင့် `… submit --evidence-hex <payload>`။

အုပ်ချုပ်ရေးသည် သက်သေ ဘိုက်များကို ကျမ်းဂန်အထောက်အထားအဖြစ် ဆက်ဆံရမည်-

1. **သက်တမ်းမကုန်မီ payload** ကိုစုဆောင်းပါ။ အကြမ်းထည် Norito bytes ကို အမြင့်/ကြည့်ရှုမှု မက်တာဒေတာနှင့်အတူ သိမ်းဆည်းပါ။
2. လိုအပ်ပါက `CancelConsensusEvidencePenalty` ကို `slashing_delay_blocks` မတိုင်မီ အထောက်အထားပေးဆောင်မှုနှင့်အတူ တင်သွင်းခြင်းဖြင့် **Cancel**၊ မှတ်တမ်းကို `penalty_cancelled` နှင့် `penalty_cancelled_at_height` ဟု အမှတ်အသားပြုထားပြီး မျဉ်းစောင်းများ သက်ရောက်မှုမရှိပါ။
3. လူထုဆန္ဒခံယူပွဲ သို့မဟုတ် sudo ညွှန်ကြားချက် (ဥပမာ၊ `Unregister::peer`) တွင် payload ကို ထည့်သွင်းခြင်းဖြင့် **ပြစ်ဒဏ်ကို အဆင့်သတ်မှတ်ပါ* လုပ်ဆောင်ချက်သည် ဝန်ဆောင်ခကို ပြန်လည်အတည်ပြုသည်။ ပုံသဏ္ဍာန်မမှန်သော၊
4. **နောက်ဆက်တွဲ topology ကို အချိန်ဇယားဆွဲပါ** ထို့ကြောင့် စော်ကားသော ih58... သည် ချက်ချင်းပြန်မလာနိုင်ပါ။ အပ်ဒိတ်စာရင်းဇယားနှင့်အတူ ပုံမှန်စီးဆင်းနေသော `SetParameter(Sumeragi::NextMode)` နှင့် `SetParameter(Sumeragi::ModeActivationHeight)`။
5. **စာရင်းစစ်ရလဒ်** သည် `/v1/sumeragi/evidence` နှင့် `/v1/sumeragi/status` မှတစ်ဆင့် သက်သေအထောက်အထားများ တန်ပြန်အဆင့်မြင့်ပြီး အုပ်ချုပ်ရေးမှ ဖယ်ရှားခြင်းကို အတည်ပြုကြောင်း သေချာစေရန်။

### Joint-Consensus Sequencing

အထွက် ih58... set သည် သတ်မှတ်အသစ်မစတင်မီ နယ်နိမိတ်ပိတ်ဆို့ခြင်းကို အပြီးသတ်ကြောင်း အာမခံပါသည်။ runtime သည် တွဲထားသော ဘောင်များမှတစ်ဆင့် စည်းမျဉ်းကို ပြဋ္ဌာန်းသည်-- `SumeragiParameter::NextMode` နှင့် `SumeragiParameter::ModeActivationHeight` သည် **တူညီသောဘလောက်** တွင် ကျူးလွန်ရပါမည်။ `mode_activation_height` သည် အပ်ဒိတ်လုပ်သော ဘလောက်အမြင့်ထက် တင်းကြပ်စွာ ကြီးနေရမည် ဖြစ်ပြီး အနည်းဆုံး တစ်တုံး နောက်ကျခြင်းကို ပေးဆောင်သည်။
- `sumeragi.npos.reconfig.activation_lag_blocks` (မူလ `1`) သည် လုံးဝလက်မလျှော့ခြင်းများကို တားဆီးပေးသည့် ဖွဲ့စည်းမှုအစောင့်အကြပ်ဖြစ်သည်-
- `sumeragi.npos.reconfig.slashing_delay_blocks` (မူလ `259200`) သည် အများဆန္ဒကို ဖြတ်တောက်ခြင်းကို နှောင့်နှေးစေသောကြောင့် အုပ်ချုပ်ရေးက ၎င်းတို့မကျင့်သုံးမီ ပြစ်ဒဏ်များကို ပယ်ဖျက်နိုင်သည်။

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- runtime နှင့် CLI သည် `/v1/sumeragi/params` နှင့် `iroha sumeragi params --summary` မှတဆင့် အဆင့်သတ်မှတ်ထားသော ဘောင်များကို ဖော်ထုတ်ပေးသောကြောင့် အော်ပရေတာများသည် activation အမြင့်များနှင့် ih58... စာရင်းများကို အတည်ပြုနိုင်ပါသည်။
- အုပ်ချုပ်မှု အလိုအလျောက်စနစ်သည် အမြဲရှိသင့်သည်-
  1. အထောက်အထား-ကျောထောက်နောက်ခံပြုထားသော ဖယ်ရှားခြင်း (သို့မဟုတ် ပြန်လည်ထည့်သွင်းခြင်း) ဆုံးဖြတ်ချက်ကို အပြီးသတ်ပါ။
  2. `mode_activation_height = h_current + activation_lag_blocks` ဖြင့် နောက်ဆက်တွဲ ပြင်ဆင်မှုကို တန်းစီပါ။
  3. `/v1/sumeragi/status` ကို `effective_consensus_mode` အထိ မျှော်လင့်ထားသည့် အမြင့်သို့ ပြန်လှန်ပါ။

ih58...s ကို လှည့်ပတ်သော သို့မဟုတ် ဖြတ်တောက်ခြင်းကို အသုံးချသည့် မည်သည့် script မဆို ** မလုပ်ဆောင်ရပါမည်** ကို လုံးဝလက်မလျော့ဘဲ အသက်သွင်းရန် ကြိုးပမ်းခြင်း သို့မဟုတ် hand-off ကန့်သတ်ဘောင်များကို ချန်လှပ်ထားခြင်း၊ ထိုသို့သော လွှဲပြောင်းမှုများကို ပယ်ချပြီး ကွန်ရက်ကို ယခင်မုဒ်တွင် ထားလိုက်ပါ။

## Telemetry မျက်နှာပြင်များ

- Prometheus မက်ထရစ်များ ပို့ကုန်အုပ်ချုပ်ရေး လုပ်ဆောင်ချက်-
  - `governance_proposals_status{status}` (gauge) သည် အခြေအနေအလိုက် အဆိုပြုချက်ရေတွက်မှုကို ခြေရာခံသည်။
  - ကာကွယ်ထားသော namespace ဝင်ခွင့်လက်ခံမှုကို ခွင့်ပြုခြင်း သို့မဟုတ် ငြင်းပယ်သည့်အခါ `governance_protected_namespace_total{outcome}` (ကောင်တာ) တိုးခြင်းများ။
  - `governance_manifest_activations_total{event}` (ကောင်တာ) သည် ထင်ရှားသောထည့်သွင်းမှုများ (`event="manifest_inserted"`) နှင့် namespace binding (`event="instance_bound"`) တို့ကို မှတ်တမ်းတင်သည်။
- `/status` တွင် `governance` သည် အဆိုပြုချက်အရေအတွက်များကို ထင်ဟပ်ပြသပေးသည့် အရာဝတ္တုတစ်ခု၊ ကာကွယ်ထားသော namespace စုစုပေါင်းများကို အစီရင်ခံခြင်းနှင့် မကြာသေးမီက ထင်ရှားပေါ်လွင်သော အသက်သွင်းမှုများ (namespace၊ စာချုပ် id၊ ကုဒ်/ABI ဟက်ရှ်၊ ပိတ်ဆို့အမြင့်၊ အသက်သွင်းချိန်တံဆိပ်) ပါဝင်သည်။ ပြဋ္ဌာန်းချက်များ မွမ်းမံထားသော မန်နီးဖက်စ်များနှင့် ကာကွယ်ထားသော namespace ဂိတ်များကို ပြဋ္ဌာန်းထားကြောင်း အတည်ပြုရန် အော်ပရေတာများသည် ဤအကွက်ကို စစ်တမ်းကောက်ယူနိုင်သည်။
- Grafana နမူနာပုံစံ (`docs/source/grafana_governance_constraints.json`) နှင့်
  `telemetry.md` ရှိ telemetry runbook တွင် ချိတ်မိစေရန် အချက်ပေးသံများကို ဝိုင်ယာကြိုးတပ်နည်းကို ပြသည်
  အဆိုပြုချက်များ၊ ထင်ရှားသော အသက်သွင်းမှုများ ပျောက်ဆုံးနေခြင်း၊ သို့မဟုတ် မျှော်လင့်မထားသော ကာကွယ်ထားသော အမည်နေရာလွတ်များ
  runtime အဆင့်မြှင့်တင်မှုများအတွင်း ငြင်းပယ်မှုများ။