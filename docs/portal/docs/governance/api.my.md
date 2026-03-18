---
lang: my
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

အခြေအနေ- အုပ်ချုပ်မှု အကောင်အထည်ဖော်ရေး လုပ်ငန်းများကို ပူးတွဲဆောင်ရွက်ရန် မူကြမ်း/ပုံကြမ်း။ အကောင်အထည်ဖော်နေစဉ်အတွင်း ပုံစံများ ပြောင်းလဲနိုင်သည်။ Determinism နှင့် RBAC မူဝါဒများသည် စံကန့်သတ်ချက်များဖြစ်သည်။ Torii သည် `authority` နှင့် `private_key` ကို ပံ့ပိုးပေးသောအခါတွင် ငွေပေးငွေယူများကို လက်မှတ်ထိုး/တင်သွင်းနိုင်သည်၊ သို့မဟုတ်ပါက clients များသည် `/transaction` သို့ တည်ဆောက်ပြီး တင်ပြနိုင်သည်။

ခြုံငုံသုံးသပ်ချက်
- အဆုံးမှတ်များအားလုံးသည် JSON ကို ပြန်ပေးသည်။ အရောင်းအ၀ယ်-ထုတ်လုပ်သည့် စီးဆင်းမှုများအတွက်၊ တုံ့ပြန်မှုများတွင် `tx_instructions` — တစ်ခု သို့မဟုတ် တစ်ခုထက်ပိုသော ညွှန်ကြားချက်အရိုးစုများ၏ အခင်းဖြစ်-
  - `wire_id`- ညွှန်ကြားချက်အမျိုးအစားအတွက် မှတ်ပုံတင်စနစ်
  - `payload_hex`: Norito payload bytes (hex)
- အကယ်၍ `authority` နှင့် `private_key` (သို့မဟုတ် မဲ DTO များပေါ်တွင် `private_key`) ကို Torii ဆိုင်းဘုတ်များဖြင့် ပေးပို့ပြီး ငွေပေးငွေယူတင်ပြပြီး `tx_instructions` ကို ဆက်လက်ပေးပို့ပါ။
- မဟုတ်ပါက၊ clients များသည် ၎င်းတို့၏ အခွင့်အာဏာနှင့် chain_id ကို အသုံးပြု၍ SignedTransaction ကိုစုပေါင်းပြီး `/transaction` သို့ လက်မှတ်ထိုးပြီး POST လုပ်ပါ။
- SDK လွှမ်းခြုံမှု-
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` သည် `GovernanceProposalResult` (ပုံမှန်အခြေအနေ/အမျိုးအစားအကွက်များ), `ToriiClient.get_governance_referendum_typed` ပြန်ပေးသည် `GovernanceReferendumResult`၊ I18NI0000000000 I18NI000000420 I18NI000000 `ToriiClient.get_governance_locks_typed` သည် `GovernanceLocksResult` ကိုပြန်ပေးသည်၊ `ToriiClient.get_governance_unlock_stats_typed` သည် `GovernanceUnlockStats` ကိုပြန်ပေးသည်၊ နှင့် `ToriiClient.list_governance_instances_typed` သည် `GovernanceInstancesPage` ကိုပြန်ပေးသည်၊ RE သည် REUTERS တွင်ရိုက်ထည့်ထားသောအသုံးပြုခွင့်နမူနာများကိုအသုံးပြုမှုအပေါ်ယံအစိုးရကိုပြဌာန်းထားသောအသုံးပြုမှုများ
- Python ပေါ့ပါးသောကလိုင်းယင့် (`iroha_torii_client`): `ToriiClient.finalize_referendum` နှင့် `ToriiClient.enact_proposal` ပြန်ရိုက်ထားသော `GovernanceInstructionDraft` အစုအဝေးများ (Torii အရိုးစု I1000000X ကို ရှောင်ရှားသည့်အခါ၊ JSON ကို ရှောင်ရှားရန်)၊ scripts များသည် Finalize/ Enact flows ကိုရေးဖွဲ့သည်။
- JavaScript (`@iroha/iroha-js`)- `ToriiClient` သည် အဆိုပြုချက်များ၊ ရည်ညွှန်းချက်၊ မှတ်တမ်းများ၊ သော့များ၊ လော့ခ်ချမှုများ၊ လော့ခ်ချသည့် ကိန်းဂဏန်းများနှင့် ယခု `listGovernanceInstances(namespace, options)` နှင့် ကောင်စီ အဆုံးမှတ်များ (I18NI000000590X၊ I18NI0000005900001 `governancePersistCouncil`၊ `getGovernanceCouncilAudit`) ထို့ကြောင့် Node.js ဖောက်သည်များသည် `/v1/gov/instances/{ns}` ကို paginate နိုင်ပြီး VRF ကျောထောက်နောက်ခံပြုထားသော အလုပ်အသွားအလာများကို လက်ရှိစာချုပ်-ဥပမာစာရင်းနှင့်အတူ မောင်းနှင်နိုင်သည်။

အဆုံးမှတ်များ

- POST `/v1/gov/proposals/deploy-contract`
  - တောင်းဆိုချက် (JSON):
    {
      "namespace": "အက်ပ်များ",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:…" | "…64 hex"၊
      "abi_hash": "blake2b32:…" | "…64 hex"၊
      "abi_version": "1",
      "window": { "lower": 12345၊ "upper": 12400 }၊
      "အာဏာပိုင်": "i105…?"၊
      "private_key": "…?"
    }
  - တုံ့ပြန်မှု (JSON):
    { "ok": true၊ "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
  - အတည်ပြုခြင်း- ပေးထားသော `abi_version` အတွက် nodes များကို canonicalise `abi_hash` နှင့် မကိုက်ညီမှုများကို ငြင်းပယ်ပါ။ `abi_version = "v1"` အတွက် မျှော်လင့်ထားသော တန်ဖိုးမှာ `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ဖြစ်သည်။

စာချုပ်များ API (အသုံးချရန်)
- POST `/v1/contracts/deploy`
  - တောင်းဆိုချက်- { "authority": "i105...", "private_key": "…", "code_b64": "…" }
  - အပြုအမူ- IVM ပရိုဂရမ်ကိုယ်ထည်မှ `code_hash` ကို တွက်ချက်ပြီး `abi_version` ခေါင်းစီးမှ `abi_hash`၊ ထို့နောက် `RegisterSmartContractCode` (manifest) နှင့် I10070NI `.to` bytes) `authority` ကိုယ်စား။
  - တုံ့ပြန်ချက်- { "ok": true, "code_hash_hex": "…", "abi_hash_hex": "…" }
  ဆက်စပ်-
    - `/v1/contracts/code/{code_hash}` → သိမ်းဆည်းထားသော မန်နီးဖက်စ်ကို ပြန်ရယူပါ။
    - `/v1/contracts/code-bytes/{code_hash}` → `{ code_b64 }` ကို ပြန်ရယူပါ
