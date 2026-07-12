---
lang: my
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality အထောက်အထားများ

ဤစာတမ်းသည် ပထမဆုံး release အတွက် bridge finality format ကို သတ်မှတ်သည်။ အထောက်အထားသည်
Sumeragi v2 က ဖန်တီးပြီး အမြဲတမ်းသိမ်းထားသည့် အတိအကျ finality evidence ကို သယ်ဆောင်သည်။
Proof envelope ၏ schema version သည် `1` ဖြစ်ပြီး အတွင်းရှိ consensus protocol version သည်
`2` ဖြစ်သည်။ Sumeragi v1 certificate projection၊ decoder သို့မဟုတ် fallback လမ်းကြောင်း မရှိပါ။

## အတိအကျ proof format

Norito သို့မဟုတ် Norito JSON ဖြင့် encode လုပ်ထားသော `BridgeFinalityProof` တွင် field သုံးခုသာရှိသည်။

```text
{ version, block_header, finality_artifact }
```

- `version` သည် `1` ဖြစ်ရမည်။
- `block_header` သည် တောင်းဆိုထားသော height ၏ canonical `BlockHeader` ဖြစ်သည်။
- `finality_artifact` သည် ထို block အတွက် သိမ်းထားသော အတိအကျ `V2FinalityArtifact` ဖြစ်သည်။
  ၎င်းသည် height-context roster အစဉ်အတိုင်း validator တစ်ဦးချင်း၏ BLS-normal PoP
  (`validator_set_pops`) ကို အမြဲတမ်း ထည့်သွင်းသိမ်းထားသည်။

Artifact တွင် ပြည့်စုံပြီး မပြောင်းလဲနိုင်သော `HeightContext`၊ အတိအကျ `BlockSubject`၊ block hash၊
CommitQC နှင့် roster-aligned PoP များ ပါဝင်သည်။ Height context သည် chain၊ epoch၊ roster၊
`DualQuorum`၊ DA layout၊ leader seed နှင့် အခြား consensus data များကို freeze လုပ်သည်။ Epoch ကို
အဆုံးသတ်သော parent block ၏ context တွင် optional `next_epoch_snapshot` လည်း ပါသည်။ ဤ field သည်
context id ၏ အစိတ်အပိုင်းဖြစ်သောကြောင့် child roster ကို ခွင့်ပြုမီ parent CommitQC က authenticate
လုပ်ထားသည်။ Finalized snapshot သည် နောက် epoch parameters များနှင့်အတူ
`epoch_end_height` နှင့် နောက် roster-aligned `validator_set_pops` ကိုလည်း authenticate လုပ်သည်။

## အမြဲတမ်းသိမ်းဆည်းမှုနှင့် စစ်ဆေးမှု

Finality မထုတ်ပြန်မီ သို့မဟုတ် block body မဖယ်ရှားမီ Kura သည် exact canonical header နှင့်
root-authenticated SCCP archive ကို immutable retained-block record တွင် ရေးပြီး exact V2
artifact ကို သီးခြား immutable finality record တွင် သိမ်းသည်။ Record နှစ်ခုလုံး idempotent ဖြစ်ပြီး
height တူ conflict ကို ငြင်းပယ်သည်။ `build_finality_proof` သည် retained header နှင့် verified
finality record ကိုသာ ဖတ်ပြီး historical block body သို့မဟုတ် mutable world state PoP ကို မသုံးပါ။
Restart တွင် header/archive/artifact/hash association ကို ပြန်စစ်သည်။ Body eviction ကြောင့် မှန်ကန်သော
proof မပျောက်ပါ။ ပျောက်ဆုံး၊ ပျက်စီး၊ conflict ဖြစ်သော သို့မဟုတ် မစစ်နိုင်သော record သည် fail closed ဖြစ်သည်။

Stateless verifier သည် version၊ chain၊ height၊ header hash၊ header ၏ canonical predecessor နှင့်
view၊ context၊ subject နှင့် CommitQC ကို အတိအကျ ကိုက်ညီစေပြီး artifact ထဲရှိ PoP အားလုံးကို
စစ်ဆေးသည်။ Signer index များသည် တင်းကျပ်စွာ
တိုးလာပြီး range အတွင်းရှိရမည်။ CommitQC သည် validator count နှင့် voting power quorum နှစ်ခုလုံးကို
ဖြည့်ဆည်းရမည်ဖြစ်ပြီး အတိအကျ Sumeragi v2 vote preimage ပေါ်ရှိ BLS aggregate signature သည် valid
ဖြစ်ရမည်။

## Trust anchor နှင့် successor စစ်ဆေးမှု

သီးခြား proof တစ်ခုသည် ၎င်းနှင့်အတူပါသော roster အောက်တွင် အတွင်းပိုင်းညီညွတ်မှုကိုသာ ပြသည်။
`BridgeFinalityVerifier` သည် ပထမ proof ကို လက်မခံမီ ရှင်းလင်းစွာ trusted ဖြစ်သော
`HeightContextId` ကို လိုအပ်သည်။ ထို့နောက် ချက်ချင်းနောက် height ကိုသာ လက်ခံပြီး child context ၏
parent CommitQC ကို ယခင် frozen roster နှင့် PoP ဖြင့် စစ်ဆေးသည်။ Epoch အတွင်း child artifact သည်
ယခင် artifact PoP များကို copy လုပ်သည်။ Boundary တွင် epoch၊ roster၊ quorum၊ seed နှင့် PoP များသည်
parent CommitQC က authenticate လုပ်ထားသော `next_epoch_snapshot` နှင့် ၎င်း၏
`epoch_end_height` အပါအဝင် ကိုက်ညီရမည်။ အဟောင်း၊ ကျော်သွားသော၊ ချိတ်ဆက်မထားသော height များကို ပယ်ချသည်။

SCCP သည် တူညီသော `BridgeFinalityProof` ကို အသုံးပြုသည်။ Message ကပေးသော roster အောက်ရှိ signature
တစ်ခုတည်းကို မယုံကြည်ရပါ။ Governance ဖြင့် ချိတ်ထားသော checkpoint context/artifact မှ message
artifact အထိ immediate successor တစ်ခုချင်းကို စစ်ဆေးရမည်။

## Bundle နှင့် API

`BridgeFinalityBundle` သည် အတိအကျ `{ commitment, finality_proof }` ဖြစ်သည်။ Commitment သည်
`{ chain_id, height_context_id, block_height, block_hash }` ဖြစ်သည်။

- `GET /v1/bridge/finality/{height}` သည် `BridgeFinalityProof` ကို ပြန်ပေးသည်။
- `GET /v1/bridge/finality/bundle/{height}` သည် `BridgeFinalityBundle` ကို ပြန်ပေးသည်။

Retained canonical header သို့မဟုတ် အတိအကျ အမြဲတမ်း v2 artifact မရှိခြင်း သို့မဟုတ် invalid
ဖြစ်ခြင်းတွင် endpoint နှစ်ခုလုံး fail closed ဖြစ်သည်။ Historical block body eviction ကြောင့် မှန်ကန်သော
proof မပျောက်ပါ။ မသိသော field များ၊ မထောက်ပံ့သော version များနှင့် retired proof shape များကို ပယ်ချရမည်။
