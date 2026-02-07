---
lang: my
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#IVM Bytecode ခေါင်းစီး


မျက်လှည့်
- 4 bytes- ASCII `IVM\0` offset 0။

အပြင်အဆင် (လက်ရှိ)
- အော့ဖ်ဆက်များနှင့် အရွယ်အစားများ (စုစုပေါင်း 17 bytes):
  - 0..4: မှော် `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6- `mode: u8` (feature bits; အောက်တွင်ကြည့်ပါ)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (အသေးဆုံးအဆင့်)
  - 16: `abi_version: u8`

မုဒ်နည်းနည်း
- `ZK = 0x01`၊ `VECTOR = 0x02`၊ `HTM = 0x04` (သီးသန့်/အင်္ဂါရပ်များ ပိတ်ထားသည်)။

နယ်ပယ်များ (အဓိပ္ပါယ်)
- `abi_version`- syscall table နှင့် pointer-ABI schema ဗားရှင်း။
- `mode`- ZK ခြေရာခံခြင်း/VECTOR/HTM အတွက် အင်္ဂါရပ်များ
- `vector_length`- vector ops အတွက် logical vector length (0 → unset)။
- `max_cycles`- ZK မုဒ်နှင့် ဝင်ခွင့်တွင် အသုံးပြုသော ကွပ်မျက်ရေးအကွက်ဘောင်။

မှတ်စုများ
- Endianness နှင့် layout ကို အကောင်အထည်ဖော်ခြင်းဖြင့် သတ်မှတ်ပြီး `version` နှင့် ချည်နှောင်ထားသည်။ အထက်ပါ on-wire layout သည် `crates/ivm_abi/src/metadata.rs` တွင် လက်ရှိအကောင်အထည်ဖော်မှုကို ထင်ဟပ်စေသည်။
- အနည်းငယ်မျှသောစာဖတ်သူသည် လက်ရှိရှေးဟောင်းပစ္စည်းများအတွက် ဤအပြင်အဆင်ကို အားကိုးနိုင်ပြီး `version` gating မှတစ်ဆင့် အနာဂတ်အပြောင်းအလဲများကို ကိုင်တွယ်သင့်သည်။
- ဟာ့ဒ်ဝဲအရှိန်မြှင့်ခြင်း (SIMD/Metal/CUDA) သည် လက်ခံသူတိုင်းအတွက် ရွေးချယ်မှုဖြစ်သည်။ runtime သည် `AccelerationConfig` မှ `AccelerationConfig` တန်ဖိုးများကို ဖတ်ပြသည်- `enable_simd` သည် မမှန်သည့်အခါ scalar fallback များကို တွန်းအားပေးပြီး `enable_metal` နှင့် `enable_cuda` တို့သည် သက်ဆိုင်ရာ backends များတွင် ၎င်းတို့၏ သက်ဆိုင်ရာ backends များမှတဆင့် gate များကို အသုံးချနေချိန်တွင်ပင်။ VM ဖန်တီးခြင်းမပြုမီ `ivm::set_acceleration_config`။
- Mobile SDKs (Android/Swift) သည် တူညီသော အဖုများကို မျက်နှာပြင်ပေါ်ရှိ၊ `IrohaSwift.AccelerationSettings`
  `connect_norito_set_acceleration_config` ကိုခေါ်ဆိုသောကြောင့် macOS/iOS တည်ဆောက်မှုများသည် Metal/
  အဆုံးအဖြတ်ပေးသော ဆုတ်ယုတ်မှုများကို ထိန်းသိမ်းနေစဉ် NEON
- အော်ပရေတာများသည် `IVM_DISABLE_METAL=1` သို့မဟုတ် `IVM_DISABLE_CUDA=1` ကို တင်ပို့ခြင်းဖြင့် ရောဂါရှာဖွေခြင်းအတွက် သီးခြား backend များကို ပိတ်နိုင်သည်။ ဤပတ်ဝန်းကျင်သည် ပြုပြင်ဖွဲ့စည်းမှုထက် သာလွန်ပြီး VM ကို အဆုံးအဖြတ်ရှိသော CPU လမ်းကြောင်းပေါ်တွင် ထားရှိပါ။

တာရှည်ခံသောပြည်နယ်အကူအညီများနှင့် ABI မျက်နှာပြင်
- တာရှည်ခံသော အခြေအနေအကူအညီပေးသည့် syscalls (0x50–0x5A- STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* နှင့် JSON/SCHEMA ကုဒ်/ကုဒ်) များသည် V1 ABI ၏ တစ်စိတ်တစ်ပိုင်းဖြစ်ပြီး `abi_hash` တွက်ချက်မှုတွင် ပါဝင်ပါသည်။
- CoreHost ဝါယာကြိုးများသည် STATE_{GET,SET,DEL} သို့ WSV-ကျောထောက်နောက်ခံပြု တာရှည်ခံစမတ်-စာချုပ်အခြေအနေ၊ dev/test hosts များသည် ထပ်ဆင့်များ သို့မဟုတ် local persistence ကို အသုံးပြုသော်လည်း တူညီသော မှတ်သားနိုင်သော အပြုအမူကို ထိန်းသိမ်းထားရပါမည်။

အတည်ပြုချက်
- Node ဝင်ခွင့်သည် `version_major = 1` နှင့် `version_minor = 0` ခေါင်းစီးများကိုသာ လက်ခံသည်။
- `mode` တွင် လူသိများသော bits များသာ ပါဝင်ရပါမည်- `ZK`, `VECTOR`, `HTM` (အမည်မသိဘစ်များကို ငြင်းပယ်သည်)။
- `vector_length` သည် အကြံပေးချက်ဖြစ်ပြီး `VECTOR` ဘစ်ကို မသတ်မှတ်ထားလျှင်တောင် သုညမဟုတ်နိုင်ပါ။ ဝင်ခွင့်သည် အထက်တန်းဘောင်တစ်ခုသာ ပြဋ္ဌာန်းသည်။
- ပံ့ပိုးထားသော `abi_version` တန်ဖိုးများ- ပထမထုတ်ဝေမှုသည် `1` (V1) ကိုသာ လက်ခံပါသည်။ အခြားတန်ဖိုးများကို ဝင်ခွင့်တွင် ငြင်းပယ်ပါသည်။

### မူဝါဒ (ထုတ်ပေးသည်)၊
အောက်ပါမူဝါဒအနှစ်ချုပ်ကို အကောင်အထည်ဖော်မှုမှထုတ်ပေးပြီး ကိုယ်တိုင်တည်းဖြတ်ခြင်းမပြုသင့်ပါ။<!-- BEGIN GENERATED HEADER POLICY -->
| လယ် | မူဝါဒ |
|---|---|
| version_major | ၁ |
| version_minor | 0 |
| မုဒ် (လူသိများသော bits) | 0x07 (ZK=0x01၊ VECTOR=0x02၊ HTM=0x04) |
| abi_version | ၁ |
| vector_length | 0 သို့မဟုတ် 1..=64 (အကြံပေးချက်; VECTOR ဘစ်၏ သီးခြား) |
<!-- END GENERATED HEADER POLICY -->

### ABI Hashes (ထုတ်ပေးသည်)
အောက်ပါဇယားကို အကောင်အထည်ဖော်မှုမှထုတ်ပေးပြီး ပံ့ပိုးထားသောမူဝါဒများအတွက် canonical `abi_hash` တန်ဖိုးများကို စာရင်းပြုစုထားသည်။

<!-- BEGIN GENERATED ABI HASHES -->
| မူဝါဒ | abi_hash (hex) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- အသေးစားမွမ်းမံမှုများသည် `feature_bits` ၏နောက်ကွယ်ရှိ ညွှန်ကြားချက်များနှင့် သီးသန့် opcode နေရာလွတ်များကို ပေါင်းထည့်နိုင်သည်။ အကြီးစား အပ်ဒိတ်များသည် ပရိုတိုကော အဆင့်မြှင့်တင်မှုနှင့်အတူသာ ကုဒ်နံပါတ်များကို ပြောင်းလဲခြင်း သို့မဟုတ် ဖယ်ရှားခြင်း/ပြန်လည်အသုံးပြုခြင်း ဖြစ်နိုင်သည်။
- Syscall အပိုင်းများသည် တည်ငြိမ်သည်။ လက်ရှိအသုံးပြုနေသော `abi_version` သည် `E_SCALL_UNKNOWN` အတွက် အမည်မသိ။
- ဓာတ်ငွေ့အချိန်ဇယားများကို `version` တွင် ချည်နှောင်ထားပြီး အပြောင်းအလဲတွင် ရွှေရောင်ကွက်လပ်များ လိုအပ်သည်။

ရှေးဟောင်းပစ္စည်းများ စစ်ဆေးခြင်း။
- ခေါင်းစီးအကွက်များကို တည်ငြိမ်သောမြင်ကွင်းအတွက် `ivm_tool inspect <file.to>` ကိုသုံးပါ။
- ဖွံ့ဖြိုးတိုးတက်မှုအတွက်၊ ဥပမာ/ တည်ဆောက်ထားသော ရှေးဟောင်းပစ္စည်းများကို စစ်ဆေးကြည့်ရှုသည့် အသေးစား Makefile ပစ်မှတ် `examples-inspect` ပါဝင်သည်။

ဥပမာ (သံချေး)- အနည်းဆုံးမှော် + အရွယ်အစား စစ်ဆေးပါ။

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

မှတ်ချက်- မှော်ထက်ကျော်လွန်သော ခေါင်းစီးအပြင်အဆင် အတိအကျကို မူကွဲနှင့် အကောင်အထည်ဖော်-သတ်မှတ်ထားသည်။ တည်ငြိမ်သော အကွက်အမည်များနှင့် တန်ဖိုးများအတွက် `ivm_tool inspect` ကို ဦးစားပေးပါ။