- POST `/v1/contracts/instance`
  - တောင်းဆိုချက်- { "authority": "i105...", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "…" }
  - အပြုအမူ- ပံ့ပိုးပေးထားသော bytecode ကို အသုံးချပြီး `ActivateContractInstance` မှတစ်ဆင့် `(namespace, contract_id)` မြေပုံကို ချက်ချင်း အသက်သွင်းသည်။
  - တုံ့ပြန်မှု- { "ok": true၊ "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "…", "abi_hash_hex": "…" }

Alias ဝန်ဆောင်မှု
- POST `/v1/aliases/voprf/evaluate`
  - တောင်းဆိုချက်- { "blinded_element_hex": "…" }
  - တုံ့ပြန်ချက်- { "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }
    - `backend` သည် အကဲဖြတ်သူ၏ အကောင်အထည်ဖော်မှုကို ထင်ဟပ်စေသည်။ ကာလတန်ဖိုး- `blake2b512-mock`။
  - မှတ်စုများ- ဒိုမိန်းခွဲခြားခြင်း `iroha.alias.voprf.mock.v1` ဖြင့် Blake2b512 ကို အသုံးပြုသည့် ခွဲခြားသတ်မှတ်ပုံတူသော အကဲဖြတ်သူ။ ထုတ်လုပ်မှု VOPRF ပိုက်လိုင်းကို Iroha မှတဆင့် ကြိုးမသွယ်မချင်း စမ်းသပ်ကိရိယာအတွက် ရည်ရွယ်သည်။
  - အမှားများ- ပုံစံမမှန်သော hex ထည့်သွင်းမှုတွင် HTTP `400`။ Torii သည် Norito `ValidationFail::QueryFailed::Conversion` စာအိတ်ကို ဒီကုဒ်ဒါအမှား မက်ဆေ့ဂျ်ဖြင့် ပြန်ပေးသည်။
- POST `/v1/aliases/resolve`
  - တောင်းဆိုချက်- { "alias": "GB82 WEST 1234 5698 7654 32" }
  - တုံ့ပြန်မှု- { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - မှတ်ချက်များ- ISO တံတားဖွင့်ချိန် အဆင့်သတ်မှတ်မှု (`[iso_bridge.account_aliases]` တွင် `iroha_config`) လိုအပ်သည်။ Torii သည် ရှာဖွေမှုမပြုမီ အဖြူကွက်နှင့် အပေါ်ဖုံးကို ဖယ်ထုတ်ခြင်းဖြင့် နာမည်များကို ပုံမှန်ဖြစ်စေသည်။ alias မရှိတော့သည့်အခါ 404 နှင့် ISO တံတားဖွင့်ချိန်ကို ပိတ်ထားသောအခါ 503 ကို ပြန်ပေးသည်။
- POST `/v1/aliases/resolve_index`
  - တောင်းဆိုချက်- { "index": 0 }
  - တုံ့ပြန်မှု- { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - မှတ်ချက်- Alias indices များကို configuration order (0-based) မှ ဆုံးဖြတ်သတ်မှတ်ပေးပါသည်။ ဖောက်သည်များသည် အတုအယောင် သက်သေခံချက်ဖြစ်ရပ်များအတွက် စာရင်းစစ်လမ်းကြောင်းများတည်ဆောက်ရန် အော့ဖ်လိုင်းတွင် တုံ့ပြန်မှုများကို ကက်ရှ်လုပ်နိုင်ပါသည်။

Code Size Cap
- စိတ်ကြိုက်ဘောင်- `max_contract_code_bytes` (JSON u64)
  - ကွင်းဆက်စာချုပ်ကုဒ်သိုလှောင်မှုအတွက် အများဆုံးခွင့်ပြုထားသောအရွယ်အစား (ဘိုက်များ) ကို ထိန်းချုပ်သည်။
  - ပုံသေ- 16 MiB။ `.to` ပုံအလျားသည် ပုံသဏ္ဍာန်အဖုံးထက်ကျော်လွန်သောအခါ Nodes များသည် `RegisterSmartContractBytes` ကို ငြင်းပယ်သည်။
  - အော်ပရေတာများသည် `SetParameter(Custom)` ကို `id = "max_contract_code_bytes"` နှင့် ဂဏန်းပေးချေမှုတစ်ခုတင်ပြခြင်းဖြင့် ချိန်ညှိနိုင်သည်။

- POST `/v1/gov/ballots/zk`
  - တောင်းဆိုချက်- { "authority": "i105...", "private_key": "…?", "chain_id": "…", "election_id": "e1", "proof_b64": "…", " public": {…} }
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }
  - မှတ်စုများ
    - ဆားကစ်၏ အများသူငှာ သွင်းအားစုများတွင် `owner`၊ `amount` နှင့် `duration_blocks` ပါ၀င်ပြီး အထောက်အထားသည် configured VK ကို ဆန့်ကျင်သောအခါ၊ node သည် I18NI0000010100102NIX ဖြင့် I18NI000001000102NIX ဦးတည်ချက်ကို ဝှက်ထားဆဲ (`unknown`); ပမာဏ/သက်တမ်း ကုန်ဆုံးမှုကိုသာ အပ်ဒိတ်လုပ်ထားသည်။ ပြန်လည်မဲများသည် မိုနိုတိုနစ်ဖြစ်သည်- ပမာဏနှင့် သက်တမ်းကုန်ဆုံးမှုသည်သာ တိုးလာသည် (node ​​သည် အမြင့်ဆုံး(amount, prev.amount) နှင့် max(expiry, prev.expiry)))။
    - ပမာဏ သို့မဟုတ် သက်တမ်းကုန်ဆုံးရန် ကြိုးပမ်းသည့် ZK ပြန်လည်မဲများကို `BallotRejected` အဖြေရှာမှုဖြင့် ဆာဗာဘက်တွင် ပယ်ချပါသည်။
    - စာချုပ်အကောင်အထည်ဖော်ခြင်း `SubmitBallot` ကိုမခေါ်ယူမီ `ZK_VOTE_VERIFY_BALLOT` ကိုခေါ်ဆိုရပါမည်။ အိမ်ရှင်များသည် တစ်ချက်ချက် လက်ကိုင်ကို တွန်းအားပေးသည်။

- POST `/v1/gov/ballots/plain`
  - တောင်းဆိုချက်- { "authority": "i105...", "private_key": "…?", "chain_id": "…", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "yetain"}|A
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }
  - မှတ်စုများ- ပြန်လည်မဲပေးခြင်းသည် သက်တမ်းတိုးခြင်းသာဖြစ်သည် — မဲအသစ်သည် ရှိပြီးသားလော့ခ်၏ပမာဏ သို့မဟုတ် သက်တမ်းကုန်ဆုံးမှုကို လျှော့ချ၍မရပါ။ `owner` သည် ငွေပေးငွေယူလုပ်ပိုင်ခွင့်အာဏာနှင့် တူညီရပါမည်။ အနည်းဆုံးကြာချိန်သည် `conviction_step_blocks` ဖြစ်သည်။

- POST `/v1/gov/finalize`
  - တောင်းဆိုချက်- { "referendum_id": "r1", "proposal_id": "…64hex", "authority": "i105…?", "private_key": "…?" }
  - တုံ့ပြန်ချက်- { "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "…" }] }
  - ကွင်းဆက်အကျိုးသက်ရောက်မှု (လက်ရှိ scaffold): အတည်ပြုထားသော ဖြန့်ကျက်အဆိုပြုချက်ကို အတည်ပြုခြင်းသည် `code_hash` ဖြင့် သော့ခတ်ထားသော အနည်းငယ်မျှသာသော `code_hash` ကို ထည့်သွင်းပြီး မျှော်မှန်းထားသော `abi_hash` နှင့် အဆိုပြုချက်ကို အတည်ပြုကြောင်း အမှတ်အသားပြုပါသည်။ အခြား `abi_hash` နှင့် `code_hash` အတွက် ထင်ရှားချက်တစ်ခု ရှိနှင့်ပြီးဖြစ်ပါက၊ အတည်ပြုချက်ကို ပယ်ချပါသည်။
  - မှတ်စုများ
    - ZK ရွေးကောက်ပွဲများအတွက်၊ စာချုပ်လမ်းကြောင်းများသည် `FinalizeElection` ကိုမလုပ်ဆောင်မီ `ZK_VOTE_VERIFY_TALLY` သို့ခေါ်ဆိုရပါမည်။ အိမ်ရှင်များသည် တစ်ချက်ချက် လက်ကိုင်ကို တွန်းအားပေးသည်။ `FinalizeReferendum` သည် ရွေးကောက်ပွဲ စာရင်းဇယား အပြီးသတ်သည်အထိ ZK ဆန္ဒခံယူမှုကို ပယ်ချပါသည်။
    - `h_end` တွင် အလိုအလျောက်ပိတ်သည် ရိုးရှင်းသောဆန္ဒခံယူခြင်းအတွက်သာ အတည်ပြုပြီး/ငြင်းပယ်သည် ZK ၏ဆန္ဒခံယူချက်ကို အပြီးသတ်စာရင်းတင်သွင်းပြီး `FinalizeReferendum` ကို အပြီးသတ်သည်အထိ ပိတ်ထားသည်။
    - မဲစာရင်းစစ်ဆေးမှုများသည် approve+reject ကိုသာအသုံးပြုသည်။ မဲဆန္ဒရှင်စာရင်းတွင် ရှောင်ရန် မပါဝင်ပါ။

- POST `/v1/gov/enact`
  - တောင်းဆိုချက်- { "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "…?" }
  - တုံ့ပြန်ချက်- { "ok": true၊ "tx_instructions": [{ "wire_id": "…EnactReferendum", "payload_hex": "…" }] }
  - မှတ်စုများ- Torii သည် `authority`/`private_key` ကို ပံ့ပိုးပေးသောအခါတွင် လက်မှတ်ရေးထိုးထားသော ငွေပေးငွေယူကို တင်သွင်းပါသည်။ မဟုတ်ပါက ၎င်းသည် ဖောက်သည်များအား လက်မှတ်ရေးထိုးပြီး တင်ပြရန် အရိုးစုကို ပြန်ပေးသည်။ အကြိုဓာတ်ပုံသည် ရွေးချယ်နိုင်ပြီး လက်ရှိတွင် အချက်အလက်ဖြစ်သည်။

- `/v1/gov/proposals/{id}` ရယူပါ။
  - လမ်းကြောင်း `{id}`- အဆိုပြုချက် id hex (64 လုံး)
  - တုံ့ပြန်ချက်- { "found": bool, "proposal": { … }? }

- `/v1/gov/locks/{rid}` ရယူပါ။
  - လမ်းကြောင်း `{rid}`- ဆန္ဒခံယူပွဲ ID စာကြောင်း
  - တုံ့ပြန်ချက်- { "found": bool, "referendum_id": "rid", "locks": { … }? }

- `/v1/gov/council/current` ရယူပါ။
  - တုံ့ပြန်ချက်- { "epoch": N၊ "အဖွဲ့ဝင်များ"- [{ "account_id": "…" }, …] }
  - မှတ်စုများ- လက်ရှိရှိနေသည့် ကောင်စီကို ပြန်ပေးသည်။ သို့မဟုတ်ပါက သတ်မှတ်ထားသော အစုရှယ်ယာပိုင်ဆိုင်မှုနှင့် ကန့်သတ်ချက်များကို အသုံးပြု၍ အဆုံးအဖြတ်ပေးသည့် ဆုတ်ယုတ်မှုတစ်ခု (VRF သတ်မှတ်ချက်ကို မှန်ချပ်များပေါ်မှ VRF အထောက်အထားများ ဆက်လက်တည်ရှိနေသည်အထိ)။- ပို့စ် `/v1/gov/council/derive-vrf` (အင်္ဂါရပ်- gov_vrf)
  - တောင်းဆိုချက်- { "committee_size": 21၊ "epoch": 123? , "လျှောက်ထားသူများ"- [{ "account_id": "…", "မူကွဲ"- "ပုံမှန်|အသေးစား", "pk_b64": "…", "proof_b64": "…" }, …] }
  - အပြုအမူ- `chain_id`၊ `epoch` နှင့် နောက်ဆုံးဘလောက် hash beacon တို့မှ ဆင်းသက်လာသော canonical input ကို ဆန့်ကျင်သည့် ကိုယ်စားလှယ်တစ်ဦးစီ၏ VRF အထောက်အထားကို အတည်ပြုသည်။ Tiebreakers ဖြင့် အထွက် bytes desc ဖြင့် စီပါ။ ထိပ်တန်း `committee_size` အဖွဲ့ဝင်များကို ပြန်ပေးသည်။ မတည်မြဲပါဘူး။
  - တုံ့ပြန်ချက်- { "epoch": N၊ "အဖွဲ့ဝင်များ"- [{ "account_id": "…" } …], "total_candidates": M, "verified": K }
  - မှတ်စုများ- G1 တွင် ပုံမှန် = pk၊ G2 (96 bytes) ဖြင့် အထောက်အထား။ Small = G2 တွင် pk၊ G1 (48 bytes) ဖြင့် အထောက်အထား။ ထည့်သွင်းမှုများကို ဒိုမိန်း-ခြားထားပြီး `chain_id` ပါဝင်သည်။

### အုပ်ချုပ်မှုပုံသေများ (iroha_config `gov.*`)

ဆက်ရှိနေသောစာရင်းစာရင်းမရှိသည့်အခါ Torii မှအသုံးပြုသော council fallback ကို `iroha_config` မှတဆင့်ကန့်သတ်ထားသည်-

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

တူညီသောပတ်ဝန်းကျင်ကို အစားထိုးသည်-

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

`parliament_committee_size` သည် ကောင်စီကိုဆက်လက်မတည်မြဲသောအခါတွင် ပြန်ပေးသည့်အလှည့်ကျအဖွဲ့ဝင်အရေအတွက်ကို ဖုံးအုပ်ထားသည်၊ `parliament_term_blocks` သည် မျိုးစေ့ဆင်းသက်မှုအတွက်အသုံးပြုသည့်အချိန်ကာလကိုသတ်မှတ်သည် (`epoch = floor(height / term_blocks)`)၊ `parliament_min_stake` သည် အနိမ့်ဆုံးနှင့် အနိမ့်ဆုံးယူနစ်များအဖြစ် သတ်မှတ်ပေးသည်၊ `parliament_eligibility_asset_id` သည် ကိုယ်စားလှယ်လောင်းသတ်မှတ်မှုကို တည်ဆောက်သည့်အခါ မည်သည့်ပိုင်ဆိုင်မှုလက်ကျန်ကို စကင်ဖတ်သည်ကို ရွေးသည်။

Governance VK အတည်ပြုခြင်းတွင် ရှောင်ကွင်းမရှိပါ- မဲစိစစ်ခြင်းတွင် Inline bytes ဖြင့် စစ်ဆေးသော `Active` သော့ကို အမြဲတမ်း လိုအပ်ပြီး အတည်ပြုခြင်းအား ကျော်ရန် ပတ်ဝန်းကျင်များသည် စမ်းသပ်မှု-သီးသန့်ခလုတ်များပေါ်တွင် အားမကိုးရပါ။

RBAC
- On-chain execution သည် ခွင့်ပြုချက်များ လိုအပ်သည်-
  - အဆိုပြုချက်များ- `CanProposeContractDeployment{ contract_id }`
  - မဲများ- `CanSubmitGovernanceBallot{ referendum_id }`
  - အတည်ပြုချက်- `CanEnactGovernance`
  - ကောင်စီစီမံခန့်ခွဲမှု (အနာဂတ်): `CanManageParliament`

ကာကွယ်ထားသော အမည်နေရာများ
- စိတ်ကြိုက်ကန့်သတ်ဘောင် `gov_protected_namespaces` (JSON အခင်းအကျင်းများ) သည် စာရင်းသွင်းထားသော namespaces များသို့ ဖြန့်ကျက်ရန်အတွက် ဝင်ခွင့်တံခါးကို ဖွင့်ပေးသည်။
- ဖောက်သည်များသည် ကာကွယ်ထားသော namespaces ကို ပစ်မှတ်ထား၍ ဖြန့်ကျက်ရန်အတွက် ငွေပေးငွေယူ မက်တာဒေတာသော့များ ပါဝင်ရမည်-
  - `gov_namespace`- ပစ်မှတ်အမည်နေရာ (ဥပမာ၊ `"apps"`)
  - `gov_contract_id`- namespace အတွင်း ယုတ္တိကျသော စာချုပ် ID
- `gov_manifest_approvers`- တရားဝင်သော အကောင့် ID များ၏ ရွေးချယ်နိုင်သော JSON အခင်းအကျင်း။ လမ်းကြောတစ်ခုက တစ်ခုထက်ပိုကြီးသော quorum ကိုကြေညာသောအခါ၊ ဝင်ခွင့်သည် ထင်ရှားသောအထမြောက်မှုကို ကျေနပ်စေရန် ငွေပေးငွေယူလုပ်ပိုင်ခွင့်အာဏာနှင့် စာရင်းသွင်းထားသောအကောင့်များ လိုအပ်ပါသည်။
- Telemetry သည် `governance_manifest_admission_total{result}` မှတစ်ဆင့် လုံး၀ ဝင်ခွင့်ကောင်တာများကို ဖော်ထုတ်ပေးသောကြောင့် အော်ပရေတာများသည် `missing_manifest`၊ `non_validator_authority`၊ `quorum_rejected`၊ I18NI0000015810 နှင့် I18NI0000015810၊ နှင့် 7 လမ်းကြောင်းများကို ခွဲခြားနိုင်သည်။
- Telemetry သည် `governance_manifest_quorum_total{outcome}` (တန်ဖိုးများ `satisfied` / `rejected`) မှတစ်ဆင့် ပြဋ္ဌာန်းထားသောလမ်းကြောင်းကို ဖုံးအုပ်ပေးသောကြောင့် အော်ပရေတာများသည် ပျောက်ဆုံးနေသော အတည်ပြုချက်များကို စစ်ဆေးနိုင်သည်။
- Lanes များသည် ၎င်းတို့၏ manifests များတွင် ထုတ်ပြန်ထားသော namespace ခွင့်ပြုစာရင်းကို တွန်းအားပေးသည်။ `gov_namespace` ကို သတ်မှတ်ပေးသည့် မည်သည့် လွှဲပြောင်းမှုမဆို `gov_contract_id` ကို ပေးဆောင်ရမည် ဖြစ်ပြီး namespace သည် manifest ၏ `protected_namespaces` set တွင် ပေါ်လာရပါမည်။ အကာအကွယ်ကိုဖွင့်ထားသောအခါ `RegisterSmartContractCode` တင်ပြချက်များကို ပယ်ချပါသည်။
- tuple `(namespace, contract_id, code_hash, abi_hash)` အတွက် အတည်ပြုထားသော အုပ်ချုပ်မှု အဆိုပြုချက်တစ်ခု ရှိနေကြောင်း ဝင်ခွင့်က ပြဋ္ဌာန်းထားသည်။ မဟုတ်ပါက ခွင့်မပြုသော အမှားတစ်ခုဖြင့် အတည်ပြုခြင်း မအောင်မြင်ပါ။

Runtime Upgrade Hooks
- Lane manifests သည် runtime upgrade ညွှန်ကြားချက်များ (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) ကိုဂိတ်ပေါက်ရန် `hooks.runtime_upgrade` ကြေငြာနိုင်ပါသည်။
- ချိတ်ကွက်များ
  - `allow` (bool၊ မူရင်း `true`): `false` တွင်၊ runtime-upgrade ညွှန်ကြားချက်များအားလုံးကို ပယ်ချပါသည်။
  - `require_metadata` (bool၊ မူရင်း `false`): `metadata_key` မှ သတ်မှတ်ထားသော ငွေပေးငွေယူ မက်တာဒေတာ ထည့်သွင်းမှု လိုအပ်ပါသည်။
  - `metadata_key` (string)- ချိတ်ဖြင့် ပြဋ္ဌာန်းထားသော မက်တာဒေတာအမည်။ မက်တာဒေတာလိုအပ်သောအခါ သို့မဟုတ် ခွင့်ပြုစာရင်းရှိသည့်အခါ ပုံသေသည် `gov_upgrade_id` ဖြစ်သည်။
  - `allowed_ids` (စာကြောင်းများ၏ အခင်းအကျင်း)- မက်တာဒေတာတန်ဖိုးများ၏ ရွေးချယ်ခွင့်စာရင်း (ချုံ့ပြီးနောက်)။ ပေးထားသည့်တန်ဖိုးကို စာရင်းမသွင်းသည့်အခါ ငြင်းပယ်သည်။
- အချိတ်အဆက်ရှိနေသောအခါ၊ ငွေပေးငွေယူတန်းစီထဲသို့မဝင်မီ တန်းစီခြင်းဝင်ခွင့်သည် မက်တာဒေတာမူဝါဒကို ပြဋ္ဌာန်းသည်။ ခွင့်ပြုစာရင်းပြင်ပရှိ မက်တာဒေတာ၊ အလွတ်တန်ဖိုးများ သို့မဟုတ် တန်ဖိုးများသည် အဆုံးအဖြတ်ပေးသော `NotPermitted` အမှားအယွင်းကို ဖြစ်ပေါ်စေသည်။
- Telemetry သည် `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` မှတစ်ဆင့် ပြဋ္ဌာန်းထားသော ရလဒ်များကို ခြေရာခံသည်။
- အချိတ်အဆက်ကို ကျေနပ်စေသော ငွေပေးငွေယူများတွင် မန်နီးဖက်စ်အထအုရမ်မှ လိုအပ်သော တရားဝင်အတည်ပြုချက်မှန်သမျှနှင့်အတူ မက်တာဒေတာ `gov_upgrade_id=<value>` (သို့မဟုတ် ထင်ရှားစွာသတ်မှတ်ထားသောသော့) ပါဝင်ရပါမည်။

အဆင်ပြေမှုအဆုံးမှတ်
- POST `/v1/gov/protected-namespaces` — `gov_protected_namespaces` ကို node ပေါ်တွင် တိုက်ရိုက်သက်ရောက်သည်။
  - တောင်းဆိုချက်- { "namespaces"- ["apps", "system"] }
  - တုံ့ပြန်ချက်- { "ok": true, "applied": 1 }
  - မှတ်စုများ- admin/testing အတွက် ရည်ရွယ်ပါသည်။ ပြင်ဆင်သတ်မှတ်ပါက API တိုကင်လိုအပ်သည်။ ထုတ်လုပ်မှုအတွက်၊ `SetParameter(Custom)` ဖြင့် လက်မှတ်ရေးထိုးထားသော အရောင်းအ၀ယ်ကို တင်သွင်းခြင်းကို ပိုနှစ်သက်သည်။

CLI အကူအညီပေးသူများ
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - namespace နှင့် cross-checks အတွက် စာချုပ်ဥပမာများကို ထုတ်ယူသည်-
    - Torii သည် `code_hash` တစ်ခုစီအတွက် bytecode ကို သိမ်းဆည်းထားပြီး ၎င်း၏ Blake2b-32 အညွှန်းသည် `code_hash` နှင့် ကိုက်ညီပါသည်။
    - `/v1/contracts/code/{code_hash}` အောက်တွင် သိမ်းဆည်းထားသော မန်နီးဖက်စ်သည် `code_hash` နှင့် `abi_hash` တန်ဖိုးများနှင့် ကိုက်ညီသော အစီရင်ခံစာများ။
    - တူညီသော proposal-id ကို node အသုံးပြုမှုများကို ဟက်ခ်ျလုပ်ခြင်းဖြင့် ဆင်းသက်လာသော `(namespace, contract_id, code_hash, abi_hash)` အတွက် အတည်ပြုပြဌာန်းထားသော အုပ်ချုပ်မှု အဆိုပြုချက်တစ်ခု ရှိပါသည်။
  - စာချုပ်တစ်ခုလျှင် `results[]` ဖြင့် JSON အစီရင်ခံစာ (ပြဿနာများ၊ သရုပ်ဖော်ခြင်း/ကုဒ်/အဆိုပြုချက် အနှစ်ချုပ်များ) နှင့် (`--no-summary`) တို့ကို ဖိနှိပ်ထားခြင်းမရှိပါက တစ်ကြောင်းတစ်ကြောင်း အကျဉ်းချုပ်ကို ထုတ်ပေးပါသည်။
  - ကာကွယ်ထားသော namespace များကို စစ်ဆေးခြင်း သို့မဟုတ် အုပ်ချုပ်မှုထိန်းချုပ်ထားသော အလုပ်အသွားအလာများကို စိစစ်ခြင်းအတွက် အသုံးဝင်သည်။
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - manifest quorum စည်းမျဉ်းများကို ကျေနပ်စေရန် ရွေးချယ်နိုင်သော `gov_manifest_approvers` အပါအဝင် ကာကွယ်ထားသော namespaces များသို့ ဖြန့်ကျက်ထည့်သွင်းရာတွင် အသုံးပြုသည့် JSON မက်တာဒေတာအရိုးစုကို ထုတ်လွှတ်သည်။
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` တွင် လော့ခ်ချရန် အရိပ်အမြွက်များ လိုအပ်ပြီး ပေးထားသည့် အရိပ်အမြွက်အစုံသည် `owner`၊ `amount` နှင့် `duration_blocks` တို့ ပါဝင်ရပါမည်။
  - Canonical account ids များကို တရားဝင်စေပြီး 32-byte nullifier အရိပ်အမြွက်များကို canonicalize လုပ်ကာ အရိပ်အမြွက်များကို `public_inputs_json` (နောက်ထပ်ထပ်ဆောင်းမှုများအတွက် `--public <path>` နှင့်) ပေါင်းထည့်သည်။
  - nullifier ကို အထောက်အထား ကတိကဝတ် (အများပြည်သူ ထည့်သွင်းမှု) နှင့် `domain_tag`၊ `chain_id` နှင့် `election_id` တို့မှ ဆင်းသက်လာပါသည်။ `--nullifier` ကို ပေးသွင်းသည့်အခါ အထောက်အထားနှင့် ကိုက်ညီကြောင်း အတည်ပြုထားသည်။
  - တစ်ကြောင်းချင်းအကျဉ်းချုပ်သည် ယခုအခါတွင် ကုဒ်လုပ်ထားသော `CastZkBallot` မှ ဆင်းသက်လာသော အဆုံးအဖြတ်ပေးသည့် `fingerprint=<hex>` ကို ဖော်ပြနေပါသည်။
  - CLI တုံ့ပြန်မှုများသည် `tx_instructions[]` နှင့် `payload_fingerprint_hex` အပေါင်းကို ကုဒ်လုပ်ထားသော အကွက်များဖြင့် မှတ်သားထားသောကြောင့် ရေအောက်တူးလ်လုပ်ခြင်း Norito ကို ထပ်ကုဒ်ကုဒ်ကို ထပ်မွမ်းမံခြင်းမပြုဘဲ အရိုးစုကို အတည်ပြုနိုင်သည်။
  - သော့ခတ်ထားသော အရိပ်အမြွက်များကို ပံ့ပိုးပေးခြင်းဖြင့် ဆားကစ်သည် တူညီသောတန်ဖိုးများကို ဖော်ထုတ်ပြီးသည်နှင့် ZK မဲများအတွက် `LockCreated`/`LockExtended` ဖြစ်ရပ်များကို ထုတ်လွှတ်နိုင်စေပါသည်။
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` သည် canonical I105 စာလုံးများကို လက်ခံသည် ။ ရွေးချယ်နိုင်သော `@<domain>` ၏ နောက်ဆက်တွဲများသည် လမ်းပြခြင်းအရိပ်အမြွက်များသာဖြစ်သည်။
  - Aliases `--lock-amount`/`--lock-duration-blocks` သည် scripting parity အတွက် ZK အလံအမည်များကို ထင်ဟပ်စေသည်။
  - ကုဒ်လုပ်ထားသော ညွှန်ကြားချက်လက်ဗွေနှင့် လူသားဖတ်နိုင်သောမဲကွက်များ (`owner`၊ `amount`၊ `duration_blocks`၊ I18NI00000226eleX) တို့ကို မပံ့ပိုးမီ ကုဒ်လုပ်ထားသော လမ်းညွှန်လက်ဗွေများ အပါအဝင် အကျဉ်းချုပ် အထွက်ကြေးမုံမှန်များ `vote --mode zk`၊

သာဓကများစာရင်း
- `/v1/gov/instances/{ns}` ကိုရယူပါ — namespace အတွက် တက်ကြွသော စာချုပ်ဖြစ်ရပ်များကို စာရင်းပြုစုပါ။
  - Query params များ-
    - `contains`- `contract_id` ၏ စာလုံးခွဲများဖြင့် စစ်ထုတ်ပါ
    - `hash_prefix`- `code_hash_hex` ၏ hex ရှေ့ဆက်ဖြင့် စစ်ထုတ်ခြင်း
    - `offset` (မူလ 0)၊ `limit` (မူလ 100၊ အများဆုံး 10_000)
    - `order`- `cid_asc` (မူရင်း)၊ `cid_desc`၊ `hash_asc`၊ `hash_desc`
  - တုံ့ပြန်မှု- { "namespace": "ns", "instances": [{ "contract_id": "…", "code_hash_hex": "…" }, …], "total": N, "offset": n, "limit": m }
  - SDK အကူအညီပေးသူ- `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) သို့မဟုတ် `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)။

Sweep ကို လော့ခ်ဖွင့်ပါ (အော်ပရေတာ/စာရင်းစစ်)
- `/v1/gov/unlocks/stats` ရယူပါ။
  - တုံ့ပြန်မှု- { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - မှတ်စုများ- `last_sweep_height` သည် သက်တမ်းလွန်သော့ခလောက်များကို သုတ်သင်ပြီး ဆက်လက်တည်ရှိနေသည့် လတ်တလော ဘလောက်အမြင့်ကို ထင်ဟပ်စေသည်။ `expired_locks_now` ကို `expiry_height <= height_current` ဖြင့် လော့ခ်ချခြင်းမှတ်တမ်းများကို စကင်န်ဖတ်ခြင်းဖြင့်တွက်ချက်ပါသည်။
- POST `/v1/gov/ballots/zk-v1`
  - တောင်းဆိုချက် (v1-စတိုင် DTO):
    {
      "authority": "i105..."၊
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?"၊
      "election_id": "ကိုးကား-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ="၊
      "root_hint": "0x…64hex?"၊
      "ပိုင်ရှင်": "i105…?", // canonical AccountId (I105 ပကတိ)
      "ပမာဏ": "100?"၊
      "duration_blocks": 6000?၊
      "ဦးတည်ချက်"- "အေး|နေ|နေသလား?"၊
      "nullifier": "blake2b32:…64hex?"
    }
  - တုံ့ပြန်ချက်- { "ok": true၊ "accepted": true, "tx_instructions": [{…}] }- ပို့စ် `/v1/gov/ballots/zk-v1/ballot-proof` (အင်္ဂါရပ်- `zk-ballot`)
  - `BallotProof` JSON ကို တိုက်ရိုက်လက်ခံပြီး `CastZkBallot` အရိုးစုကို ပြန်ပေးသည်။
  - တောင်းဆိုချက်-
    {
      "authority": "i105..."၊
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?"၊
      "election_id": "ကိုးကား-1",
      "မဲ"- {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ="၊ ZK1 သို့မဟုတ် H2* ကွန်တိန်နာ၏ // base64
        "root_hint": null၊ // ရွေးချယ်နိုင်သော 32-byte hex string (အရည်အချင်းပြည့်မီမှု အမြစ်)
        "ပိုင်ရှင်"- null၊ // ရွေးချယ်နိုင်သော canonical AccountId (I105 ပကတိ)
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
    - ဆာဗာသည် စိတ်ကြိုက်ရွေးချယ်နိုင်သော `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` အား မဲစနစ်မှ `nullifier` မှ `nullifier` `CastZkBallot`။
    - စာအိတ်ဘိုက်များကို ညွှန်ကြားချက်ပေးဆောင်မှုအတွက် base64 အဖြစ် ပြန်လည်ကုဒ်လုပ်ထားပါသည်။
    - Torii မဲစာရင်းတင်သွင်းသောအခါ `reason` တုံ့ပြန်မှုသည် `submitted transaction` သို့ပြောင်းသည်။
    - `zk-ballot` အင်္ဂါရပ်ကို ဖွင့်ထားမှသာ ဤအဆုံးမှတ်ကို ရရှိနိုင်သည်။

CastZkBallot အတည်ပြုခြင်းလမ်းကြောင်း
- `CastZkBallot` သည် ပံ့ပိုးပေးထားသော base64 အထောက်အထားကို ကုဒ်ဖျက်ပြီး ဗလာ သို့မဟုတ် ပုံစံမမှန်သော payloads (`BallotRejected` with `invalid or empty proof`) ကို ငြင်းပယ်သည်။
- အကယ်၍ `public_inputs_json` ကို ထောက်ပံ့ပေးပါက၊ ၎င်းသည် JSON အရာဝတ္ထုတစ်ခု ဖြစ်ရပါမည်။ အရာဝတ္ထုမဟုတ်သော payload များကို ငြင်းပယ်သည်။
- အိမ်ရှင်သည် ဆန္ဒခံယူပွဲ (`vk_ballot`) သို့မဟုတ် အုပ်ချုပ်မှုပုံသေများမှ မဲစိစစ်သည့်သော့ကို ဖြေရှင်းပေးပြီး မှတ်တမ်းတည်ရှိရန် လိုအပ်သည်၊ `Active` ဖြစ်ရန်နှင့် inline bytes သယ်ဆောင်ပါ။
- သိမ်းဆည်းထားသော အတည်ပြုသော့ဘိုက်များကို `hash_vk` ဖြင့် ပြန်လည် ဟက်ခ်လုပ်ထားပါသည်။ ကတိကဝတ်များ မကိုက်ညီပါက ဖျက်လိုက်သော registry entries များကို ကာကွယ်ရန် စိစစ်ခြင်းမပြုမီ ကတိကဝတ်များ ပျက်သွားသည် (`BallotRejected` with `verifying key commitment mismatch`)။
- အထောက်အထားဘိုက်များကို `zk::verify_backend` မှတစ်ဆင့် စာရင်းသွင်းထားသော နောက်ခံသို့ ပေးပို့သည်။ `invalid proof` ဖြင့် `BallotRejected` အဖြစ် မမှန်ကန်သော စာသားများ ပေါ်လာပြီး ညွှန်ကြားချက်သည် တိကျစွာ ပျက်ကွက်ပါသည်။
- အထောက်အထားသည် အများသူငှာ သွင်းအားစုများအဖြစ် မဲကတိကဝတ်နှင့် အရည်အချင်းပြည့်မီမှု အရင်းမြစ်ကို ဖော်ထုတ်ရမည်။ အမြစ်သည် ရွေးကောက်ပွဲ၏ `eligible_root` နှင့် ကိုက်ညီရမည်ဖြစ်ပြီး ဆင်းသက်လာသော nullifier သည် ပေးထားသည့် မည်သည့်အရိပ်အမြွက်နှင့်မဆို ကိုက်ညီရပါမည်။
- အောင်မြင်သောအထောက်အထားများသည် `BallotAccepted` ကိုထုတ်လွှတ်သည်။ ပွားနေသော nullifiers၊ ဟောင်းနွမ်းနေသော အရည်အသွေးပြည့်မီမှု အမြစ်များ သို့မဟုတ် လော့ခ်ချခြင်း ဆုတ်ယုတ်မှုများသည် ဤစာတမ်းတွင် ဖော်ပြထားသော ရှိပြီးသား ငြင်းပယ်ခြင်းဆိုင်ရာ အကြောင်းပြချက်များကို ဆက်လက်ဖြစ်ပေါ်စေပါသည်။

## မမှန်မကန်ပြုမူမှုနှင့် ပူးတွဲသဘောဆန္ဒကို အတည်ပြုသည်။

### ခုတ်ထစ်ခြင်းနှင့် ထောင်ချခြင်းလုပ်ငန်း

အတည်ပြုသူသည် ပရိုတိုကောကိုချိုးဖောက်သည့်အခါတိုင်း သဘောတူညီမှုသည် Norito-ကုဒ်လုပ်ထားသော `Evidence` ကို ထုတ်ပေးသည်။ payload တစ်ခုစီသည် in-memory `EvidenceStore` တွင်ရှိပြီး၊ မမြင်ရပါက WSV ကျောထောက်နောက်ခံပြုထားသော `consensus_evidence` မြေပုံတွင် ရုပ်လုံးပေါ်လာပါသည်။ `sumeragi.npos.reconfig.evidence_horizon_blocks` ထက်ဟောင်းသော မှတ်တမ်းများ (မူလ `7 200` ဘလောက်များ) ကို ပယ်ချသည် ထို့ကြောင့် မော်ကွန်းကို ကန့်သတ်ထားသော်လည်း ငြင်းပယ်ခြင်းကို အော်ပရေတာများအတွက် မှတ်တမ်းတင်ထားသည်။ မိုးကုပ်စက်ဝိုင်းအတွင်းမှ အထောက်အထားများသည် ပူးတွဲသဘောတူညီချက် (`mode_activation_height requires next_mode to be set in the same block`)၊ စတင်ခြင်းနှောင့်နှေးခြင်း (`sumeragi.npos.reconfig.activation_lag_blocks`၊ မူရင်း `1`) နှင့် မျဉ်းစောင်းနှောင့်နှေးခြင်း (I18NI0000028380204 ပုံမှန် I18NI0000028380204 I180) မလျှောက်ထားမီ ပြစ်ဒဏ်များကို ပယ်ဖျက်ပါ။

အသိအမှတ်ပြုထားသော ပြစ်မှုများသည် `EvidenceKind` သို့ တစ်ပုံမှတစ်ပုံ မြေပုံညွှန်းပေးသည်။ ခွဲခြားဆက်ဆံသူများသည် ဒေတာပုံစံဖြင့် တည်ငြိမ်ပြီး ကျင့်သုံးသည်-

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

- **DoublePrepare/DoubleCommit** — တရားဝင်သူသည် တူညီသော `(phase,height,view,epoch)` tuple အတွက် ကွဲလွဲနေသော hashe များကို လက်မှတ်ရေးထိုးခဲ့သည်။
- **InvalidQc** — စုစည်းသူသည် အဆုံးအဖြတ်စစ်ဆေးမှုများ ပျက်ကွက်သည့် ပုံသဏ္ဍာန်ရှိ ကတိပြုလက်မှတ် (ဥပမာ၊ အချည်းနှီးသော လက်မှတ်ထိုးသူ ဘစ်မြေပုံ) ကို အတင်းအဖျင်းပြောပါသည်။
- **InvalidProposal** — ခေါင်းဆောင်တစ်ဦးသည် ဖွဲ့စည်းတည်ဆောက်ပုံဆိုင်ရာ စစ်ဆေးအတည်ပြုခြင်းမှ ပျက်ကွက်သောပိတ်ဆို့တစ်ခု (ဥပမာ၊ သော့ခတ်ထားသောကွင်းဆက်စည်းမျဉ်းကို ချိုးဖောက်သည်)။
- **ဆင်ဆာဖြတ်တောက်ခြင်း** — လက်မှတ်ရေးထိုးထားသော တင်သွင်းမှုပြေစာများသည် အဆိုပြုခြင်း/ကတိက၀တ် မပြုဖူးသော အရောင်းအ၀ယ်တစ်ခုကို ပြသသည်။

VRF ပြစ်ဒဏ်များသည် `activation_lag_blocks` (ပြစ်မှုကျူးလွန်သူများကို ထောင်သွင်းအကျဉ်းချခြင်း) ပြီးနောက် အလိုအလျောက် ပြဋ္ဌာန်းပါသည်။ အုပ်ချုပ်ရေးမှ ပြစ်ဒဏ်ကို မပယ်ဖျက်ပါက `slashing_delay_blocks` ဝင်းဒိုးပြီးနောက်မှသာ အများသဘောတူချက်ကို ဖြတ်တောက်ခြင်းကို အသုံးပြုပါသည်။

အော်ပရေတာများနှင့် ကိရိယာတန်ဆာပလာများသည် ဝန်ဆောင်ခများကို စစ်ဆေးပြီး ပြန်လည်ထုတ်လွှင့်နိုင်သည်-

- Torii: `GET /v1/sumeragi/evidence` နှင့် `GET /v1/sumeragi/evidence/count`။
- CLI- `iroha ops sumeragi evidence list`၊ `… count` နှင့် `… submit --evidence-hex <payload>`။

အုပ်ချုပ်ရေးသည် သက်သေ ဘိုက်များကို ကျမ်းဂန်အထောက်အထားအဖြစ် ဆက်ဆံရမည်-

1. **သက်တမ်းမကုန်မီ payload** ကိုစုဆောင်းပါ။ အကြမ်းထည် Norito bytes ကို အမြင့်/ကြည့်ရှုမှု မက်တာဒေတာနှင့်အတူ သိမ်းဆည်းပါ။
2. လိုအပ်ပါက `CancelConsensusEvidencePenalty` ကို `slashing_delay_blocks` မတိုင်မီ အထောက်အထားပေးဆောင်မှုနှင့်အတူ တင်သွင်းခြင်းဖြင့် **Cancel**၊ မှတ်တမ်းကို `penalty_cancelled` နှင့် `penalty_cancelled_at_height` ဟု အမှတ်အသားပြုထားပြီး မျဉ်းစောင်းများ သက်ရောက်မှုမရှိပါ။
3. လူထုဆန္ဒခံယူပွဲ သို့မဟုတ် sudo ညွှန်ကြားချက် (ဥပမာ၊ `Unregister::peer`). လုပ်ဆောင်ချက်သည် ဝန်ဆောင်ခကို ပြန်လည်အတည်ပြုသည်။ ပုံသဏ္ဍာန်မမှန်သော၊
4. **နောက်ဆက်တွဲ topology ကို အချိန်ဇယားဆွဲပါ** ထို့ကြောင့် ပြစ်မှုကျူးလွန်ကြောင်း အတည်ပြုသူသည် ချက်ချင်းပြန်မပါဝင်နိုင်ပါ။ အပ်ဒိတ်စာရင်းဇယားနှင့်အတူ ရိုးရိုးစီးဆင်းမှုများ `SetParameter(Sumeragi::NextMode)` နှင့် `SetParameter(Sumeragi::ModeActivationHeight)` ။
5. **စာရင်းစစ်ရလဒ်** သည် `/v1/sumeragi/evidence` နှင့် `/v1/sumeragi/status` မှတစ်ဆင့် သက်သေအထောက်အထားများ တန်ပြန်အဆင့်မြင့်ပြီး အုပ်ချုပ်ရေးမှ ဖယ်ရှားခြင်းကို အတည်ပြုကြောင်း သေချာစေရန်။

### Joint-Consensus Sequencing

အစုအဝေးအသစ်ကို စတင်အဆိုပြုခြင်းမစတင်မီ အကျုံးဝင်သော တရားဝင်သတ်မှတ်သူသည် နယ်နိမိတ်ပိတ်ဆို့ခြင်းကို အပြီးသတ်ကြောင်း ပူးတွဲသဘောတူချက်က အာမခံပါသည်။ runtime သည် တွဲထားသော ဘောင်များမှတစ်ဆင့် စည်းမျဉ်းကို ပြဋ္ဌာန်းသည်-

- `SumeragiParameter::NextMode` နှင့် `SumeragiParameter::ModeActivationHeight` သည် **တူညီသောပိတ်ဆို့ခြင်း** တွင် ကျူးလွန်ရပါမည်။ `mode_activation_height` သည် အပ်ဒိတ်ကို သယ်ဆောင်သည့် ဘလောက်အမြင့်ထက် တင်းကြပ်စွာ ကြီးနေရမည် ဖြစ်ပြီး အနည်းဆုံး ဘလောက် နောက်ကျခြင်းကို ပေးဆောင်သည်။
- `sumeragi.npos.reconfig.activation_lag_blocks` (မူလ `1`) သည် လုံးဝလက်မလျှော့ခြင်းများကို တားဆီးပေးသည့် ဖွဲ့စည်းမှုအစောင့်အကြပ်ဖြစ်သည်-
- `sumeragi.npos.reconfig.slashing_delay_blocks` (မူလ `259200`) သည် အများဆန္ဒကို ဖြတ်တောက်ခြင်းကို နှောင့်နှေးစေသောကြောင့် အုပ်ချုပ်ရေးက ၎င်းတို့မကျင့်သုံးမီ ပြစ်ဒဏ်များကို ပယ်ဖျက်နိုင်သည်။

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- runtime နှင့် CLI သည် `/v1/sumeragi/params` နှင့် `iroha --output-format text ops sumeragi params` မှတဆင့် အဆင့်လိုက်သတ်မှတ်ထားသောဘောင်များကို ဖော်ထုတ်ပေးသောကြောင့် အော်ပရေတာများသည် activation အမြင့်များနှင့် validator စာရင်းများကို အတည်ပြုနိုင်ပါသည်။
- အုပ်ချုပ်မှု အလိုအလျောက်စနစ်သည် အမြဲရှိသင့်သည်-
  1. အထောက်အထား-ကျောထောက်နောက်ခံပြုထားသော ဖယ်ရှားခြင်း (သို့မဟုတ် ပြန်လည်ထည့်သွင်းခြင်း) ဆုံးဖြတ်ချက်ကို အပြီးသတ်ပါ။
  2. `mode_activation_height = h_current + activation_lag_blocks` ဖြင့် နောက်ဆက်တွဲ ပြင်ဆင်မှုကို တန်းစီပါ။
  3. `/v1/sumeragi/status` ကို `effective_consensus_mode` အထိ မျှော်လင့်ထားသည့် အမြင့်သို့ ပြန်လှန်ပါ။

အတည်ပြုသူများကို လှည့်ပတ်သည့် သို့မဟုတ် ဖြတ်တောက်ခြင်းကို အသုံးချသည့် မည်သည့် script မဆို ** လုံးဝလက်မကျန်အောင် အသက်သွင်းရန် ကြိုးစားခြင်း သို့မဟုတ် hand-off parameters များကို ချန်လှပ်ခြင်းမပြုရပါ။ ထိုသို့သော လွှဲပြောင်းမှုများကို ပယ်ချပြီး ကွန်ရက်ကို ယခင်မုဒ်တွင် ထားလိုက်ပါ။

## Telemetry မျက်နှာပြင်များ

- Prometheus မက်ထရစ်များ ပို့ကုန်အုပ်ချုပ်ရေး လုပ်ဆောင်ချက်-
  - `governance_proposals_status{status}` (gauge) သည် အခြေအနေအလိုက် အဆိုပြုချက်ရေတွက်မှုကို ခြေရာခံသည်။
  - ကာကွယ်ထားသော namespace ဝင်ခွင့်ခွင့်ပြုခြင်း သို့မဟုတ် ငြင်းဆိုသည့်အခါ `governance_protected_namespace_total{outcome}` (ကောင်တာ) တိုးခြင်းများ။
  - `governance_manifest_activations_total{event}` (ကောင်တာ) သည် ထင်ရှားသောထည့်သွင်းမှုများ (`event="manifest_inserted"`) နှင့် namespace binding (`event="instance_bound"`) တို့ကို မှတ်တမ်းတင်သည်။
- `/status` တွင် `governance` သည် အဆိုပြုချက်အရေအတွက်များကို ထင်ဟပ်ပြသပေးသည့် အရာတစ်ခု၊ ကာကွယ်ထားသော namespace စုစုပေါင်းများကို အစီရင်ခံခြင်းနှင့် မကြာသေးမီက ထင်ရှားသောအသက်သွင်းမှုများ (namespace၊ စာချုပ် id၊ ကုဒ်/ABI ဟက်ရှ်၊ ပိတ်ဆို့အမြင့်၊ စဖွင့်ချိန်တံဆိပ်) တို့ ပါဝင်ပါသည်။ ပြဋ္ဌာန်းချက်များ မွမ်းမံထားသော မန်နီးဖက်စ်များနှင့် ကာကွယ်ထားသော namespace ဂိတ်များကို ပြဋ္ဌာန်းထားကြောင်း အတည်ပြုရန် အော်ပရေတာများသည် ဤအကွက်ကို စစ်တမ်းကောက်ယူနိုင်သည်။
- Grafana နမူနာပုံစံ (`docs/source/grafana_governance_constraints.json`) နှင့်
  `telemetry.md` ရှိ telemetry runbook တွင် ချိတ်မိစေရန် အချက်ပေးသံများကို ဝိုင်ယာကြိုးတပ်နည်းကို ပြသည်
  အဆိုပြုချက်များ၊ ထင်ရှားသော အသက်သွင်းမှုများ ပျောက်ဆုံးနေခြင်း၊ သို့မဟုတ် မျှော်လင့်မထားသော ကာကွယ်ထားသော အမည်နေရာလွတ်များ
  runtime အဆင့်မြှင့်တင်မှုများအတွင်း ငြင်းပယ်မှုများ။