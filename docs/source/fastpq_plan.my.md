---
lang: my
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ Prover အလုပ်ပြိုကွဲခြင်း။

ဤစာရွက်စာတမ်းသည် ထုတ်လုပ်မှု-အဆင်သင့် FASTPQ-ISI သက်သေပြချက်တစ်ခုပေးပို့ခြင်းအတွက် အဆင့်လိုက်အစီအစဥ်ကို ဖမ်းယူထားပြီး ၎င်းကို ဒေတာအာကာသအစီအစဉ်ဆွဲသည့်ပိုက်လိုင်းသို့ ကြိုးပေးထားသည်။ အောက်ဖော်ပြပါ အဓိပ္ပါယ်ဖွင့်ဆိုချက်တိုင်းသည် TODO အဖြစ် အမှတ်အသားမပြုပါက စံနှုန်းဖြစ်သည်။ ခန့်မှန်းခြေ အသံထွက်သည် ကိုင်ရိုစတိုင် DEEP-FRI ဘောင်များကို အသုံးပြုသည်။ တိုင်းတာထားသောဘောင်သည် 128 bits အောက်တွင် ကျဆင်းသွားပါက CI ရှိ အလိုအလျောက် ငြင်းပယ်ခြင်း-နမူနာစစ်ဆေးမှုများ မအောင်မြင်ပါ။

## အဆင့် 0 — Hash Placeholder (ဆင်းသက်သည်)
- BLAKE2b ကတိကဝတ်ဖြင့် Norito ကုဒ်နံပါတ်ကို ဆုံးဖြတ်ခြင်း။
- `BackendUnavailable` ကို ပြန်လာသော Placeholder backend
- `fastpq_isi` မှပေးဆောင်ထားသော Canonical ကန့်သတ်ဘောင်ဇယား။

## အဆင့် 1 — ခြေရာခံတည်ဆောက်သူ နမူနာပုံစံ

> **အခြေအနေ (2025-11-09)-** `fastpq_prover` သည် ယခုအခါ ကျမ်းဂန်ထုပ်ပိုးမှုကို ဖော်ထုတ်ပြသသည်
> အကူအညီပေးသူများ (`pack_bytes`၊ `PackedBytes`) နှင့် အဆုံးအဖြတ်ပေးသော Poseidon2
> Goldilocks အပေါ် ကတိကဝတ်များ မှာယူခြင်း။ ကိန်းသေများကို ချိတ်တွဲထားသည်။
> `ark-poseidon2` `3f2b7fe` ကို ကတိပြုပြီး၊ ကြားဖြတ် BLAKE2 ကို လဲလှယ်ခြင်းနှင့်ပတ်သက်သည့် နောက်ဆက်တွဲကို ပိတ်ခြင်း
> နေရာယူမှုကို ပိတ်ထားသည်။ ရွှေရောင်စုံများ (`tests/fixtures/packing_roundtrip.json`၊
> `tests/fixtures/ordering_hash.json`) ယခု regression suite ကို ရပ်လိုက်ပါ။

### ရည်ရွယ်ချက်များ
- KV-update AIR အတွက် FASTPQ ခြေရာခံတည်ဆောက်သူကို အကောင်အထည်ဖော်ပါ။ အတန်းတစ်ခုစီသည် ကုဒ်လုပ်ရမည်-
  - `key_limbs[i]`: canonical key path ၏ base-256 ခြေလက်အင်္ဂါများ (7 bytes၊ little-endian)။
  - `value_old_limbs[i]`၊ `value_new_limbs[i]`- ကြိုတင်/ပို့စ်တန်ဖိုးများအတွက် တူညီသောထုပ်ပိုးမှု။
  - ရွေးချယ်ရေးကော်လံများ- `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, I180NI6700, `s_perm`။
  - အရန်ကော်လံများ- `delta = value_new - value_old`၊ `running_asset_delta`၊ `metadata_hash`၊ `supply_counter`။
  - ပိုင်ဆိုင်မှုကော်လံများ- 7-byte ခြေလက်အင်္ဂါများကို အသုံးပြု၍ `asset_id_limbs[i]`။
  - အဆင့် `ℓ` အလိုက် SMT ကော်လံများ- `path_bit_ℓ`၊ `sibling_ℓ`၊ `node_in_ℓ`၊ `node_out_ℓ`၊ အပေါင်း `neighbour_leaf`။
  - မက်တာဒေတာကော်လံများ- `dsid`, `slot`။
- **အဆုံးအဖြတ်ပေးသော အစီအစဉ်။** တည်ငြိမ်သောအမျိုးအစားကို အသုံးပြု၍ `(key_bytes, op_rank, original_index)` ဖြင့် အတန်းများကို အဘိဓာန်အလိုက် စီပါ။ `op_rank` မြေပုံဆွဲခြင်း- `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, Prometheus `original_index` သည် အမျိုးအစားမခွဲမီ 0 အခြေခံအညွှန်းဖြစ်သည်။ ရရှိလာသော Poseidon2 မှာယူမှု hash (ဒိုမိန်း tag `fastpq:v1:ordering`) ကို ဆက်ထားပါ။ အရှည်များသည် u64 အကွက်ဒြပ်စင်များဖြစ်သည့် `[domain_len, domain_limbs…, payload_len, payload_limbs…]` အဖြစ် hash preimage ကို ကုဒ်လုပ်ထားသောကြောင့် သုညဘိုက်များနောက်သို့ ဆက်သွားခြင်းကို ခွဲခြားနိုင်ပါသည်။
- ရှာဖွေမှုသက်သေ- သိမ်းဆည်းထားသောကော်လံ `s_perm` (ယုတ္တိနည်း OR `s_role_grant` နှင့် `s_role_revoke`) သည် 1. အခန်းကဏ္ဍ/ခွင့်ပြုချက် ID များသည် ပုံသေ-အကျယ် 32-byte ဖြစ်သည်၊ LE အပိုင်းသည် 8-byte LE ဖြစ်သည်။
- AIR မတိုင်မီနှင့် အတွင်းပိုင်းနှစ်မျိုးစလုံး၏ ပုံစံကွဲများကို တွန်းအားပေးပါ- ရွေးချယ်မှုများ အပြန်အလှန်သီးသန့်၊ ပိုင်ဆိုင်မှုတစ်ခုထိန်းသိမ်းမှု၊ dsid/slot ကိန်းသေများ။
- `N_trace = 2^k` (အတန်းအရေအတွက်၏ `pow2_ceiling`); `N_eval = N_trace * 2^b` နေရာတွင် `b` သည် မှုတ်ချဲ့ကိန်းဖြစ်သည်။
- ပစ္စည်းများနှင့် ပိုင်ဆိုင်မှုစစ်ဆေးမှုများ ဆောင်ရွက်ပေးပါ-
  - အသွားအပြန် ထုပ်ပိုးခြင်း (`fastpq_prover/tests/packing.rs`၊ `tests/fixtures/packing_roundtrip.json`)။
  - မှာယူမှုတည်ငြိမ်မှု hash (`tests/fixtures/ordering_hash.json`)။
  - အသုတ်တွဲများ (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`)။### AIR Column Schema
| ကော်လံအုပ်စု | အမည်များ | ဖော်ပြချက် |
| -----------------| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| လုပ်ဆောင်ချက် | `s_active` | အတန်းအမှန်အတွက် 1၊ 0 padding အတွက်။                                                                                       |
| ပင်မ | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | ထုပ်ပိုးထားသော Goldilocks ဒြပ်စင်များ (အသေးဆုံး၊ 7-byte ခြေလက်အင်္ဂါများ)။                                                             |
| ပိုင်ဆိုင်မှု | `asset_id_limbs[i]` | ထုပ်ပိုးထားသော canonical asset identifier (little-endian၊ 7-byte ခြေလက်အင်္ဂါ)။                                                      |
| ရွေးချယ်သူများ | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | ၀/၁။ ကန့်သတ်ချက်- ∑ ရွေးချယ်မှုစနစ်များ (`s_perm` အပါအဝင်) = `s_active`; `s_perm` သည် အတန်းများကို ပံ့ပိုးပေးသည်/ပြန်ရုတ်သိမ်းခြင်း အခန်းကဏ္ဍကို ထင်ဟပ်စေသည်။              |
| အရန် | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | ကန့်သတ်ချက်များ၊ ထိန်းသိမ်းရေးနှင့် စာရင်းစစ်လမ်းကြောင်းများအတွက် အသုံးပြုသည့်ပြည်နယ်။                                                           |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | အဆင့်အလိုက် Poseidon2 သွင်းအားစု/အထွက်များ နှင့် အဖွဲ့ဝင်မဟုတ်သည့်အတွက် အိမ်နီးချင်းသက်သေ။                                         |
| ရှာဖွေမှု | `perm_hash` | ခွင့်ပြုချက်ရှာဖွေမှုအတွက် Poseidon2 hash (`s_perm = 1` တွင်သာကန့်သတ်ထားသည်)။                                            |
| မက်တာဒေတာ | `dsid`, `slot` | အဆက်မပြတ်တန်းစီ။                                                                                                 |### သင်္ချာနှင့် ကန့်သတ်ချက်များ
- **Field packing-** bytes ကို 7-byte ခြေလက်အင်္ဂါများ (little-endian) အဖြစ် ပိုင်းဖြတ်ထားသည်။ ကိုယ်လက်အင်္ဂါတစ်ခုစီ `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; ခြေလက်အင်္ဂါ ≥ Goldilocks modulus ကို ငြင်းပယ်ပါ။
- **Balance/conservation:** `δ = value_new - value_old` ကို ခွင့်ပြုပါ။ `asset_id` ဖြင့် အတန်းများကို အုပ်စုဖွဲ့ပါ။ ပိုင်ဆိုင်မှုအုပ်စုတစ်ခုစီ၏ ပထမအတန်းတွင် `r_asset_start = 1` ကို သတ်မှတ်ပါ (0 မဟုတ်ရင်) နှင့် ကန့်သတ်ပါ
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  ပိုင်ဆိုင်မှုအုပ်စုတစ်ခုစီ၏ နောက်ဆုံးအတန်းတွင် အခိုင်အမာ
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  လွှဲပြောင်းမှုများသည် အုပ်စုတစ်လျှောက် ၎င်းတို့၏ δ တန်ဖိုးများကို သုညမှ သုညအထိ ပေါင်းထားသောကြောင့် ကန့်သတ်ချက်ကို အလိုအလျောက် ကျေနပ်စေသည်။ ဥပမာ- mint အတန်းတွင် `value_old = 100` နှင့် `value_new = 120` ဖြစ်ပါက၊ δ = 20၊ ထို့ကြောင့် mint sum သည် +20 နှင့် နောက်ဆုံးစစ်ဆေးမှုသည် မီးလောင်မှုမဖြစ်ပွားသည့်အခါ သုညသို့ ဖြေရှင်းသည်။
- ** Padding:** `s_active` ကို မိတ်ဆက်ပေးပါရစေ။ အတန်းကန့်သတ်ချက်အားလုံးကို `s_active` ဖြင့် မြှောက်ပြီး ဆက်တိုက်ရှေ့ဆက်- `s_active[i] ≥ s_active[i+1]` ကို ထည့်သွင်းပါ။ အကွက်တန်းများ (`s_active=0`) သည် ကိန်းသေတန်ဖိုးများကို ထိန်းသိမ်းထားရမည်ဖြစ်သော်လည်း အခြားနည်းဖြင့် အတားအဆီးမရှိပါ။
- **အော်ဒါမှာယူခြင်း hash-** Poseidon2 hash (domain `fastpq:v1:ordering`) အတန်းကုဒ်နံပါတ်များ၊ စာရင်းစစ်နိုင်စေရန် Public IO တွင် သိမ်းဆည်းထားသည်။

## အဆင့် 2 — STARK Prover Core

### ရည်ရွယ်ချက်များ
- Poseidon2 Merkle သည် ခြေရာခံခြင်းနှင့် အကဲဖြတ်ခြင်း vector များကို ရှာဖွေခြင်းအတွက် ကတိကဝတ်များကို တည်ဆောက်ပါ။ ကန့်သတ်ချက်များ- နှုန်း=2၊ စွမ်းရည်=1၊ အပြည့်အဝိုင်း=8၊ တစ်စိတ်တစ်ပိုင်း အဝိုင်း=57၊ ကိန်းသေများကို `ark-poseidon2` တွင် ချိတ်ထားသော `3f2b7fe` (v0.3.0)။
- Low-degree extension- `N_eval = 2^{k+b}` သည် Goldilocks ၏ 2-adic စွမ်းရည်ကို ပိုင်းခြားပေးသည့် domain `D = { g^i | i = 0 .. N_eval-1 }` တွင် ကော်လံတစ်ခုစီကို အကဲဖြတ်ပါ။ Goldilocks ၏ မူလရင်းမြစ်ဖြစ်သော `ω` နှင့် `g = ω^{(p-1)/N_eval}` အား `g = ω^{(p-1)/N_eval}` အား ထားလိုက်ပါ။ အခြေခံအုပ်စုခွဲကို အသုံးပြုပါ (coset မရှိပါ။) စာသားမှတ်တမ်း (tag `fastpq:v1:lde`) တွင် `g` ကို မှတ်တမ်းတင်ပါ။
- Composition polynomials- ကန့်သတ်ချက်တစ်ခုစီအတွက် `C_j`၊ အောက်တွင်ဖော်ပြထားသော ဒီဂရီအနားသတ်များဖြင့် `F_j(X) = C_j(X) / Z_N(X)` ပုံစံ။
- အထောက်အထားရှာဖွေခြင်း (ခွင့်ပြုချက်များ)- နမူနာ `γ` ခြေရာခံထုတ်ကုန် `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`။ ဇယားထုတ်ကုန် `T = ∏_j (table_perm_j - γ)`။ နယ်နိမိတ်ကန့်သတ်ချက်- `Z_final / T = 1`။
- arity `r ∈ {8, 16}` ပါသော DEEP-FRI- အလွှာတစ်ခုစီအတွက်၊ tag `fastpq:v1:fri_layer_ℓ`၊ နမူနာ `β_ℓ` (tag `fastpq:v1:beta_ℓ`) နှင့် `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` မှတဆင့် အမြစ်ကို စုပ်ယူပါ။
- အထောက်အထားပစ္စည်း (Norito-ကုဒ်လုပ်ထားသည်)
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- Verifier mirrors prover; ရွှေရောင်မှတ်တမ်းများပါရှိသော 1k/5k/20k အတန်းခြေရာများပေါ်တွင် ဆုတ်ယုတ်မှုအစုံကို လုပ်ဆောင်ပါ။

### စာရင်းကိုင်ဘွဲ့
| ကန့်သတ်ချက် | ဘွဲ့မရခင် တန်းခွဲ| ရွေးချယ်မှုများပြီးနောက် ဘွဲ့ဒီဂရီ Margin vs `deg(Z_N)` |
|--------------------|---------------------------------|---------------------------------|--------------------------------|
| Transfer/mint/burn ထိန်းသိမ်းရေး | ≤1 | ≤1 | `deg(Z_N) - 2` |
| အခန်းကဏ္ဍ ထောက်ပံ့ကြေး/ပြန်လည်ရုပ်သိမ်းခြင်း ရှာဖွေမှု | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Metadata သတ်မှတ် | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT hash (အဆင့်အလိုက်) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| ခမ်းနားသောထုတ်ကုန်ကို ရှာဖွေပါ | ထုတ်ကုန်ဆက်စပ် | N/A | နယ်နိမိတ်ကန့်သတ်ချက် |
| နယ်နိမိတ်အမြစ်များ / ထောက်ပံ့မှုစုစုပေါင်း | 0 | 0 | အတိအကျ |

အကွက်တန်းများကို `s_active` မှတဆင့် ကိုင်တွယ်သည်။ dummy အတန်းများသည် ကန့်သတ်ချက်များကို ချိုးဖောက်ခြင်းမရှိဘဲ သဲလွန်စကို `N_trace` သို့ တိုးချဲ့သည်။## ကုဒ်သွင်းခြင်းနှင့် စာသားမှတ်တမ်း (ကမ္ဘာလုံးဆိုင်ရာ)
- **Byte ထုပ်ပိုးခြင်း-** အခြေခံ-256 (7-byte ခြေလက်အင်္ဂါများ၊ အဆုံးအစမဲ့)။ `fastpq_prover/tests/packing.rs` တွင် စမ်းသပ်မှုများ။
- **Field encoding:** canonical Goldilocks (little-endian 64-bit ကိုယ်လက်အင်္ဂါ၊ ≥ p ကို ငြင်းပယ်); Poseidon2 အထွက်များ/SMT အမြစ်များကို 32-byte little-endian အခင်းအကျင်းများအဖြစ် အမှတ်အသားပြုထားသည်။
- **စာသားမှတ်တမ်း (Fiat–Shamir):**
  1. BLAKE2b သည် `protocol_version`, `params_version`, `parameter_set`, `public_io`, နှင့် Poseidon2 commit tag (`fastpq:v1:init`) ကို စုပ်ယူသည်။
  2. `trace_root`, `lookup_root` (`fastpq:v1:roots`) ကို စုပ်ယူပါ။
  3. ရှာဖွေမှုစိန်ခေါ်မှု `γ` (`fastpq:v1:gamma`) ရယူလိုက်ပါ။
  4. ဖွဲ့စည်းမှုဆိုင်ရာစိန်ခေါ်မှုများ `α_j` (`fastpq:v1:alpha_j`) ရယူပါ။
  5. FRI အလွှာတစ်ခုစီအတွက်၊ `fastpq:v1:fri_layer_ℓ` ဖြင့်စုပ်ယူပါ၊ `β_ℓ` (`fastpq:v1:beta_ℓ`) ကို ရယူပါ။
  6. မေးမြန်းမှုအညွှန်းကိန်းများ (`fastpq:v1:query_index`) ရယူပါ။

  တဂ်များသည် စာလုံးသေး ASCII; စိန်ခေါ်မှုများကို နမူနာမကောက်ယူမီ အတည်ပြုသူများသည် မကိုက်ညီမှုများကို ငြင်းဆိုသည်။ ရွှေရောင်မှတ်တမ်းဖိုင်- `tests/fixtures/transcript_v1.json`။
- **ဗားရှင်းဖွင့်ခြင်း-** `protocol_version = 1`၊ `params_version` သည် `fastpq_isi` သတ်မှတ်ဘောင်နှင့် ကိုက်ညီပါသည်။

## ရှာဖွေမှု အငြင်းပွားမှု (ခွင့်ပြုချက်များ)
- ကတိသစ္စာပြုဇယားကို `(role_id_bytes, permission_id_bytes, epoch_le)` ဖြင့်ခွဲထုတ်ပြီး Poseidon2 Merkle သစ်ပင် (`PublicIO` တွင် `perm_root`) မှတစ်ဆင့် ကတိပြုပါသည်။
- ခြေရာခံသက်သေ `perm_hash` နှင့် ရွေးချယ်ပေးသူ `s_perm` (OR အခန်းကဏ္ဍ ထောက်ပံ့ကြေး/ပြန်လည်ရုပ်သိမ်းခြင်း) ကို အသုံးပြုသည်။ tuple ကို ပုံသေ အကျယ်များ (32၊ 32၊ 8 bytes) ဖြင့် `role_id_bytes || permission_id_bytes || epoch_u64_le` အဖြစ် ကုဒ်လုပ်ထားသည်။
- ထုတ်ကုန်ဆက်စပ်မှု-
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  နယ်နိမိတ်သတ်မှတ်ချက်- `Z_final / T = 1`။ ကွန်ကရစ် accumulator လမ်းညွှန်ချက်အတွက် `examples/lookup_grand_product.md` ကို ကြည့်ပါ။

## ကျဲ Merkle သစ်ပင် ကန့်သတ်ချက်များ
- `SMT_HEIGHT` (အဆင့်အရေအတွက်) သတ်မှတ်ပါ။ ကော်လံများ `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` သည် `ℓ ∈ [0, SMT_HEIGHT)` အားလုံးအတွက် ပေါ်လာသည်။
- Poseidon2 ဘောင်များကို `ark-poseidon2` တွင် တွဲချိတ်ထားသော `3f2b7fe` (v0.3.0); ဒိုမိန်း tag `fastpq:v1:poseidon_node`။ node အားလုံးသည် little-endian အကွက်ကုဒ်ကို အသုံးပြုသည်။
- အဆင့်အလိုက် စည်းမျဉ်းများကို အပ်ဒိတ်လုပ်ပါ။
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- သတ်မှတ်ထည့်သွင်းမှုများ `(node_in_0 = 0, node_out_0 = value_new)`; `(node_in_0 = value_old, node_out_0 = 0)` ကို ဖျက်သည်။
- အသင်းဝင်မဟုတ်သော အထောက်အထားများသည် `neighbour_leaf` အား မေးမြန်းထားသည့်ကြားကာလသည် ဗလာဖြစ်ကြောင်းပြသသည်။ လုပ်ဆောင်ခဲ့သည့် ဥပမာနှင့် JSON အပြင်အဆင်အတွက် `examples/smt_update.md` ကို ကြည့်ပါ။
- နယ်နိမိတ်ကန့်သတ်ချက်- နောက်ဆုံး hash သည် အတန်းကြိုအတွက် `old_root` နှင့် ပို့စ်အတန်းများအတွက် `new_root` နှင့် ညီမျှသည်။

## အသံထွက်သတ်မှတ်ချက်များနှင့် SLO များ
| N_trace | မှုတ်ထုတ် | FRI arity | အလွှာများ | မေးမြန်းချက် | est bits | အထောက်အထားအရွယ်အစား (≤) | RAM (≤) | P95 latency (≤) |
| -------| ------| ---------| ------| -------| --------| --------------- | -------| ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1.5 GB | 0.40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2.5 GB | 0.75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3.5 GB | 1.20 s (A100) |

ဆင်းသက်လာမှုများသည် နောက်ဆက်တွဲ A. CI ကြိုးသိုင်းကြိုးများသည် ပုံမမှန်သော အထောက်အထားများကို ထုတ်လုပ်ပြီး ခန့်မှန်းခြေ bits <128 ဖြစ်လျှင် ပျက်ကွက်သည်။## အများသူငှာ IO Schema
| လယ် | ဘိုက် | ြဖစ် | မှတ်စုများ |
|--------------------|--------------------|------------------------------------------------|-------------------------------------------------|
| `dsid` | 16 | little-endian UUID | ဝင်ရောက်မှု၏လမ်းကြောင်းအတွက် Dataspace ID (မူရင်းလမ်းကြောအတွက် ကမ္ဘာလုံးဆိုင်ရာ) ကို tag `fastpq:v1:dsid` ဖြင့် hash လုပ်ထားသည်။ |
| `slot` | 8 | အဆုံးအမ u64 | ခေတ်ကတည်းက နာနိုစက္ကန့်။            |
| `old_root` | 32 | little-endian Poseidon2 အကွက် ဘိုက် | SMT root batch မတိုင်ခင်။              |
| `new_root` | 32 | little-endian Poseidon2 အကွက် ဘိုက် | သုတ်ပြီးနောက် SMT root ။               |
| `perm_root` | 32 | little-endian Poseidon2 အကွက် ဘိုက် | အထိုင်အတွက် ခွင့်ပြုချက်ဇယားအမြစ်။ |
| `tx_set_hash` | 32 | BLAKE2b | ညွှန်ကြားချက်များကို ခွဲခြားသတ်မှတ်ထားသည်။     |
| `parameter` | var | UTF-8 (e.g., `fastpq-lane-balanced`) | သတ်မှတ်ချက်အမည်။                 |
| `protocol_version`, `params_version` | 2 စီ | အဆုံးအမ u16 | ဗားရှင်းတန်ဖိုးများ။                      |
| `ordering_hash` | 32 | Poseidon2 (နည်းနည်း-အဆုံး) | စီထားသောအတန်းများ၏ တည်ငြိမ်သော hash။         |

ဖျက်ခြင်းအား သုညတန်ဖိုးခြေလက်အင်္ဂါများဖြင့် ကုဒ်လုပ်ထားသည်။ ပျက်ကွက်သော့များသည် သုညအရွက် + အိမ်နီးနားချင်းသက်သေကို အသုံးပြုသည်။

`FastpqTransitionBatch.public_inputs` သည် `dsid`၊ `slot` နှင့် root ကတိကဝတ်များအတွက် canonical carrier ဖြစ်သည်။
batch metadata ကို entry hash/transcript count bookkeeping အတွက် သီးသန့်ထားသည်။

## Hashes ကို ကုဒ်လုပ်ခြင်း။
- မှာယူမှု hash- Poseidon2 (tag `fastpq:v1:ordering`)။
- Batch artifact hash- `PublicIO || proof.commitments` (tag `fastpq:v1:artifact`) ကျော် BLAKE2b။

## ပြီးပြီ (DoD) ၏ အဆင့်သတ်မှတ်ချက်များ
- ** အဆင့် 1 DoD **
  - အသွားအပြန် စစ်ဆေးမှုများနှင့် ပစ္စည်းများ ပေါင်းစပ်ထုပ်ပိုးခြင်း။
  - AIR spec (`docs/source/fastpq_air.md`) တွင် `s_active`၊ ပိုင်ဆိုင်မှု/SMT ကော်လံများ၊ ရွေးချယ်သူ အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ (`s_perm` အပါအဝင်) နှင့် သင်္ကေတဆိုင်ရာ ကန့်သတ်ချက်များ ပါဝင်သည်။
  - PublicIO တွင်မှတ်တမ်းတင်ထားသော hash မှာယူခြင်းနှင့် fixtures များမှတဆင့်စစ်ဆေးပါ။
  - SMT/lookup မျက်မြင်သက်သေမျိုးဆက်ကို အဖွဲ့ဝင်ခြင်းနှင့် အဖွဲ့ဝင်မဟုတ်သော vector များဖြင့် အကောင်အထည်ဖော်ခြင်း။
  - ထိန်းသိမ်းရေးစစ်ဆေးမှုများသည် လွှဲပြောင်းခြင်း၊ mint၊ မီးရှို့ခြင်းနှင့် ရောနှောထားသော အသုတ်များကို အကျုံးဝင်သည်။
- ** အဆင့် 2 DoD **
  - Transcript spec ကိုအကောင်အထည်ဖော်ခဲ့သည် ရွှေရောင်မှတ်တမ်း (`tests/fixtures/transcript_v1.json`) နှင့် ဒိုမိန်းတဂ်များကို စစ်ဆေးပြီးပါပြီ။
  - Poseidon2 ပါရာမီတာသည် ဗိသုကာလက်ရာများတစ်လျှောက် အဆုံးစွန်သောစမ်းသပ်မှုများနှင့်အတူ `3f2b7fe` ကို သက်သေပြခြင်းနှင့် အတည်ပြုခြင်းတွင် တွဲထားသည်။
  - Soundness CI အစောင့်အကြပ်တက်ကြွစွာ; အထောက်အထားအရွယ်အစား/RAM/latency SLO များကို မှတ်တမ်းတင်ထားသည်။
- ** အဆင့် 3 DoD **
  - Scheduler API (`SubmitProofRequest`, `ProofResult`) idempotency သော့များဖြင့် မှတ်တမ်းတင်ထားသည်။
  - ပြန်စမ်း/ပြန်ပိတ်ခြင်းဖြင့် သိမ်းဆည်းထားသော အကြောင်းအရာ အထောက်အထားများ။
  - တယ်လီမီတာကို တန်းစီခြင်းအတိမ်အနက်၊ တန်းစီစောင့်ဆိုင်းချိန်၊ သက်သေပြပြီးသော တုံ့ပြန်မှုကြာချိန်၊ ထပ်စမ်းရေတွက်မှုများ၊ နောက်ကွယ်တွင် ရှုံးနိမ့်မှုအရေအတွက်နှင့် GPU/CPU အသုံးချမှု၊ မက်ထရစ်တစ်ခုစီအတွက် ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်အဆင့်များပါရှိသည်။## အဆင့် 5 — GPU Acceleration & Optimization
- ပစ်မှတ်စေ့များ- LDE (NTT)၊ Poseidon2 hashing၊ Merkle သစ်ပင်တည်ဆောက်မှု၊ FRI ခေါက်ခြင်း။
- Determinism- လျင်မြန်သောသင်္ချာကိုပိတ်ပါ၊ CPU၊ CUDA၊ Metal တစ်လျှောက်တွင် bit-identical output များကိုသေချာပါစေ။ CI သည် စက်များတွင် အထောက်အထားအမြစ်များကို နှိုင်းယှဉ်ရပါမည်။
- ရည်ညွှန်းဟာ့ဒ်ဝဲတွင် CPU နှင့် GPU နှိုင်းယှဉ်ထားသော Benchmark suite (ဥပမာ၊ Nvidia A100၊ AMD MI210)။
- သတ္တုနောက်ခံ (Apple Silicon)
  - Build script သည် kernel suite (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) ကို `fastpq.metallib` မှတဆင့် `xcrun metal`/`xcrun metallib` သို့ စုစည်းပေးပါသည်။ macOS developer ကိရိယာများတွင် Metal toolchain (`xcode-select --install`၊ ထို့နောက် လိုအပ်ပါက `xcodebuild -downloadComponent MetalToolchain`) ပါဝင်ကြောင်း သေချာပါစေ။【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - CI သွေးပူပေးခြင်း သို့မဟုတ် အဆုံးအဖြတ်ပေးသော ထုပ်ပိုးခြင်းအတွက် ကိုယ်တိုင်ပြန်လည်တည်ဆောက်ခြင်း (မှန်ချပ် `build.rs`)
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    အောင်မြင်သောတည်ဆောက်မှုများသည် `FASTPQ_METAL_LIB=<path>` ကိုထုတ်လွှတ်သောကြောင့် runtime သည် metallib ကို အဆုံးအဖြတ်ပေးနိုင်သည်။【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - ယခုအခါ LDE kernel သည် အကဲဖြတ်သည့်ကြားခံအား လက်ခံသူပေါ်တွင် သုည-အစပြုသည်ဟု ယူဆသည်။ လက်ရှိ `vec![0; ..]` ခွဲဝေမှုလမ်းကြောင်းကို သိမ်းဆည်းထားပါ သို့မဟုတ် ၎င်းတို့ကို ပြန်လည်အသုံးပြုသည့်အခါ သုညပြတ်သားစွာ ကြားခံထားပါ။【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - အပိုဖြတ်သန်းမှုကို ရှောင်ရှားရန် Coset မြှောက်ခြင်းကို နောက်ဆုံး FFT အဆင့်တွင် ပေါင်းစပ်ထားသည်။ LDE အဆင့်တွင် ပြောင်းလဲမှုတိုင်းသည် ထိုမူကွဲကို ထိန်းသိမ်းထားရပါမည်။【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - မျှဝေထားသော Memory FFT/LDE kernel သည် ယခုအခါ အကွက်အတိမ်အနက်တွင် ရပ်တန့်သွားပြီး ကျန်ရှိသောလိပ်ပြာများကို သီးသန့် `fastpq_fft_post_tiling` pass သို့ လွှဲပြောင်းပေးပါသည်။ Rust host သည် kernel နှစ်ခုလုံးမှတစ်ဆင့် တူညီသောကော်လံအတွဲများကို ဖြန့်ကျက်ပြီး `log_len` သည် အကွက်ကန့်သတ်ချက်ထက်ကျော်လွန်သည့်အခါမှသာ တူညီသောကော်လံအတွဲများကို ဖြန့်ပေးသည်၊ ထို့ကြောင့် တန်းစီ-အတိမ်အနက် တယ်လီမီတာ၊ kernel ကိန်းဂဏာန်းများနှင့် နောက်ခံအပြုအမူများသည် GPU သည် ကျယ်ပြန့်သည့်အဆင့်ကို လုံးလုံးလျားလျားလုပ်ဆောင်နေချိန်တွင် အဆုံးအဖြတ်ရှိနေပါသည်။ စက်ပေါ်ရှိ။ 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - ပစ်လွှတ်ပုံသဏ္ဍာန်များဖြင့် စမ်းသပ်ရန် `FASTPQ_METAL_THREADGROUP=<width>` ကို သတ်မှတ်ပါ။ ပေးပို့မှုလမ်းကြောင်းသည် စက်ပစ္စည်းကန့်သတ်ချက်သို့ တန်ဖိုးကို ကန့်သတ်ထားပြီး ပရိုဖိုင်လုပ်ဆောင်ခြင်းများသည် ပြန်လည်ပေါင်းစည်းခြင်းမပြုဘဲ threadgroup အရွယ်အစားများကို ချဲ့ထွင်နိုင်စေရန်အတွက် တန်ဘိုးကို ကိရိယာကန့်သတ်ချက်သို့ ကန့်သတ်ပြီး ထပ်လောင်းမှတ်တမ်းတင်ပါသည်။- FFT အကွက်ကို တိုက်ရိုက်ချိန်ညှိပါ- အိမ်ရှင်သည် ယခု threadgroup လမ်းသွားများ (16 လမ်းတိုအတွက်၊ 32 ကြိမ် `log_len ≥ 6`၊ 64 ကြိမ် `log_len ≥ 10`၊ 128 တစ်ကြိမ် `log_len ≥ 14`၊ နှင့် I102s အဆင့်တွင် 2560X) သေးငယ်သောခြေရာများ၊ 4 `log_len ≥ 12` နှင့် domain သည် `log_len ≥ 18/20/22` သို့ရောက်ရှိသည်နှင့်တစ်ပြိုင်နက် မျှဝေထားသော-memory pass သည် ယခုအခါ 12/14/16 အဆင့်များကို run သည်) တောင်းဆိုထားသောဒိုမိန်းမှ တောင်းဆိုထားသော domain နှင့် စက်၏လုပ်ဆောင်မှု width/max threads မှ 12/14/16 အဆင့်များကို လုပ်ဆောင်ပါသည်။ သတ်မှတ်ထားသော ပစ်လွှတ်မှုပုံစံများကို ပင်ထိုးရန် `FASTPQ_METAL_FFT_LANES` (8 နှင့် 256 အကြား နှစ်ခုပါဝါ) နှင့် `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) ဖြင့် အစားထိုးပါ။ တန်ဖိုးနှစ်ခုစလုံးသည် `FftArgs` မှတဆင့်စီးဆင်းသွားပြီး၊ ပံ့ပိုးထားသောဝင်းဒိုးသို့ ချိတ်ဆွဲကာ ပရိုဖိုင်ပြုလုပ်ရန်အတွက် မှတ်တမ်းဝင်ထားသည်။ တံမြက်လှည်းများ။
- FFT/IFFT နှင့် LDE ကော်လံအတွဲလိုက်သည် ယခုအခါ ဖြေရှင်းထားသော threadgroup width မှ ဆင်းသက်လာပါပြီ- host သည် command buffer တစ်ခုလျှင် 4096 logical threads အကြမ်းဖျင်းအားဖြင့် ပစ်မှတ်ထားကာ၊ တစ်ကြိမ်လျှင် ကော်လံ 64 ခုအထိ fuse လုပ်ပြီး circular-buffer tile staging နှင့် 64 → 32 → 16 → ကော်လံ → as2 ကိုဖြတ်ကာသာ ratchet လုပ်ပါသည်။ 2¹⁶/2¹⁸/2²⁰/2²² သတ်မှတ်ချက်များ။ ၎င်းသည် ပေးပို့မှုတစ်ခုလျှင် ကော်လံ ≥64 ကော်လံတွင် 20k-row ဖမ်းယူမှုကို ရှည်လျားသော cosets များကို အဆုံးအဖြတ်အတိုင်း ပြီးဆုံးကြောင်း သေချာစေသည်။ ≈2ms ပစ်မှတ်သို့ ပေးပို့သည့်တိုင်အောင် ကော်လံအကျယ်ကို နှစ်ဆတိုးနေသေးပြီး နမူနာပေးပို့မှုတစ်ခုသည် ထိုပစ်မှတ်ထက် ≥30% ကျလာသည့်အခါတိုင်း အသုတ်လိုက်အပြားကို အလိုအလျောက် တစ်ဝက်ခွဲပေးကာ၊ ထို့ကြောင့် ကော်လံတစ်ခုလျှင် ကုန်ကျစရိတ်ကို တိုးစေသော လမ်းကြော/အကွက်များ အပြောင်းအလဲများသည် manual overrides မပါဘဲ ပြန်ကျသွားပါသည်။ Poseidon ပြောင်းလဲမှုများသည် တူညီသော လိုက်လျောညီထွေရှိသော အချိန်ဇယားကို မျှဝေထားပြီး `fastpq_metal_bench` ရှိ `metal_heuristics.batch_columns.poseidon` ဘလောက်သည် ယခုအခါ ဖြေရှင်းပြီးသား အခြေအနေ အရေအတွက်၊ ဦးထုပ်၊ နောက်ဆုံးကြာချိန်နှင့် အလံကို ထပ်လှန်ခြင်းတို့ကို မှတ်တမ်းတင်ထားသောကြောင့် တန်းစီကျကျ တယ်လီမီတာကို Poseidon ချိန်ညှိခြင်းတွင် တိုက်ရိုက်ချိတ်နိုင်သည်။ သတ်မှတ်ထားသော FFT အသုတ်အရွယ်အစားကို ပင်ဆင်ရန် `FASTPQ_METAL_FFT_COLUMNS` (1-64) ဖြင့် အစားထိုးပြီး `FASTPQ_METAL_LDE_COLUMNS` (1-64) ကို LDE ပေးပို့သူအား ပုံသေကော်လံအရေအတွက်ကို ဂုဏ်ပြုရန် လိုအပ်သောအခါ၊ သတ္တုခုံတန်းလျားသည် ဖမ်းယူမှုတိုင်းတွင် ဖြေရှင်းပြီးသား `kernel_profiles.*.columns` များကို ဖုံးအုပ်ထားသောကြောင့် ချိန်ညှိခြင်းစမ်းသပ်မှုများ ဆက်လက်ရှိနေမည်ဖြစ်သည်။ ပြန်လည်ထုတ်လုပ်နိုင်သည်။【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】- Multi-queue dispatch သည် discrete Macs များတွင် ယခုအလိုအလျောက်ဖြစ်သည်- host သည် `is_low_power`၊ `is_headless` ကိုစစ်ဆေးပြီး Metal command နှစ်ခုလုံးကို တန်းစီခြင်းအား လှည့်ခြင်းရှိမရှိ ဆုံးဖြတ်ရန်၊ အလုပ်ဝန်သည် အနည်းဆုံး 16 ကော်လံ (ဖြေရှင်းထားသော fan-bin များအတိုင်း အတိုင်းအတာဖြင့် အတိုင်းအတာ) နှင့် GPU နှစ်ခုလုံးရှည်နေသည့်အခါမှသာ တန်းစီစောင့်ဆိုင်းပေးပါသည်။ အဆုံးအဖြတ်မပေးဘဲ လမ်းကြောများ ရှုပ်နေသည်။ command-buffer semaphore သည် ယခုအခါ "တစ်တန်းလျှင် ပျံသန်းမှုနှစ်ခု" ကြမ်းပြင်ကို ပြဌာန်းထားပြီး၊ တန်းစီတယ်လီမီတာမှ စုစည်းတိုင်းတာခြင်းဝင်းဒိုး (`window_ms`) နှင့် ပုံမှန်မအားလပ်သောအချိုးများ (`busy_ratio`) အတွက် ကမ္ဘာလုံးဆိုင်ရာ semaphore နှင့် တန်းစီတိုင်း ဝင်ရောက်မှု ≥ နှစ်ခုစလုံး အလုပ်များနေမည်ဖြစ်သောကြောင့် တန်းစီခြင်း 5% ကျော်နေနိုင်သည် ။ `FASTPQ_METAL_QUEUE_FANOUT` (1–4 လမ်းသွား) နှင့် `FASTPQ_METAL_COLUMN_THRESHOLD` (ပန်ကာမဖွင့်မီ အနည်းဆုံး စုစုပေါင်းကော်လံများ) ဖြင့် မူရင်းများကို အစားထိုးပါ။ Metal parity tests များသည် overrides များကို တွန်းအားပေးသောကြောင့် multi-GPU Macs များကို ဖုံးကွယ်ထားပြီး ဖြေရှင်းထားသောမူဝါဒကို တန်းစီ-အတိမ်အနက် တယ်လီမီတာနှင့် `metal_dispatch_queue.queues[*]` အသစ်တို့နှင့်အတူ မှတ်တမ်းတင်ထားသည်။ တုံး။
- ယခု သတ္တုရှာဖွေခြင်းသည် `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` ကို တိုက်ရိုက် (ခေါင်းမဲ့ခွံများတွင် CoreGraphics ကို ပူနွေးအောင်ပြုလုပ်သည်) ကို `system_profiler` သို့ ပြန်ကျသွားစေရန်နှင့် `FASTPQ_DEBUG_METAL_ENUM` သည် ကိန်းဂဏာန်းစက်များကို ပရင့်ထုတ်သည့်အခါ အဘယ်ကြောင့်နည်း200000280X သည် ဦးခေါင်းမပါသော I180 ၏ အကြောင်းရင်းကို ရှင်းပြနိုင်သည် CPU လမ်းကြောင်းသို့ အဆင့်နှိမ့်ထားသည်။ override ကို `gpu` သို့သတ်မှတ်ထားသော်လည်း accelerator မတွေ့ပါက၊ ယခု `fastpq_metal_bench` သည် CPU တွင်တိတ်တဆိတ်ဆက်လက်လုပ်ဆောင်မည့်အစား အမှားရှာခလုတ်ဆီသို့ pointer ဖြင့်ချက်ချင်းအမှားဖြစ်သွားသည်။ ၎င်းသည် WP2‑E တွင် ခေါ်ဝေါ်သော “အသံတိတ် CPU ဆုတ်ယုတ်မှု” အတန်းကို ကျဉ်းမြောင်းစေပြီး ထုပ်ပိုးထားသော အတွင်း၌ စာရင်းကောက်ယူမှုမှတ်တမ်းများကို ဖမ်းယူရန် အော်ပရေတာများအား ခလုတ်တစ်ခုပေးသည်။ စံသတ်မှတ်ချက်များ။ 【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU အချိန်ဇယားများသည် CPU ကျဆင်းမှုများကို "GPU" ဒေတာအဖြစ် ကုသရန် ငြင်းဆိုထားသည်။ `hash_columns_gpu` သည် အရှိန်မြှင့်စက် အမှန်တကယ် လည်ပတ်ခြင်း ရှိမရှိ အစီရင်ခံသည်၊ `measure_poseidon_gpu` သည် ပိုက်လိုင်း ပြန်ကျသွားသည့်အခါတိုင်း နမူနာများ ကျဆင်းသွားသည် (သတိပေးချက်မှတ်တမ်း) နှင့် Poseidon microbench ကလေးသည် အမှားအယွင်းတစ်ခုဖြင့် ထွက်သွားပါသည်။ ရလဒ်အနေဖြင့် `gpu_recorded=false` သတ္တုလုပ်ဆောင်မှု ပြန်ကျသွားသည့်အခါတိုင်း၊ တန်းစီအနှစ်ချုပ်သည် မအောင်မြင်သော dispatch window ကို မှတ်တမ်းတင်ထားဆဲဖြစ်ပြီး ဒက်ရှ်ဘုတ်အနှစ်ချုပ်များသည် ဆုတ်ယုတ်မှုကို ချက်ချင်းဖော်ပြသည်။ ထုပ်ပိုးခြင်း (`scripts/fastpq/wrap_benchmark.py`) သည် ယခု `metal_dispatch_queue.poseidon.dispatch_count == 0` တွင် ပျက်သွားသောကြောင့် Stage7 အစုအဝေးများကို အမှန်တကယ် GPU Poseidon ပေးပို့ခြင်းမရှိဘဲ လက်မှတ်မထိုးနိုင်ပါ။ အထောက်အထား။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- Poseidon hashing သည် ယခုစာချုပ်ကို ရောင်ပြန်ဟပ်နေပါသည်။ `PoseidonColumnBatch` သည် ပြန့်ကျဲနေသော payload buffers များအပြင် offset/length descriptors များကို ထုတ်လုပ်ပေးသည်၊ host သည် batch တစ်ခုလျှင် အဆိုပါ descriptors များကို rebase လုပ်ပြီး `COLUMN_STAGING_PIPE_DEPTH` double buffer ကို run သောကြောင့် payload + descriptor သည် GPU အလုပ်နှင့် ထပ်နေစေပြီး သတ္တု/CUDA script kernels နှစ်ခုလုံးသည် disabled rate တစ်ခုစီကို တိုက်ရိုက်စုပ်ယူပေးပါသည်။ ကော်လံအချေအတင်များကို ထုတ်လွှတ်ခြင်းမပြုမီ စက်ပေါ်တွင် `hash_columns_from_coefficients` သည် ယခု အစုလိုက်အပြုံလိုက်များကို GPU အလုပ်သမားချည်မျှင်မှတဆင့် ထုတ်လွှင့်နေပြီး 64+ ကော်လံများကို သီးခြား GPU များပေါ်တွင် မူရင်းအတိုင်း ပျံသန်းခြင်း (`FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH`) မှတဆင့် ထုတ်လွှင့်ပါသည်။ Metal bench တွင် ဖြေရှင်းပြီးသား ပိုက်လိုင်းဆက်တင်များ + `metal_dispatch_queue.poseidon_pipeline` အောက်တွင် အတွဲအရေအတွက်များကို မှတ်တမ်းတင်ထားပြီး `kernel_profiles.poseidon.bytes` တွင် ဖော်ပြချက်အသွားအလာပါဝင်သောကြောင့် Stage7 ဖမ်းယူမှုများသည် ABI အသစ်ကို သက်သေပြသည် အဆုံးအထိ။ 【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_cuda.cu:351】
- Stage7-P2 သည် Poseidon hashing ကို ပေါင်းစပ်ထားသော GPU နောက်ခံနှစ်ခုလုံးတွင် ယခုရောက်ရှိနေပြီဖြစ်သည်။ တိုက်ရိုက်လွှင့်လုပ်သားသည် ဆက်စပ်နေသော `PoseidonColumnBatch::column_window()` အချပ်များကို `hash_columns_gpu_fused` သို့ ပေးပို့ပေးကာ ၎င်းတို့အား `poseidon_hash_columns_fused` သို့ ပေးပို့ပေးသောကြောင့် ပေးပို့မှုတစ်ခုစီသည် `leaf_digests || parent_digests` ကို canonical `(⌈columns / 2⌉)` ဖြင့် ရေးသားပါသည်။ `ColumnDigests` သည် အချပ်နှစ်ခုစလုံးကို ထိန်းသိမ်းထားပြီး `merkle_root_with_first_level` သည် ပင်မအလွှာကို ချက်ခြင်းစားသုံးသည်၊ ထို့ကြောင့် CPU သည် depth-1 nodes များကို ဘယ်သောအခါမှ ပြန်လည်တွက်ချက်မည်မဟုတ်ကြောင်းနှင့် Stage7 telemetry သည် GPU သည် ပေါင်းစပ်ထားသော kernel ကို ဖမ်းယူသည့်အခါတိုင်း “fallback” မိဘများကို လုံးဝအစီရင်ခံမည်မဟုတ်ကြောင်း အခိုင်အမာပြောဆိုနိုင်သည်။ အောင်မြင်ပါတယ်။ 【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq3】cuda1
- ယခု `fastpq_metal_bench` သည် `device_profile` ဘလောက်တစ်ခုကို သတ္တုစက်ပစ္စည်းအမည်၊ မှတ်ပုံတင်အမှတ်၊ `low_power`/`headless` အလံများ၊ တည်နေရာ ( built-in၊ အပေါက်၊ ပြင်ပ)၊ discrete ညွှန်ပြထားသော 000X နှင့် Apple မှ Socrete ညွှန်ပြချက် 8 နှင့် I18NI တံဆိပ် (ဥပမာ၊ "M3 Max")။ Stage7 ဒက်ရှ်ဘုတ်များသည် M4/M3 နှင့် discrete GPU များကို အသုံးမပြုဘဲ ခွဲခြမ်းစိပ်ဖြာခြင်းမပြုဘဲ အစုလိုက်အပုံးဖမ်းရန်အတွက် ဤအကွက်ကို စားသုံးပြီး JSON သည် တန်းစီခြင်း/ heuristic အထောက်အထားများဘေးရှိ JSON သင်္ဘောများကို ပြေးလွှားစေသည့် သင်္ဘောအမျိုးအစားဖြစ်ကြောင်း သက်သေပြပါသည်။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench6】- FFT host/device overlap သည် ယခု double-buffered staging window ကိုအသုံးပြုသည်- ယခု batch *n* သည် `fastpq_fft_post_tiling` တွင် ပြီးသွားချိန်တွင်၊ host သည် batch *n+1* ကို ဒုတိယအဆင့်ကြားခံအဖြစ်သို့ ပြားပြီး buffer ကို ပြန်လည်အသုံးပြုရသည့်အခါမှသာ ခေတ္တရပ်ပါသည်။ နောက်ကွယ်မှသည် အသုတ်အရေအတွက်မည်မျှ ပြားသွားသည်နှင့် GPU ပြီးစီးမှုကို စောင့်ဆိုင်းနေချိန်နှင့် ပြားသွားသည့်အချိန်တို့ကို မှတ်တမ်းတင်ပြီး `fastpq_metal_bench` သည် ပေါင်းစည်းထားသော `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` ဘလောက်ကို ဖုံးအုပ်ထားသောကြောင့် ထုတ်ဝေလိုက်သော ပစ္စည်းများသည် အသံတိတ်အိမ်ရှင်ဆိုင်များအစား ထပ်နေကြောင်း သက်သေပြနိုင်သည်။ ယခု JSON အစီရင်ခံစာသည် `column_staging.phases.{fft,lde,poseidon}` အောက်တွင် အဆင့်အလိုက် စုစုပေါင်းပမာဏကို ပိုင်းဖြတ်ပြီး Stage7 ဖမ်းယူမှုများကို FFT/LDE/Poseidon အဆင့်တွင် လက်ခံဆောင်ရွက်ပေးခြင်း သို့မဟုတ် GPU ပြီးဆုံးခြင်းတွင် စောင့်ဆိုင်းခြင်းရှိမရှိ သက်သေပြနိုင်စေမည်ဖြစ်သည်။ Poseidon ပြောင်းလဲမှုများသည် တူညီသော ပေါင်းစပ်ထည့်သွင်းထားသော ဇာတ်ဝင်ခန်းကြားခံများကို ပြန်လည်အသုံးပြုသောကြောင့် ယခု `--operation poseidon_hash_columns` ဖမ်းယူမှုများသည် ယခုအခါတွင် Poseidon-specific `column_staging` မြစ်ဝကျွန်းပေါ်ဒေသများကို တန်းစီကျကျ အထောက်အထားများနှင့်အတူ တန်းစီကျကျ အထောက်အထားများနှင့်အတူ ထုတ်လွှတ်ပါသည်။ `column_staging.samples.{fft,lde,poseidon}` အသစ်သည် တစ်သုတ် `batch/flatten_ms/wait_ms/wait_ratio` tuples များကို မှတ်တမ်းတင်ထားပြီး `COLUMN_STAGING_PIPE_DEPTH` ထပ်နေနေကြောင်း သက်သေပြရန် အသေးအဖွဲလေးဖြစ်စေသည် (သို့မဟုတ် လက်ခံသူသည် GPU ကို စောင့်နေသည့်အချိန်ကို သိရန် ပြီးစီးမှုများ။ tpq_prover/src/metal.rs:2488 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- Poseidon2 အရှိန်မြှင့်ခြင်းကို ယခုအခါ မြင့်မားသော သတ္တု kernel တစ်ခုအနေဖြင့် လုပ်ဆောင်နေသည်- threadgroup တစ်ခုစီသည် round constants နှင့် MDS အတန်းများကို threadgroup memory ထဲသို့ ကူးယူကာ အပြည့်အဝ/တစ်စိတ်တစ်ပိုင်း rounds များကို ဖယ်ထုတ်ပြီး လမ်းကြောတစ်ခုစီတွင် states အများအပြားကို လမ်းလျှောက်သွားသောကြောင့် dispatch တစ်ခုစီတိုင်းသည် အနည်းဆုံး 4096 logical threads များကို ဖွင့်ပေးပါသည်။ `FASTPQ_METAL_POSEIDON_LANES` (32 နှင့် 256 ကြား နှစ်ခု၏ ပါဝါများ ၊ ကိရိယာ ကန့်သတ်ချက်သို့ ချိတ်ထားသည်) နှင့် `FASTPQ_METAL_POSEIDON_BATCH` (လမ်းကြောင်းတစ်ခုလျှင် ပြည်နယ် 1-32) ဖြင့် ပရိုဖိုင်းစမ်းသပ်မှုများကို ပြန်လည်ထုတ်လုပ်ရန် `fastpq.metallib` (1-32 ပြည်နယ်) မှတဆင့် ပရိုဖိုင်းစမ်းသပ်မှုများကို ပြန်လည်ထုတ်လုပ်ရန်၊ Rust host သည် မပို့မီ `PoseidonArgs` မှတဆင့် ဖြေရှင်းထားသော ချိန်ညှိမှုကို ချည်နှောင်ထားသည်။ ယခု host သည် boot တစ်ခုလျှင် `MTLDevice::{is_low_power,is_headless,location}` ကို လျှပ်တစ်ပြက်ရိုက်ပြီး 32GiB၊ Prometheus သို့ အခြားနည်းဖြင့် ပါဝါနိမ့်နေချိန်တွင် VRAM-tiered လွှတ်တင်မှုများဆီသို့ discrete GPUs များကို အလိုအလျောက် ဘက်လိုက်သည် (`256×24`၊ ≥48GiB အစိတ်အပိုင်းများပေါ်တွင် `256×20`) `256×8` (128/64 လမ်းသွား ဟာ့ဒ်ဝဲအတွက် အမှားအယွင်းများကို လမ်းကြောတစ်ခုလျှင် 8/6 ပြည်နယ်များ ဆက်လက်အသုံးပြုနေသည်) ထို့ကြောင့် အော်ပရေတာများသည် env vars များကိုမထိဘဲ 16-state pipeline depth ကိုရရှိသည်။ `fastpq_metal_bench` သည် `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` အောက်တွင် သူ့ကိုယ်သူ ပြန်လည်လုပ်ဆောင်ပြီး `poseidon_microbench` ဘလောက်ကို ဘက်စုံပြည်နယ် kernel နှင့် နှိုင်းယှဉ်ကာ စီစကလာလမ်းကြားကို ဖမ်းယူထားသောကြောင့် ထုတ်လွှတ်သည့်အရာများသည် ကွန်ကရစ်အရှိန်မြှင့်ခြင်းကို ကိုးကားနိုင်သည်။ တူညီသောမျက်နှာပြင်သည် `poseidon_pipeline` တယ်လီမီတာ (`chunk_columns`၊ `pipe_depth`၊ `batches`၊ `fallbacks`) ထို့ကြောင့် Stage7 အထောက်အထားသည် GPU တစ်ခုစီရှိ ထပ်နေသည့်ဝင်းဒိုးကို သက်သေပြသည် ခြေရာခံ။ 【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【crates tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - LDE tile ဇာတ်ကြောင်းသည် ယခု FFT heuristics ကို ထင်ဟပ်နေပါသည်- လေးလံသောခြေရာများသည် `log₂(len) ≥ 18` တစ်ကြိမ်တွင် အဆင့် 12 ဆင့်ကိုသာ လုပ်ဆောင်ပြီး log₂20 တွင် အဆင့် 10 သို့ ကျဆင်းသွားကာ log₂22 တွင် အဆင့်ရှစ်ဆင့်သို့ ချည်နှောင်ထားသောကြောင့် ကျယ်ပြန့်သောလိပ်ပြာများသည် ကြွေပြားကပ်ခြင်းသို့ ရွေ့သွားကြသည်။ သင်သည် အဆုံးအဖြတ်အတိမ်အနက်ကို လိုအပ်သည့်အခါတိုင်း `FASTPQ_METAL_LDE_TILE_STAGES` (1-32) ဖြင့် အစားထိုးပါ။ စောစောပိုင်းရပ်သွားသည့်အခါတွင် အိမ်ရှင်သည် ကြွေပြားခင်းထားသော dispatch ကို စတင်လုပ်ဆောင်သည် ထို့ကြောင့် တန်းစီခြင်း၏အတိမ်အနက်နှင့် kernel တယ်လီမီတာသည် အဆုံးအဖြတ်အတိုင်းရှိနေပါသည်။【crates/fastpq_prover/src/metal.rs:827】
  - Kernel micro-optimisation- မျှဝေထားသော Memory FFT/LDE tiles များသည် ယခုအခါ လိပ်ပြာတိုင်းအတွက် `pow_mod*` ကို ပြန်လည်အကဲဖြတ်မည့်အစား တစ်လမ်းသွား အလှည့်ကျနှင့် coset side များကို ပြန်လည်အသုံးပြုပါသည်။ လမ်းကြောတစ်ခုစီသည် `w_seed`၊ `w_stride` ကို ကြိုတင်တွက်ချက်ပြီး (လိုအပ်သောအခါ) တစ်တုံးလျှင် တစ်ကြိမ်လျှင် coset side ကို ဖြတ်ကာ အော့ဖ်ဆက်များမှတစ်ဆင့် စီးဆင်းကာ `apply_stage_tile`/Prometheus နှင့် အတန်းကို အဓိပ္ပာယ်ရှိသော L18NI00000 သို့ ကျဆင်းသွားစေသည်။ နောက်ဆုံးပေါ် heuristics ဖြင့် ~1.55s (950ms ပန်းတိုင်ထက်သာနေသေးသော်လည်း batching-only tweak ထက် နောက်ထပ် ~50ms တိုးတက်မှု) 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_bench_run11.json:1】- ယခု kernel suite တွင် ဝင်ပေါက်အမှတ်တစ်ခုစီ၊ `fastpq.metallib` တွင် ပြဌာန်းထားသော threadgroup/tile ကန့်သတ်ချက်များနှင့် metallib ကို ကိုယ်တိုင်ပြုစုခြင်းအတွက် မျိုးပွားခြင်းအဆင့်များကို မှတ်တမ်းတင်ထားသည့် သီးခြားရည်ညွှန်းချက် (`docs/source/fastpq_metal_kernels.md`) ရှိသည်။ 【docs/source/fastpq_metal_kernels】။md
  - ယခုအခါ စံညွှန်းအစီရင်ခံစာသည် သီးခြားစီတင်ထားသော kernel တွင် FFT/IFFT/LDE အစီအစဥ်မည်မျှလည်ပတ်ခဲ့သည်ကို မှတ်တမ်းတင်သည့် `post_tile_dispatches` အရာဝတ္ထုကို ထုတ်လွှတ်လိုက်ပါသည်။ `scripts/fastpq/wrap_benchmark.py` သည် ဘလောက်ကို `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary` သို့ မိတ္တူကူးပြီး သက်သေအထောက်အထားကို ချန်လှပ်ထားသည့် GPU ဖမ်းယူမှုကို ထင်ရှားစွာ ငြင်းဆိုထားသောကြောင့် အတန်း 20k-profact တိုင်းသည် multi-pass kernel လည်ပတ်မှုကို သက်သေပြသည် စက်ပေါ်တွင်။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - ကိရိယာ/သတ္တုခြေရာကောက်ဆက်စပ်မှုအတွက် တူရိယာ/သတ္တုခြေရာခံဆက်စပ်မှုအတွက် `FASTPQ_METAL_TRACE=1` (ပိုက်လိုင်းတံဆိပ်၊ ချည်ကြိုးအကျယ်၊ လွှင့်တင်သည့်အဖွဲ့များ၊ လွန်သွားသောအချိန်) ကို ထုတ်လွှတ်ရန် `FASTPQ_METAL_TRACE=1` ကို သတ်မှတ်ပါ။ 【crates/fastpq_prover/src/metal.rs:346】
- dispatch တန်းစီကို ယခု တပ်ဆင်ထားသည်- `FASTPQ_METAL_MAX_IN_FLIGHT` သည် တစ်ပြိုင်တည်း Metal command buffers များကို ထုပ်ပေးသည် ( macOS စက်ပစ္စည်းကို အစီရင်ခံရန် ငြင်းဆိုသောအခါတွင် အနည်းဆုံး တန်းစီနေသော ပန်ကာအထွက်ကြမ်းပြင်တွင် ချိတ်ထားသည်)။ ခုံတန်းလျားသည် တန်းစီခြင်း-နမူနာကို ဖွင့်ထားသောကြောင့် တင်ပို့သော JSON သည် `metal_dispatch_queue`၊ `limit`၊ `dispatch_count`၊ `max_in_flight`၊ `busy_ms`၊ `busy_ms`၊ နှင့် I10018 အထောက်အထားများအတွက် အကွက်များပေါင်းထည့်ရန် `busy_ms`500018 Poseidon-only capture (`--operation poseidon_hash_columns`) လည်ပတ်သည့်အခါတိုင်း `metal_dispatch_queue.poseidon` ဘလောက်ကို nested လုပ်ထားပြီး ဖြေရှင်းပြီးသား command-buffer limit ကိုဖော်ပြသည့် FFT/LDE batch columns (တန်ဖိုးများကို ဖယ်ရှားနိုင်သည်ရှိမရှိ အပါအဝင်) (ထို့ကြောင့် ditschedu များ အပါအဝင်) ကို ဖယ်ရှားနိုင်သည်ဖြစ်စေ (ထို့ကြောင့် ditschedu တန်ဖိုးကို သုံးသပ်သူများ အပါအဝင်) တယ်လီမီတာနှင့် တွဲ၍ ဆုံးဖြတ်ချက်များ Poseidon kernels များသည် kernel နမူနာများမှ ပေါင်းခံထားသော သီးခြား `poseidon_profiles` ဘလောက်တစ်ခုကိုလည်း ကျွေးမွေးသောကြောင့် bytes/thread၊ occupancy, and dispatch geometry တို့ကို artefacts တစ်လျှောက် ခြေရာခံပါသည်။ မူလလုပ်ငန်းလည်ပတ်မှုတွင် တန်းစီအတိမ်အနက်ကို မစုဆောင်းနိုင်ပါက သို့မဟုတ် LDE သုည-ဖြည့်စွက်ကိန်းဂဏန်းများ (ဥပမာ၊ GPU ပေးပို့မှုတစ်ခုသည် CPU သို့ တိတ်တဆိတ်ပြန်ကျသွားသည့်အခါ) ကြိုးသည် ပျောက်ဆုံးနေသော တယ်လီမီတာကို စုဆောင်းရန်အတွက် တစ်ခုတည်းသော probe dispatch ကို အလိုအလျောက်ထုတ်ပေးပြီး GPU မှ ၎င်းတို့အား အစီရင်ခံရန် ငြင်းဆိုသောအခါတွင် လက်ခံရရှိသည့် သုည-ဖြည့်ရမည့်အချိန်များကို ပေါင်းစပ်လုပ်ဆောင်ပေးသည်၊ ထို့ကြောင့် ထုတ်ပြန်ထားသော အထောက်အထားများတွင် အမြဲတမ်း 010X183NI ပါဝင်သည်။ ပိတ်ဆို့ခြင်း။ _prover/src/bin/fastpq_metal_bench.rs:1524 】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Metal toolchain မပါဘဲ cross-compiling လုပ်သောအခါ `FASTPQ_SKIP_GPU_BUILD=1` ကို သတ်မှတ်ပါ။ သတိပေးချက်သည် ကျော်သွားခြင်းကို မှတ်တမ်းတင်ပြီး စီစဉ်သူသည် CPU လမ်းကြောင်းပေါ်တွင် ဆက်လက်ရှိနေပါသည်။【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- Runtime detection သည် Metal support ကိုအတည်ပြုရန် `system_profiler` ကိုအသုံးပြုသည်။ မူဘောင်၊ စက်ပစ္စည်း သို့မဟုတ် metallib ပျောက်ဆုံးနေပါက တည်ဆောက်မှု script သည် `FASTPQ_METAL_LIB` ကိုရှင်းလင်းစေပြီး အစီအစဉ်ရေးဆွဲသူသည် အဆုံးအဖြတ်ပေးသော CPU တွင်ရှိနေသည် လမ်းကြောင်း။【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - အော်ပရေတာစစ်ဆေးချက်စာရင်း (သတ္တုအိမ်ရှင်များ):
    1. စုစည်းထားသော `.metallib` (`.metallib` တွင် `FASTPQ_METAL_LIB` အမှတ်များ ရှိနေကြောင်း အတည်ပြုပါ (`echo $FASTPQ_METAL_LIB` သည် `cargo build --features fastpq-gpu` ပြီးနောက် အလွတ်မဖြစ်သင့်ပါ))။【crates/fastpq_prover/88】rs:
    2. GPU လမ်းကြောများကို ဖွင့်ထားခြင်းဖြင့် တူညီသောစစ်ဆေးမှုများကို လုပ်ဆောင်ပါ- `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`။ ၎င်းသည် သတ္တုစေ့များကို လေ့ကျင့်ခန်းလုပ်ကာ ထောက်လှမ်းမှု မအောင်မြင်ပါက အလိုအလျောက် ပြန်ကျသွားပါသည်။
    3. ဒက်ရှ်ဘုတ်များအတွက် စံနမူနာယူပါ- စုစည်းထားသော သတ္တုစာကြည့်တိုက်ကို ရှာပါ။
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`) မှတဆင့် တင်ပို့ပါ။
       `FASTPQ_METAL_LIB` နှင့် run ပါ။
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`။
       Canonical `fastpq-lane-balanced` သည် ယခု ဖမ်းယူမှုတိုင်းကို 32,768 အတန်းအထိ မြှင့်တင်ပေးသည်၊ ထို့ကြောင့်၊
       JSON သည် တောင်းဆိုထားသော 20k အတန်းများနှင့် GPU ကိုမောင်းနှင်သည့် padded domain နှစ်ခုလုံးကို ရောင်ပြန်ဟပ်သည်။
       စေ့များ။ JSON/log ကို သင့်အထောက်အထားစတိုးတွင် အပ်လုဒ်လုပ်ပါ။ ညစဉ် macOS အလုပ်အသွားအလာမှန်များ
      ဤလုပ်ဆောင်ချက်သည် အကိုးအကားအတွက် မော်ကွန်းတင်ခြင်း ဖြစ်သည်။ အစီရင်ခံစာ မှတ်တမ်းတင်သည်။
     လည်ပတ်မှုတစ်ခုစီ၏ `speedup` နှင့်အတူ `fft_tuning.{threadgroup_lanes,tile_stage_limit}`၊
     LDE အပိုင်းသည် `zero_fill.{bytes,ms,queue_delta}` ကို ပေါင်းထည့်ထားသောကြောင့် ထွက်လာသည့် ပစ္စည်းများသည် အဆုံးအဖြတ်ဖြစ်ကြောင်း သက်သေပြသည်၊
     host zero-fill overhead နှင့် တိုးမြင့်သော GPU တန်းစီအသုံးပြုမှု (ကန့်သတ်ချက်၊ ပေးပို့မှုအရေအတွက်၊
     ပျံသန်းမှုအထွတ်အထိပ်၊ အလုပ်များ/ထပ်နေချိန်) နှင့် `kernel_profiles` ဘလောက်အသစ်သည် kernel တစ်ခုစီကို ဖမ်းယူသည်
     နေထိုင်မှုအချိုးများ၊ ခန့်မှန်းခြေ bandwidth နှင့် ကြာချိန်အပိုင်းအခြားများ ဖြစ်သောကြောင့် ဒက်ရှ်ဘုတ်များသည် GPU ကိုအလံပြနိုင်သည်။
       ကုန်ကြမ်းနမူနာများကို ပြန်လည်မပြင်ဆင်ဘဲ ဆုတ်ယုတ်မှုများ။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       သတ္တု LDE လမ်းကြောင်းသည် 950ms အောက်တွင် ရှိနေရန် မျှော်လင့်ပါ (`<1 s` သည် Apple M-series ဟာ့ဒ်ဝဲပေါ်တွင် ပစ်မှတ်);
4. စစ်မှန်သော ExecWitness မှ အတန်းအသုံးပြုမှု တယ်လီမီတာကို ဖမ်းယူပါ၊ သို့မှသာ ဒက်ရှ်ဘုတ်များသည် လွှဲပြောင်း gadget ကိုဇယားဆွဲနိုင်သည်
   မွေးစားခြင်း။ Torii မှ သက်သေကို ရယူပါ။
  (`iroha_cli audit witness --binary --out exec.witness`) ဖြင့် ၎င်းကို ကုဒ်လုပ်ပါ။
  `iroha_cli audit witness --decode exec.witness` (ရွေးချယ်နိုင်သည်။
  မျှော်လင့်ထားသော ကန့်သတ်ဘောင်ကို အတည်ပြုရန် `--fastpq-parameter fastpq-lane-balanced` FastPQ အတွဲများ
  ပုံမှန်အားဖြင့် ထုတ်လွှတ်ခြင်း၊ အထွက်ကို ချုံ့ရန် လိုအပ်မှသာ `--no-fastpq-batches` ကို ကျော်ဖြတ်ပါ။)
   အသုတ်ဝင်ရောက်မှုတိုင်းသည် ယခု `row_usage` အရာဝတ္ထု (`total_rows`၊ `transfer_rows`၊
   `non_transfer_rows`၊ တစ်ခုချင်းရွေးချယ်မှုအရေအတွက်များနှင့် `transfer_ratio`)။ JSON အတိုအထွာကို သိမ်းဆည်းပါ။
   စာသားအကြမ်းများကို ပြန်လည်လုပ်ဆောင်နေပါသည်။【crates/iroha_cli/src/audit.rs:209】 ဖမ်းယူမှုအသစ်နှင့် နှိုင်းယှဉ်ပါ
   `scripts/fastpq/check_row_usage.py` ဖြင့် ယခင်အခြေခံလိုင်းသည် လွှဲပြောင်းမှုအချိုးများ သို့မဟုတ် CI ပျက်ကွက်ပါက၊
   စုစုပေါင်းအတန်းများ နောက်ပြန်ဆုတ်သည်-

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```မီးခိုးစမ်းသပ်မှုများအတွက် နမူနာ JSON blobs များသည် `scripts/fastpq/examples/` တွင် နေထိုင်ပါသည်။ ပြည်တွင်း၌ သင်သည် `make check-fastpq-row-usage` ကို အသုံးပြုနိုင်သည်။
   (`ci/check_fastpq_row_usage.sh`) နှင့် CI သည် ကျူးလွန်ထားသည်ကို နှိုင်းယှဉ်ရန် `.github/workflows/fastpq-row-usage.yml` မှတစ်ဆင့် တူညီသော script ကို လုပ်ဆောင်သည်
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` လျှပ်တစ်ပြက်ရိုက်ချက်များကြောင့် အထောက်အထားအစုအဝေးသည် အချိန်တိုင်းတွင် မြန်ဆန်စွာပျက်ကွက်သည်
   လွှဲပြောင်းအတန်းများ creep back up ။ စက်ဖြင့်ဖတ်နိုင်သော ကွဲပြားမှုကို လိုချင်ပါက `--summary-out <path>` ကို ဖြတ်ပါ (CI အလုပ်သည် `fastpq_row_usage_summary.json` ကို အပ်လုဒ်လုပ်သည်)။
   ExecWitness သည် အဆင်မပြေသောအခါ၊ ဆုတ်ယုတ်မှုနမူနာကို `fastpq_row_bench` ဖြင့် ပေါင်းစပ်ပါ
   အတိအကျတူညီသော `row_usage` ထုတ်လွှတ်သော (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)
   ပြုပြင်နိုင်သော ရွေးချယ်နိုင်သော အရေအတွက်များအတွက် အရာဝတ္ထု (ဥပမာ၊ 65536 အတန်း ဖိစီးမှုစမ်းသပ်မှု)-

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Stage7-3 ဖြန့်ချိရေးအစုအဝေးများသည် `scripts/fastpq/validate_row_usage_snapshot.py` ကိုလည်း ကျော်ဖြတ်ရမည်၊
   `row_usage` တွင် ထည့်သွင်းမှုတိုင်းတွင် ရွေးချယ်သူအရေအတွက်များ ပါရှိပြီး ၎င်းကို တွန်းအားပေးပါသည်။
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` သည် အကူအညီပေးသူကို ခေါ်သည်။
   GPU လမ်းကြောများကို ပြဌာန်းခြင်းမပြုမီ ထိုပုံစံကွဲများ ပျောက်နေသော အစုအဝေးများသည် အလိုအလျောက် ပျက်သွားပါသည်။【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       bench manifest gate သည် `--max-operation-ms lde=950` မှတစ်ဆင့် ၎င်းအား တွန်းအားပေးသည်၊ ထို့ကြောင့် ပြန်လည်ဆန်းသစ်ပါ။
       မင်းရဲ့သက်သေတွေက အဲဒီဘောင်ကိုကျော်လွန်တဲ့အခါတိုင်း ဖမ်းပါ။
      တူရိယာ အထောက်အထားများ လိုအပ်သောအခါ `--trace-dir <dir>` ကို ကျော်ပြီး ကြိုးကို
      `xcrun xctrace record` (မူရင်း “Metal System Trace” template) မှတဆင့် သူ့ကိုယ်သူ ပြန်လည်စတင်ပြီး
      JSON နှင့်အတူ အချိန်တံဆိပ်ရိုက်ထားသော `.trace` ဖိုင်ကို သိမ်းဆည်းပါ။ သင်သည် တည်နေရာကို လွှမ်းမိုးနိုင်ဆဲ /
      `--trace-output <path>` နှင့် စိတ်ကြိုက်ရွေးချယ်နိုင်သော `--trace-template` နှင့် ကိုယ်တိုင်ပုံစံပလိတ်
      `--trace-seconds`။ ရလာတဲ့ JSON က `metal_trace_{template,seconds,output}` ဆိုတော့ ကြော်ငြာတယ်။
      Artefact အစုအဝေးများသည် ဖမ်းယူထားသော သဲလွန်စကို အမြဲခွဲခြားသတ်မှတ်ပါသည်။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      ဖမ်းယူမှုတစ်ခုစီကို ထုပ်ပိုးပါ။
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (လက်မှတ်ထိုးသည့်အထောက်အထားကို ပင်ထိုးရန်လိုအပ်ပါက `--gpg-key <fingerprint>` ကိုထည့်ပါ) ထို့ကြောင့် အတွဲလိုက်ပျက်သွားသည်
       GPU LDE ဆိုသည်မှာ 950ms ပစ်မှတ်ကို ချိုးဖောက်သည့်အခါတိုင်း၊ Poseidon ထက် 1s သို့မဟုတ်
       Poseidon တယ်လီမီတာဘလောက်များ ပျောက်ဆုံးနေပြီး `row_usage_snapshot` ကို ထည့်သွင်းထားသည်
      JSON ၏ဘေးတွင်၊ `benchmarks.poseidon_microbench` အောက်တွင် Poseidon microbench အနှစ်ချုပ်ကို ခင်းကျင်းထားသည်။
      runbooks နှင့် Grafana dashboard အတွက် မက်တာဒေတာကို သယ်ဆောင်ဆဲ
    (`dashboards/grafana/fastpq_acceleration.json`)။ JSON သည် ယခု `speedup.ratio` / ထုတ်ပေးသည်
     လုပ်ဆောင်ချက်တစ်ခုလျှင် `speedup.delta_ms` ဖြစ်သောကြောင့် ထုတ်ပြန်ချက်အထောက်အထားများသည် GPU နှင့် သက်သေပြနိုင်သည်။
     အကြမ်းနမူနာများကို ပြန်လည်မလုပ်ဆောင်ဘဲ CPU သည် အမြတ်များပြီး wrapper သည် နှစ်ခုလုံးကို မိတ္တူပွားသည်။
     Zero-fill ကိန်းဂဏန်းများ (+ `queue_delta`) မှ `zero_fill_hotspots` သို့ (bytes၊ latency၊ ဆင်းသက်လာသည်
     GB/s) ၊ `metadata.metal_trace` အောက်တွင် Instruments metadata ကို မှတ်တမ်းတင်ပြီး စိတ်ကြိုက်ရွေးချယ်နိုင်သည်
     `--row-usage <decoded witness>` ကို ထောက်ပံ့ပေးသောအခါတွင် `metadata.row_usage_snapshot` ဘလောက်ကို ပြားစေပြီး၊
     per-kernel counters များသည် `benchmarks.kernel_summary` သို့ padding ပိတ်ဆို့မှုများ၊ သတ္တုတန်းစီခြင်း၊
     အသုံးပြုမှု၊ kernel ၏နေထိုင်မှုနှင့် bandwidth ဆုတ်ယုတ်မှုများကို တစ်ချက်ခြင်းမရှိဘဲ မြင်နိုင်သည်
     အကြမ်းဖျဉ်း အစီရင်ခံစာ။
     အတန်းအသုံးပြုမှုလျှပ်တစ်ပြက်ရိုက်ချက်သည် ထုပ်ပိုးထားသောပစ္စည်းများနှင့်အတူ လည်ပတ်နေပြီဖြစ်သောကြောင့် လက်မှတ်များကို ရိုးရိုးရှင်းရှင်းပင်၊
     ဒုတိယ JSON အတိုအထွာကို ပူးတွဲမည့်အစား အစုအဝေးကို ကိုးကားပါ၊ CI သည် မြှုပ်သွင်းထားသော ကွဲပြားနိုင်သည်
    Stage7 တင်ပြမှုများကို အတည်ပြုသည့်အခါ တိုက်ရိုက်ရေတွက်သည်။ microbench ဒေတာကို သူ့ဘာသာသူ သိမ်းဆည်းရန်၊
    `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json` ကို run ပါ။
    ရလဒ်ဖိုင်ကို `benchmarks/poseidon/` အောက်တွင် သိမ်းဆည်းပါ။ စုစည်းထားသော သရုပ်ကို အသစ်အဆန်းဖြင့် ထားပါ။
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    ထို့ကြောင့် ဒက်ရှ်ဘုတ်များ/CI သည် ဖိုင်တစ်ခုစီကို ကိုယ်တိုင်မလျှောက်ဘဲ မှတ်တမ်းအပြည့်အစုံကို ကွဲပြားနိုင်သည်။4. `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus အဆုံးမှတ်) သို့မဟုတ် `telemetry::fastpq.execution_mode` မှတ်တမ်းများကို ရှာဖွေခြင်းဖြင့် တယ်လီမီတာကို အတည်ပြုပါ မထင်မှတ်ထားသော `resolved="cpu"` ထည့်သွင်းမှုများသည် GPU ရည်ရွယ်ချက်ရှိသော်လည်း လက်ခံသူသည် ပြန်ကျသွားသည်ကို ညွှန်ပြပါသည်။ 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. ပြုပြင်ထိန်းသိမ်းနေစဉ်အတွင်း CPU လုပ်ဆောင်ချက်ကို တွန်းအားပေးရန်နှင့် မှားယွင်းသောမှတ်တမ်းများ ပေါ်လာသေးကြောင်း အတည်ပြုရန် `FASTPQ_GPU=cpu` (သို့မဟုတ် config knob) ကိုသုံးပါ။ ၎င်းသည် SRE runbooks များကို အဆုံးအဖြတ်ပေးသည့်လမ်းကြောင်းနှင့် လိုက်လျောညီထွေဖြစ်စေသည်။【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetry နှင့် နောက်ပြန်ဆုတ်ခြင်း-
  - အကောင်အထည်ဖော်မှုမုဒ်မှတ်တမ်းများ (`telemetry::fastpq.execution_mode`) နှင့် ကောင်တာများ (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) သည် တောင်းဆိုထားသည့် ဆန့်ကျင်ဘက်ဖြေရှင်းမုဒ်ကို ဖော်ထုတ်ပြသနိုင်သောကြောင့် အသံတိတ်တုံ့ပြန်မှုများကို မြင်နိုင်သည် ဒက်ရှ်ဘုတ်များ။【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ Acceleration Overview` Grafana ဘုတ် (`dashboards/grafana/fastpq_acceleration.json`) သည် Metal မွေးစားနှုန်းကို မြင်ယောင်ပြီး စံအမှတ်အသားများကို ပြန်လည်ချိတ်ဆက်ပေးကာ တွဲချိတ်ထားသည့်သတိပေးစည်းမျဉ်းများ (`dashboards/alerts/fastpq_acceleration_rules.yml`) ဂိတ်များကို ရပ်ဆိုင်းလိုက်ပါသည်။
  - `FASTPQ_GPU={auto,cpu,gpu}` သည် overrides များကို ဆက်လက်ပံ့ပိုးထားသည်။ အမည်မသိတန်ဖိုးများသည် သတိပေးချက်များ တိုးလာသော်လည်း စာရင်းစစ်အတွက် telemetry တွင် ဆက်လက်ပျံ့နှံ့နေပါသည်။【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - GPU parity စမ်းသပ်မှုများ (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) သည် CUDA နှင့် Metal အတွက် အောင်မြင်ရမည်။ metallib မရှိခြင်း သို့မဟုတ် ထောက်လှမ်းမှု မအောင်မြင်သည့်အခါ CI သည် လှပစွာ ကျော်သွားပါသည်။ 【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - သတ္တုဆိုင်ရာ အဆင်သင့်ဖြစ်မှု အထောက်အထားများ (ပြသမှုတိုင်းနှင့် အောက်ဖော်ပြပါအရာများကို သိမ်းဆည်းထားပါ၊ သို့မှသာ လမ်းပြမြေပုံစာရင်းစစ်သည် အဆုံးအဖြတ်ပေးမှု၊ တယ်လီမီတာလွှမ်းခြုံမှုနှင့် တုံ့ပြန်မှုအပြုအမူတို့ကို သက်သေပြနိုင်သည်-။| အဆင့် | ပန်းတိုင် | Command / Evidence |
    | ----| ----| ------------------|
    | metallib ဆောက် | `xcrun metal`/`xcrun metallib` ကိုရရှိနိုင်ကြောင်းသေချာစေပြီး ဤကော်မတီအတွက် အဆုံးအဖြတ်ပေးသော `.metallib` ကို ထုတ်လွှတ်ပါ | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; တင်ပို့သည့် `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`။ 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | env var | ကိုအတည်ပြုပါ။ build script | မှမှတ်တမ်းတင်ထားသော env var ကိုစစ်ဆေးခြင်းဖြင့် Metal ကိုဆက်လက်ဖွင့်ထားကြောင်းအတည်ပြုပါ။ `echo $FASTPQ_METAL_LIB` (ပကတိလမ်းကြောင်းကို ပြန်ရပါမည်၊ ဗလာဆိုသည်မှာ နောက်ခံဖိုင်ကို ပိတ်ထားသည်)။【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU တူညီမှုအစုံ | | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` နှင့် `backend="metal"` သို့မဟုတ် နောက်ပြန်ဆုတ်ခြင်းသတိပေးချက်ကို ပြသသည့် ရလဒ်ထွက်မှတ်တမ်းအတိုအထွာကို သိမ်းဆည်းပါ။【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | စံနမူနာ | `speedup.*` နှင့် FFT ချိန်ညှိမှုကို မှတ်တမ်းတင်သည့် JSON/မှတ်တမ်းအတွဲကို ဖမ်းယူပါ၊ သို့မှသာ ဒက်ရှ်ဘုတ်များသည် အရှိန်မြှင့်ကိရိယာ အထောက်အထားများကို ထည့်သွင်းနိုင်သည် | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; JSON ၊ အချိန်တံဆိပ်ရိုက်ထားသော `.trace` နှင့် ထုတ်ဝေရေးမှတ်စုများနှင့်အတူ stdout တို့ကို သိမ်းဆည်းထားပါက Grafana ဘုတ်သည် Metal လည်ပတ်မှုကို ကောက်ယူသည် (အစီရင်ခံစာသည် တောင်းဆိုထားသော 20k အတန်းနှင့် padded 32,768-row domain ကို မှတ်တမ်းတင်ထားသောကြောင့် ပြန်လည်သုံးသပ်သူများသည် L4010X18NI ကို အတည်ပြုနိုင်သည် ပစ်မှတ်)။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | အစီရင်ခံစာ | GPU LDE သည် ဆိုလိုသည်မှာ 950ms ချိုးဖောက်ပါက၊ Poseidon ထက် 1s သို့မဟုတ် Poseidon တယ်လီမီတာလုပ်ကွက်များ ပျောက်ဆုံးနေပြီး လက်မှတ်ရေးထိုးထားသော artefact အစုအဝေးကို ထုတ်လုပ်မည်ဆိုပါက ထုတ်ဝေမှု မအောင်မြင်ပါ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; ထုပ်ပိုးထားသော JSON နှင့် ထုတ်လုပ်ထားသော `.json.asc` လက်မှတ် နှစ်ခုစလုံးကို စာရင်းစစ်များက အလုပ်ချိန်ကို ပြန်မလုပ်ဆောင်ဘဲ ဒုတိယ မက်ထရစ်များကို စစ်ဆေးနိုင်မည်ဖြစ်သည်။【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    | ခုံတန်းရှည် | သတ္တု/CUDA အစုအဝေးများတွင် `<1 s` LDE အထောက်အထားများကို တွန်းအားပေးပြီး ထုတ်ဝေခွင့်ပြုချက်အတွက် လက်မှတ်ထိုးထားသော မှတ်တမ်းများကို ဖမ်းယူ | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; မန်နီးဖက်စ် + လက်မှတ်ကို ထုတ်ဝေခွင့်လက်မှတ်တွင် ပူးတွဲပါရှိသောကြောင့် အောက်ပိုင်းအလိုအလျောက်စနစ်ဖြင့် ဒုတိယ အထောက်အထားမက်ထရစ်များကို အတည်ပြုနိုင်သည်။【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA အတွဲ | SM80 CUDA ဖမ်းယူမှုကို သတ္တုအထောက်အထားဖြင့် လော့ခ်ချသည့်အဆင့်တွင် ထားရှိခြင်းဖြင့် သရုပ်ပြမှုများသည် GPU အတန်းအစားနှစ်ခုလုံးကို အကျုံးဝင်စေသည်။ | Xeon+RTX host → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output` ရှိ `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`; `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` တွင် ပတ်ထားသောလမ်းကြောင်းကို ပေါင်းထည့်ပါ၊ `.json`/`.asc` အတွဲကို သတ္တုအစုအဝေးဘေးတွင် ထားရှိကာ စာရင်းစစ်များ အကိုးအကားလိုအပ်သည့်အခါ မျိုးစေ့ `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` ကို ကိုးကားပါ။ အပြင်အဆင်။【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Telemetry စစ်ဆေးခြင်း | Prometheus/OTEL မျက်နှာပြင်များကို `device_class="<matrix>", backend="metal"` ရောင်ပြန်ဟပ်ကြောင်း စစ်ဆေးအတည်ပြုပါ (သို့မဟုတ် အဆင့်နှိမ့်ချခြင်းမှတ်တမ်း) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` နှင့် စတင်ချိန်တွင် ထုတ်လွှတ်သော `telemetry::fastpq.execution_mode` မှတ်တမ်းကို ကူးယူပါ။ 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| အတင်းအဓမ္မ ဆုတ်ယုတ်မှု | SRE playbooks | အတွက် အဆုံးအဖြတ်ပေးသော CPU လမ်းကြောင်းကို မှတ်တမ်းတင်ပါ။ `FASTPQ_GPU=cpu` သို့မဟုတ် `zk.fastpq.execution_mode = "cpu"` ဖြင့် တိုတောင်းသော အလုပ်ဝန်ကို လုပ်ဆောင်ပြီး အဆင့်နှိမ့်ချမှု မှတ်တမ်းကို ဖမ်းယူနိုင်ရန် အော်ပရေတာများသည် rollback လုပ်ထုံးလုပ်နည်းကို ပြန်လည်လေ့ကျင့်နိုင်ပါသည်။【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src5/7.
    | ခြေရာခံဖမ်းယူ (ချန်လှပ်ထားနိုင်သည်) | ပရိုဖိုင်ပြုလုပ်သည့်အခါ၊ ပေးပို့သည့်ခြေရာများကို ဖမ်းယူပါ၊ ထို့ကြောင့် kernel လမ်းကြော/အကွက်များကို နောက်မှပြန်လည်သုံးသပ်နိုင်သည် | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ဖြင့် တူညီသောစမ်းသပ်မှုတစ်ခုအား ပြန်လည်လုပ်ဆောင်ပြီး ထွက်ရှိလာသော ခြေရာခံမှတ်တမ်းကို သင့်ထုတ်လွှတ်သည့်ပစ္စည်းများတွင် ပူးတွဲပါရှိသည်။ 【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    ထုတ်ဝေခွင့်လက်မှတ်နှင့်အတူ အထောက်အထားများကို သိမ်းဆည်းပြီး `docs/source/fastpq_migration_guide.md` တွင် တူညီသောစစ်ဆေးချက်စာရင်းကို ထင်ဟပ်ထားသောကြောင့် အဆင့်မြှင့်တင်ခြင်း/ထုတ်ကုန်များကို တူညီသောပြခန်းစာအုပ်အဖြစ် လိုက်နာပါ။【docs/source/fastpq_migration_guide.md:1】

### စစ်ဆေးရန်စာရင်းကို ထုတ်ပြန်ပါ။

FASTPQ ထွက်ခွင့်လက်မှတ်တိုင်းတွင် အောက်ပါဂိတ်များကို ထည့်ပါ။ ပစ္စည်းအားလုံး မထွက်မချင်း ထုတ်ဝေမှုများကို ပိတ်ဆို့ထားသည်။
ဖြည့်စွက်ပြီး ရေးထိုးထားသည့် ပစ္စည်းများအဖြစ် ပူးတွဲပါရှိသည်။

1. **ဒုတိယ သက်သေ တိုင်းတာချက်များ** — Canonical Metal benchmark ဖမ်းယူမှု
   (`fastpq_metal_bench_*.json`) သည် 20000 အတန်း အလုပ်ဝန် (32768 padded row) တွင် ပြီးကြောင်း သက်သေပြရပါမည်။
   <1 စက္ကန့် တိကျစွာပြောရလျှင် `benchmarks.operations` သည် `operation = "lde"` နှင့် ကိုက်ညီသည့်နေရာ၊
   `report.operations` နမူနာ `gpu_mean_ms ≤ 950` ကို ပြရပါမည်။ မျက်နှာကျက် လိုအပ်သည်ထက် ကျော်လွန်သော ပြေးခြင်း။
   စစ်ဆေးရေးစာရင်းကို လက်မှတ်မထိုးမီ စုံစမ်းစစ်ဆေးခြင်းနှင့် ပြန်လည်ရယူခြင်း
2. **လက်မှတ်ထိုးထားသော စံသတ်မှတ်ချက်** — လတ်ဆတ်သော Metal + CUDA အစုအဝေးများကို မှတ်တမ်းတင်ပြီးနောက်၊ လည်ပတ်ပါ။
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` ကို ထုတ်လွှတ်သည်။
   `artifacts/fastpq_bench_manifest.json` နှင့် သီးခြားလက်မှတ်
   (`artifacts/fastpq_bench_manifest.sig`)။ ဖိုင်နှစ်ခုလုံးနှင့် အများသူငှာသော့လက်ဗွေကို ဖိုင်နှစ်ခုလုံးကို ပူးတွဲပါ။
   လက်မှတ်ကို ထုတ်ဝေလိုက်သောကြောင့် သုံးသပ်သူများသည် အကျေအလည်နှင့် လက်မှတ်ကို သီးခြားစီစစ်နိုင်မည်ဖြစ်သည်။【xtask/src/fastpq.rs:1】
3. **အထောက်အထား ပူးတွဲပါဖိုင်များ** — အကြမ်းစံ JSON၊ stdout မှတ်တမ်း (သို့မဟုတ် တူရိယာခြေရာခံသည့်အခါ သိမ်းဆည်းပါ
   ဖမ်းထားသည်) နှင့် ထုတ်ဝေခွင့်လက်မှတ်ပါရှိသော မန်နီးဖက်စ်/လက်မှတ်တွဲ။ စစ်ဆေးရန်စာရင်းသည်သာဖြစ်သည်။
   လက်မှတ်သည် ထိုပစ္စည်းများထံ လင့်ခ်ချိတ်ပြီး ဖုန်းခေါ်ဆိုမှု စစ်ဆေးသူမှ အတည်ပြုသောအခါတွင် အစိမ်းရောင်ဟု သတ်မှတ်သည်။
   `fastpq_bench_manifest.json` တွင် မှတ်တမ်းတင်ထားသော အချေအတင်သည် အပ်လုဒ်လုပ်ထားသောဖိုင်များနှင့် ကိုက်ညီပါသည်။ 【artifacts/fastpq_benchmarks/README.md:1】

## အဆင့် 6 — မာကျောခြင်းနှင့် မှတ်တမ်းပြုစုခြင်း။
- Placeholder backend အငြိမ်းစား; အင်္ဂါရပ် ခလုတ်ဖွင့်ခြင်းမရှိဘဲ ပုံမှန်အားဖြင့် ထုတ်လုပ်မှု ပိုက်လိုင်းသင်္ဘောများ။
- ပြန်လည်ထုတ်လုပ်နိုင်သော တည်ဆောက်မှုများ (ပင်ထိုးကိရိယာများ၊ ကွန်တိန်နာပုံများ)။
- ခြေရာကောက်၊ SMT၊ ရှာဖွေမှုတည်ဆောက်ပုံများအတွက် Fuzzers။
- IVM အပြည့်အဝမတိုင်မီ Stage6 ပြိုင်ပွဲများကို တည်ငြိမ်နေစေရန် အုပ်ချုပ်သူအဆင့် မီးခိုးစမ်းသပ်မှုများသည် အုပ်ချုပ်မှုမဲပေးငွေများနှင့် ငွေလွှဲပို့ခြင်းများ အကျုံးဝင်ပါသည်။【crates/fastpq_prover/tests/realistic_flows.rs:1】
- သတိပေးချက်အဆင့်များ၊ ပြန်လည်ပြင်ဆင်ခြင်းလုပ်ငန်းစဉ်များ၊ စွမ်းရည်စီမံချက်ရေးဆွဲခြင်းလမ်းညွှန်ချက်များပါရှိသော Runbooks။
- CI တွင် ဗိသုကာဆိုင်ရာ သက်သေပြခြင်း (x86_64၊ ARM64)။

### Bench manifest & release gate

ယခုထုတ်ပြန်လိုက်သော အထောက်အထားများတွင် သတ္တုနှင့် နှစ်မျိုးစလုံးကို အကျုံးဝင်သည့် အဆုံးအဖြတ်ပေးသည့် သရုပ်ပြမှုတစ်ခု ပါဝင်သည်။
CUDA စံညွှန်းအတွဲများ။ ပြေး-

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```အမိန့်သည် ထုပ်ပိုးထားသော အစုအဝေးများကို တရားဝင်စေပြီး latency/speedup thresholds များကို တွန်းအားပေးသည်၊
BLAKE3 + SHA-256 သည် အချေအတင်များကို ထုတ်လွှတ်ပြီး (ရွေးချယ်နိုင်သည်) သည် manifest ဖြင့် နိမိတ်လက္ခဏာများ
Ed25519 သော့အား ထုတ်လွှတ်ခြင်းကိရိယာသည် သက်သေအထောက်အထားကို အတည်ပြုနိုင်သည်။ ကြည့်ပါ။
`xtask/src/fastpq.rs`/`xtask/src/main.rs` ကို အကောင်အထည်ဖော်ရန်အတွက် လည်းကောင်း၊
လုပ်ငန်းလည်ပတ်မှုလမ်းညွှန်မှုအတွက် `artifacts/fastpq_benchmarks/README.md`။

> **မှတ်ချက်-** `benchmarks.poseidon_microbench` ကို ချန်လှပ်ထားသည့် သတ္တုအစုအဝေးများကို ယခု ဖြစ်ပေါ်စေသည်
> ကျရှုံးမည့် မျိုးဆက်။ `scripts/fastpq/wrap_benchmark.py` ကို ပြန်ဖွင့်ပါ။
> (နှင့် `scripts/fastpq/export_poseidon_microbench.py` သီးသန့်လိုအပ်ပါက
> အကျဉ်းချုပ်) Poseidon အထောက်အထားများ ပျောက်ဆုံးသည့်အခါတိုင်း ထင်ရှားစွာ ထုတ်ပြန်ပါ။
> scalar-vs-default နှိုင်းယှဉ်မှုကို အမြဲဖမ်းယူပါ။【xtask/src/fastpq.rs:409】

`--matrix` အလံ (`artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` သို့ ပုံသေဖြစ်သည်
လက်ရှိအချိန်တွင်) ဖမ်းယူထားသော cross-device medians ကို load လုပ်ပါ။
`scripts/fastpq/capture_matrix.sh`။ မန်နီးဖက်စ်သည် အတန်း 20000 တန်း အလွှာကို ကုဒ်လုပ်ထားသည်။
စက်ပစ္စည်းအတန်းတိုင်းအတွက် လုပ်ဆောင်ချက် latency/speedup ကန့်သတ်ချက်များ၊ ထို့ကြောင့် စိတ်ကြိုက်လုပ်ဆောင်ပါ။
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` အစားထိုးခြင်းများ မရှိပါ။
တိကျသော ဆုတ်ယုတ်မှုကို အမှားရှာမဖွေပါက အချိန်ပိုကြာရန် လိုအပ်ပါသည်။

ထုပ်ပိုးထားသော စံနှုန်းလမ်းကြောင်းများကို ထည့်သွင်းခြင်းဖြင့် matrix ကို ပြန်လည်စတင်ပါ။
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` စာရင်းများနှင့် လုပ်ဆောင်နေသည်။
`scripts/fastpq/capture_matrix.sh`။ ဇာတ်ညွှန်းသည် စက်တစ်ခုစီတိုင်း၏ မီဒီယာများကို လျှပ်တစ်ပြက်ရိုက်သည်၊
ပေါင်းစည်းထားသော `matrix_manifest.json` ကို ထုတ်လွှတ်ပြီး ဆက်စပ်လမ်းကြောင်းကို ပရင့်ထုတ်သည် ။
`cargo xtask fastpq-bench-manifest` လောင်မည်။ AppleM4၊ Xeon+RTX နှင့်
Neoverse+MI300 ရိုက်ကူးမှုစာရင်းများ (`devices/apple-m4-metal.txt`၊
`devices/xeon-rtx-sm80.txt`၊ `devices/neoverse-mi300.txt`) နှင့် ၎င်းတို့၏ထုပ်ပိုးထားသည်
စံနှုန်းအစုအဝေးများ
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`၊
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`၊
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) ကို ယခု စစ်ဆေးပြီးပါပြီ။
တွင်၊ ထို့ကြောင့် ထုတ်ဝေမှုတိုင်းသည် မန်နီးဖက်စ်မတိုင်မီတွင် တူညီသော စက်ပစ္စည်းဖြတ်ကျော် မီဒီယံများကို ပြဋ္ဌာန်းထားသည်။
သည် လက်မှတ်ရေးထိုးခဲ့သည်။ 【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1 】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## ဝေဖန်ချက် အကျဉ်းချုပ်နှင့် လုပ်ဆောင်ချက်များကို ဖွင့်ပါ။

## အဆင့် 7 — ရေယာဉ်မွေးစားမှုနှင့် သက်သေအထောက်အထားများ

Stage 7 သည် “မှတ်တမ်းယူထားပြီး စံချိန်စံညွှန်းသတ်မှတ်ထားသော” (Stage6) မှ သက်သေကို ယူသည်။
"ထုတ်လုပ်ရေးယာဉ်များအတွက် မူရင်း-အဆင်သင့်"။ အဓိက ကတော့ telemetry ingestion ၊
စက်ပစ္စည်းနှစ်ခုမှ ဖမ်းယူမှု တူညီမှု၊ နှင့် အော်ပရေတာ အထောက်အထား အစုအစည်းများသည် GPU အရှိန်မြှင့်ခြင်း။
တိကျစွာ ပြဌာန်းနိုင်သည်။- ** အဆင့် 7-1 — ရေယာဉ်စု တယ်လီမီတာ ထည့်သွင်းခြင်းနှင့် SLO များ။** ထုတ်လုပ်မှု ဒက်ရှ်ဘုတ်များ
  (`dashboards/grafana/fastpq_acceleration.json`) အသက်ရှင်ရန်အတွက် ကြိုးတပ်ရပါမည်။
  Prometheus/OTel feeds သည် Alertmanager လွှမ်းခြုံမှုဖြင့် တန်းစီနေသော အနက်ပိုင်းဆိုင်များ၊
  Zero-fill regressions နှင့် silent CPU ကျဆင်းမှုများ။ သတိပေးချက်အထုပ်သည် အောက်တွင် ရှိနေသည်။
  `dashboards/alerts/fastpq_acceleration_rules.yml` နှင့် တူညီသော အထောက်အထားများ ကျက်သည်။
  Stage6 တွင် လိုအပ်သောအတွဲ။【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  ယခုအခါ ဒက်ရှ်ဘုတ်သည် `device_class`၊ `chip_family` အတွက် နမူနာပုံစံများကို ဖော်ထုတ်ပေးပါသည်။
  နှင့် `gpu_kind`၊ အော်ပရေတာများအား အတိအကျမက်ထရစ်ဖြင့် သတ္တုမွေးစားခြင်းကို ညွှန်ပြစေခြင်း
  တံဆိပ် (ဥပမာ၊ `apple-m4-max`)၊ Apple ချစ်ပ်မိသားစု၊ သို့မဟုတ် သီးခြားဆန့်ကျင်မှုအားဖြင့်
  မေးခွန်းများကို မတည်းဖြတ်ဘဲ ပေါင်းစပ်ထားသော GPU အတန်းများ။
  `irohad --features fastpq-gpu` ဖြင့် တည်ဆောက်ထားသော macOS node များကို ယခု ထုတ်လွှတ်ပါသည်။
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`၊
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (အလုပ်များ/ထပ်နေသော အချိုးများ) နှင့်
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (ကန့်သတ်၊ max_in_flight၊ dispatch_count၊ window_seconds) ထို့ကြောင့် ဒိုင်ခွက်များနှင့်
  Alertmanager စည်းမျဉ်းများသည် Metal semaphore duty-cycle/headroom မှ တိုက်ရိုက်ဖတ်နိုင်သည်။
  စံသတ်မှတ်ချက်အတွဲကို မစောင့်ဘဲ Prometheus။ လက်ခံဆောင်ရွက်ပေးနေပြီဖြစ်သည်။
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` နှင့်
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` အခါတိုင်း
  LDE အကူအညီပေးသူက GPU အကဲဖြတ်ခြင်းကြားခံများကို သုညဖြစ်ပြီး Alertmanager က ရရှိခဲ့သည်။
  `FastpqQueueHeadroomLow` (10m အတွက် headroom < 1) နှင့်
  `FastpqZeroFillRegression` (15m အထက် 0.40ms အထက်) စည်းမျဥ်းများ တန်းစီရန် headroom နှင့်
  zero-fill regressions စာမျက်နှာအော်ပရေတာများကို စောင့်ဆိုင်းမည့်အစား ချက်ချင်းလုပ်ဆောင်ပါ။
  နောက်ထုပ်ထားသော စံသတ်မှတ်ချက်။ `FastpqCpuFallbackBurst` စာမျက်နှာအဆင့် သတိပေးချက်အသစ်တစ်ခု
  GPU သည် အလုပ်တာဝန်၏ 5% ကျော်အတွက် CPU backend တွင်ရောက်ရှိရန် တောင်းဆိုသည်၊
  အော်ပရေတာများအား အထောက်အထားများနှင့် root-ဖြစ်စေသော ယာယီ GPU ချို့ယွင်းမှုများကို ဖမ်းယူရန် အတင်းအကျပ်ခိုင်းစေခြင်း။
  ထုတ်ဝေမှုကို ပြန်မစမ်းမီ။【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alertspq_tests/】
  ယခုအခါ SLO သတ်မှတ်မှုသည် ≥50% Metal duty-cycle ပစ်မှတ်ကို အဆိုပါမှတစ်ဆင့် ပြဋ္ဌာန်းထားသည်။
  ပျမ်းမျှအားဖြင့် `FastpqQueueDutyCycleDrop` စည်းမျဉ်း
  `fastpq_metal_queue_ratio{metric="busy"}` သည် 15 မိနစ်ကြာ ဝင်းဒိုး နှင့် လူးလိမ့်နေသည်။
  GPU အလုပ်အား အချိန်ဇယားဆွဲထားဆဲဖြစ်သော်လည်း တန်းစီခြင်းကို ထိန်းသိမ်းရန် ပျက်ကွက်သည့်အခါတိုင်း သတိပေးသည်။
  လိုအပ်သောနေထိုင်မှု။ ၎င်းသည် တိုက်ရိုက် တယ်လီမီတာ စာချုပ်ကို ၎င်းနှင့် လိုက်လျောညီထွေဖြစ်စေသည်။
  GPU လမ်းကြောင်းများကို ပြဌာန်းခြင်းမပြုမီ စံပြအထောက်အထားများ။【 dashboards/alerts/fastpq_acceleration_rules.yml:1 】【 dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1 】
- **Stage7-2 — Cross-device capture matrix.** အသစ်
  `scripts/fastpq/capture_matrix.sh` တည်ဆောက်သည်။
  စက်တစ်ခုစီမှ `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
  `artifacts/fastpq_benchmarks/matrix/devices/` အောက်တွင် ရိုက်ကူးထားသော စာရင်းများ။ AppleM4၊
  Xeon+RTX၊ နှင့် Neoverse+MI300 မီဒီယာများသည် ယခု ၎င်းတို့နှင့်အတူ in-repo တွင် နေထိုင်လျက်ရှိသည်။
  ထုပ်ပိုးထားသောအတွဲများ၊ ထို့ကြောင့် `cargo xtask fastpq-bench-manifest` သည် manifest ကိုဖွင့်သည်
  အလိုအလျောက်၊ အတန်း 20000 တန်းကြမ်းပြင်ကို တွန်းအားပေးပြီး စက်တစ်ခုစီကို သက်ရောက်သည်။
  ထုတ်ဝေမှုအစုအဝေးမတိုင်မီတွင် သတ်မှတ်ထားသော CLI အလံများမပါဘဲ latency/speedup ကန့်သတ်ချက်များအတည်ပြုခဲ့သည်။ 【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/:1fastpq】
စုစည်းမတည်မငြိမ်ဖြစ်ရသည့်အကြောင်းရင်းများကို ယခု matrix-pass နှင့် တွဲ၍ ပို့ပါသည်။
ထုတ်လွှတ်ရန် `--reason-summary-out` မှ `scripts/fastpq/geometry_matrix.py`
ပျက်ကွက်/သတိပေးချက်၏ JSON ၏ ဟစ်စတိုဂရမ်ကို လက်ခံဆောင်ရွက်ပေးသူ အညွှန်းနှင့် ရင်းမြစ်မှ သော့ခတ်ထားသော အကြောင်းရင်းများ
အနှစ်ချုပ်၊ ထို့ကြောင့် Stage7-2 ပြန်လည်သုံးသပ်သူများသည် CPU အမှားအယွင်းများ သို့မဟုတ် ပျောက်ဆုံးနေသော telemetry ကို ကြည့်ရှုနိုင်ပါသည်။
Markdown ဇယားအပြည့်အစုံကို စကင်န်မဖတ်ဘဲ တစ်ချက်ကြည့်လိုက်ပါ။ အခုလည်း အလားတူ ကူညီသူပါ။
`--host-label chip_family:Chip` ကို လက်ခံသည် (သော့များစွာအတွက် ပြန်လုပ်သည်) ထို့ကြောင့် အဆိုပါ
Markdown/JSON ရလဒ်များတွင် မြှုပ်နှံမည့်အစား စုစည်းထားသော လက်ခံသူ အညွှန်းကော်လံများ ပါဝင်သည်။
၎င်းသည် OS တည်ဆောက်မှုများ သို့မဟုတ် စစ်ထုတ်ရန် အသေးအဖွဲ အကြမ်းဖျင်းရှိ မက်တာဒေတာဖြစ်သည်။
Stage7-2 အထောက်အထားအစုအဝေးကို ပြုစုသောအခါ သတ္တုဒရိုင်ဘာဗားရှင်းများ။【scripts/fastpq/geometry_matrix.py:1】
Geometry သည် ISO8601 `started_at` / `completed_at` အကွက်များထဲသို့လည်း တံဆိပ်တုံးထုသည်
အနှစ်ချုပ်၊ CSV နှင့် Markdown အထွက်များ ဖမ်းယူထားသော အစုအစည်းများသည် ဝင်းဒိုးအတွက် သက်သေပြနိုင်သည်
Stage7-2 matrices အများအပြားကို lab runs များပေါင်းစည်းသောအခါ host တစ်ခုစီ။【scripts/fastpq/launch_geometry_sweep.py:1】
ယခု `scripts/fastpq/stage7_bundle.py` သည် ဂျီသြမေတြီမက်ထရစ်ကို တွဲလျက်တွဲလုပ်သည်။
`row_usage/*.json` သည် Stage7 အစုအဝေးတစ်ခုသို့ လျှပ်တစ်ပြက်ရိုက်ချက်များ (`stage7_bundle.json`
+ `stage7_geometry.md`) မှတဆင့် လွှဲပြောင်းမှုအချိုးများကို သက်သေပြနေသည်။
`validate_row_usage_snapshot.py` နှင့် ဆက်လက်တည်ရှိနေသော host/env/reason/source အနှစ်ချုပ်များ
ထို့ကြောင့် ဖြန့်ချိရေးလက်မှတ်များသည် ကယောင်ကတမ်းကစားမည့်အစား အဆုံးအဖြတ်ပေးသည့်အရာတစ်ခုကို ပူးတွဲတင်ပြနိုင်သည်။
တစ်ခုချင်း host tables။【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Stage7-3 — အော်ပရေတာမွေးစားခြင်းဆိုင်ရာ အထောက်အထားနှင့် နောက်ကြောင်းပြန်လေ့ကျင့်ခန်းများ။** အသစ်
  `docs/source/fastpq_rollout_playbook.md` သည် artefact အတွဲကို ဖော်ပြသည်။
  (`fastpq_bench_manifest.json`၊ ထုပ်ပိုးထားသော သတ္တု/CUDA ဖမ်းယူမှု၊ Grafana တင်ပို့မှု၊
  ဖြန့်ချိရေး လက်မှတ်တိုင်းတွင် တွဲပေးရမည့် အချက်ပြ လျှပ်တစ်ပြက်၊ နောက်ပြန်လှည့်မှု မှတ်တမ်းများ)
  ပေါင်းစည်းထားသော (လေယာဉ်မှူး → ချဉ်းကပ်လမ်း → ပုံသေ) အချိန်ဇယားနှင့် အတင်းအကြပ် ဆုတ်ခွာလေ့ကျင့်ခန်းများ။
  `ci/check_fastpq_rollout.sh` သည် ဤအစုအဝေးများကို အတည်ပြုပေးသောကြောင့် CI သည် Stage7 ကို တွန်းအားပေးသည်
  မထွက်ခင်မှာ ဂိတ်ပေါက်က ရှေ့ကို ဆက်သွားပါ။ ပိုက်လိုင်းက ဒီအတိုင်းပဲ ဆွဲထုတ်လို့ရပါပြီ။
  `artifacts/releases/<version>/fastpq_rollouts/…` မှတဆင့် အစုအဝေးများ
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>` ကိုသေချာစေပါသည်။
  လက်မှတ် ရေးထိုးထားသော သက်သေများနှင့် သက်သေ အထောက်အထားများသည် အတူရှိနေပါသည်။ ရည်ညွှန်းအစုအဝေးတစ်ခု အသက်ရှင်သည်။
  `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` အောက်တွင်ထားရှိရန်
  GitHub အလုပ်အသွားအလာ (`.github/workflows/fastpq-rollout.yml`) သည် မှန်ကန်နေချိန်တွင် အစိမ်းရောင်ဖြစ်သည်။
  တင်သွင်းခြင်းများကို ပြန်လည်သုံးသပ်ပါသည်။

### Stage7 FFT တန်းစီနေသော fan-outယခု `crates/fastpq_prover/src/metal.rs` သည် ယင်း `QueuePolicy` ကို ချက်ချင်းရနိုင်သည်
အိမ်ရှင်က a သတင်းပို့သည့်အခါတိုင်း Metal command များ တန်းစီခြင်းကို အလိုအလျောက် ထုတ်ပေးသည်။
သီးခြား GPU။ ပေါင်းစပ်ထားသော GPU များသည် တန်းစီခြင်းလမ်းကြောင်းကို ထိန်းသိမ်းထားသည်။
(`MIN_QUEUE_FANOUT = 1`)၊ သီးခြားစက်ပစ္စည်းများသည် ပုံသေ တန်းစီခြင်းနှစ်ခုသာဖြစ်ပြီး၊
အလုပ်တာဝန်တစ်ခုသည် အနည်းဆုံး ကော်လံ 16 ခုကို ဖုံးလွှမ်းသွားသောအခါတွင် လှုံ့ဆော်ပေးသည်။ နှစ်ခုစလုံးကို heuristic ချိန်ညှိနိုင်သည်။
`FASTPQ_METAL_QUEUE_FANOUT` နှင့် `FASTPQ_METAL_COLUMN_THRESHOLD` အသစ်မှတစ်ဆင့်
ပတ်၀န်းကျင် ပြောင်းလဲနိုင်သော ကိန်းရှင်များနှင့် အစီအစဉ်ဆွဲသူသည် အလှည့်ကျ FFT/LDE အတွဲများကို ဖြတ်ကျော်သည်။
တူညီသောတန်းစီပေါ်တွင် တွဲထားသော ကြွေပြားခင်းထားသော ပေးပို့မှုကို မထုတ်ပေးမီ တက်ကြွသောတန်းစီများ
အော်ဒါမှာခြင်း အာမခံချက် ထိန်းသိမ်းရန်။
Node အော်ပရေတာများသည် ထို env vars များကို ကိုယ်တိုင်တင်ပို့ရန် မလိုအပ်တော့ပါ။
`iroha_config` ပရိုဖိုင်သည် `fastpq.metal_queue_fanout` နှင့်
`fastpq.metal_queue_column_threshold` နှင့် `irohad` မှတဆင့် ၎င်းတို့ကို သက်ရောက်သည်
`fastpq_prover::set_metal_queue_policy` သည် Metal backend မှ အစပြုခြင်းမပြုမီ
သင်္ဘောပရိုဖိုင်များကို စိတ်ကြိုက်ပစ်လွှတ်သည့်ထုပ်ပိုးမှုများမပါဘဲ ပြန်လည်ထုတ်လုပ်နိုင်သည်။【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
ယခု FFT အစုလိုက်အပြုံလိုက်ပြောင်းပြန်များသည် အလုပ်ချိန်တစ်ခုတည်းသာရှိသည့်အခါတိုင်း တန်းစီတစ်ခုတည်းတွင် ကပ်ထားသည်။
fan-out တံခါးပေါက် (ဥပမာ၊ 16-ကော်လံ လမ်းသွား-ဟန်ချက်ညီသော ဖမ်းယူမှု) ကို ထိသည် ။
FFT/LDE/Poseidon ကော်လံကြီးများကို ချန်ထားစဉ် WP2-D အတွက် ≥1.0× parity ကို ပြန်လည်ရယူသည်
တန်းစီသည့်လမ်းကြောင်းများစွာတွင် ပို့ဆောင်ပေးပါသည်။【crates/fastpq_prover/src/metal.rs:2018】

Helper tests သည် CI လုပ်နိုင်စေရန် တန်းစီ-မူဝါဒကုပ်များနှင့် parser validation ကိုကျင့်သုံးသည်။
တည်ဆောက်သူတိုင်းတွင် GPU ဟာ့ဒ်ဝဲမလိုအပ်ဘဲ Stage7 heuristics ကို သက်သေပြပါ၊
နှင့် GPU သီးသန့်စမ်းသပ်မှုများသည် ပြန်လည်ပြသမှုတွင် ဆက်လက်ပါဝင်နေစေရန် fan-out overrides အား တွန်းအားပေးသည်။
ပုံသေအသစ်များနှင့် ထပ်တူပြုပါသည်။ 【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 စက်ပစ္စည်း အညွှန်းများနှင့် သတိပေးချက် စာချုပ်

ယခု `scripts/fastpq/wrap_benchmark.py` သည် macOS ဖမ်းယူမှုတွင် `system_profiler` ကို စစ်ဆေးနေပြီဖြစ်သည်။
ထုပ်ပိုးထားသော စံနှုန်းတိုင်းတွင် ဟာ့ဒ်ဝဲအညွှန်းများကို လက်ခံဆောင်ရွက်ပေးပြီး မှတ်တမ်းတင်ထားသောကြောင့် Fleet telemetry
နှင့် capture matrix သည် စိတ်ကြိုက်စာရင်းဇယားများမပါဘဲ စက်ဖြင့် လှည့်နိုင်သည်။ တစ်
20000 အတန်း သတ္တုဖမ်းယူမှုတွင် အောက်ပါကဲ့သို့သော အကြောင်းအရာများကို သယ်ဆောင်လာပါသည်။

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

ဤတံဆိပ်များကို `benchmarks.zero_fill_hotspots` နှင့် အတူ မျိုချမိပါသည်။
`benchmarks.metal_dispatch_queue` ဒါကြောင့် Grafana လျှပ်တစ်ပြက်ရိုက်ချက်၊ မက်ထရစ်ကို ဖမ်းယူ၊
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) နှင့် Alertmanager
မက်ထရစ်များကို ထုတ်လုပ်သည့် ဟာ့ဒ်ဝဲ အတန်းအစား အထောက်အထားအားလုံးက သဘောတူသည်။ ဟိ
`--label` အလံသည် ဓာတ်ခွဲခန်းလက်ခံဆောင်ရွက်ပေးသူမရှိသည့်အခါတွင် manual overrides ခွင့်ပြုဆဲဖြစ်သည်။
`system_profiler`၊ သို့သော် ယခုအခါ အလိုအလျောက်စုံစမ်းစစ်ဆေးသည့် အမှတ်အသားများသည် AppleM1–M4 နှင့် အကျုံးဝင်ပါသည်။
PCIe GPU များကို ကွက်လပ်တွင် သီးခြားခွဲထုတ်ပါ။【scripts/fastpq/wrap_benchmark.py:1】

Linux ဖမ်းယူမှုများသည် တူညီသောကုသမှုကိုခံယူသည်- `wrap_benchmark.py` သည် ယခုစစ်ဆေးနေသည်။
`/proc/cpuinfo`၊ `nvidia-smi`/`rocm-smi`၊ နှင့် `lspci` ထို့ကြောင့် CUDA နှင့် OpenCL သည် အလုပ်လုပ်သည်
`cpu_model`၊ `gpu_model` နှင့် canonical `device_class` (`xeon-rtx-sm80`
Stage7 CUDA လက်ခံသူအတွက်၊ MI300A ဓာတ်ခွဲခန်းအတွက် `neoverse-mi300`)။ အော်ပရေတာများ လုပ်နိုင်သည်။
အလိုအလျောက်ရှာဖွေတွေ့ရှိထားသောတန်ဖိုးများကို လွှမ်းမိုးနေဆဲဖြစ်သော်လည်း Stage7 အထောက်အထားအစုအဝေးများမရှိတော့ပါ။
မှန်ကန်သောစက်ပစ္စည်းဖြင့် Xeon/Neoverse ဖမ်းယူမှုများကို တဂ်ရန် ကိုယ်တိုင်တည်းဖြတ်မှုများ လိုအပ်သည်။
မက်တာဒေတာ။runtime တွင် host တစ်ခုစီသည် `fastpq.device_class`၊ `fastpq.chip_family` နှင့်
`fastpq.gpu_kind` (သို့မဟုတ် သက်ဆိုင်ရာ `FASTPQ_*` ပတ်ဝန်းကျင် ကိန်းရှင်များ)
ဖမ်းယူမှုအစုအဝေးတွင် ပေါ်လာသည့် တူညီသော matrix အညွှန်းများ Prometheus ကို ထုတ်ယူရန်
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` နှင့်
FASTPQ Acceleration dashboard သည် axes သုံးခုထဲမှ တစ်ခုခုကို စစ်ထုတ်နိုင်သည်။ ဟိ
Alertmanager သည် တူညီသော အညွှန်းသတ်မှတ်မှုထက် စည်းမျဥ်းများ စုစည်းကာ အော်ပရေတာများကို ဇယားကွက်ခွင့်ပြုသည်။
ဟာ့ဒ်ဝဲပရိုဖိုင်တစ်ခုအတွက် မွေးစားခြင်း၊ အဆင့်နှိမ့်ချခြင်းနှင့် တုံ့ပြန်မှုများ
ရေယာဉ်စု အချိုးအစား။ 【crates/iroha_config/src/parameters/user.rs:1224】【 dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

telemetry SLO/သတိပေးချက် စာချုပ်သည် ယခုအခါ ဖမ်းယူထားသော မက်ထရစ်များကို အဆင့် 7 သို့ ပြန်လည် ချိတ်ဆက်ပေးပါသည်။
တံခါးများ။ အောက်ဖော်ပြပါဇယားသည် အချက်ပြမှုများနှင့် လိုက်နာကျင့်သုံးရမည့်အချက်များကို အကျဉ်းချုပ်ဖော်ပြသည်-

| အချက်ပြ | အရင်းအမြစ် | ပစ်မှတ် / Trigger | ပြဋ္ဌာန်း |
| ------| ------| ---------------- | ----------- |
| GPU မွေးစားမှုအချိုး | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | တစ်ခုလျှင် (device_class၊ chip_family၊ gpu_kind) ဆုံးဖြတ်ချက်များ၏ ≥95% သည် `resolved="gpu", backend="metal"` ပေါ်တွင် ဆင်းသက်ရပါမည်။ 15m နှင့်အထက် 50% အောက်သို့ ကျဆင်းသွားသောအခါ စာမျက်နှာ `FastpqMetalDowngrade` သတိပေးချက် (စာမျက်နှာ) 【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| နောက်ခံကွာဟချက် | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | triplet တိုင်းအတွက် 0 တွင် ရှိနေရမည်။ ဆက်တိုက် (>10m) ပေါက်ကွဲပြီးနောက် သတိပေးပါ။ `FastpqBackendNoneBurst` သတိပေးချက် (သတိပေးချက်)【 dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU အချိုးအစား | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | GPU တောင်းဆိုထားသည့် အထောက်အထားများ ≤5% သည် မည်သည့် triplet မဆိုအတွက် CPU backend ပေါ်တွင် ရောက်သွားနိုင်သည်။ triplet သည် ≥10m | အတွက် 5% ကျော်လွန်သောအခါ၊ `FastpqCpuFallbackBurst` သတိပေးချက် (စာမျက်နှာ)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| သတ္တုတန်းစီဂျူတီစက်ဝန်း | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | GPU အလုပ်များတန်းစီသည့်အခါတိုင်း 15m ပျမ်းမျှ ≥50% ရှိနေရမည်၊ GPU တောင်းဆိုမှုများ ဆက်လက်ရှိနေချိန်တွင် အသုံးပြုမှုသည် ပစ်မှတ်အောက်သို့ ကျဆင်းသွားသည့်အခါ သတိပေးပါ။ `FastpqQueueDutyCycleDrop` သတိပေးချက် (သတိပေးချက်)【 dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| တန်းစီအတိမ်အနက်နှင့် သုည-ဖြည့်စွက်ဘတ်ဂျက် | စံညွှန်းအမှတ် `metal_dispatch_queue` နှင့် `zero_fill_hotspots` ထုပ်လုပ်ကွက် | `max_in_flight` သည် `limit` အောက်တွင် အနည်းဆုံး အပေါက်တစ်ခုရှိနေရမည်ဖြစ်ပြီး LDE သုည-ဖြည့်စွက်ဆိုလိုသည်မှာ စည်းမျဉ်းစည်းကမ်း 20000-အတန်းအတွက် ≤0.4ms (≈80GB/s) ရှိနေရမည်၊ ဆုတ်ယုတ်မှုမှန်သမျှသည် ဖြန့်ချိရေးအစုအဝေး | `scripts/fastpq/wrap_benchmark.py` ထုတ်ပေးမှုမှတစ်ဆင့် ပြန်လည်သုံးသပ်ပြီး Stage7 အထောက်အထားအစုအဝေး (`docs/source/fastpq_rollout_playbook.md`) တွင် တွဲထားသည်။ |
| Runtime တန်းစီဇယား headroom | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | triplet တစ်ခုစီအတွက် `limit - max_in_flight ≥ 1`; headroom မပါဘဲ 10m အကြာတွင် သတိပေးပါ။ `FastpqQueueHeadroomLow` သတိပေးချက် (သတိပေးချက်)【 dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Runtime zero-fill latency | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | နောက်ဆုံး သုည-ဖြည့်စွက်နမူနာသည် ≤0.40ms (Stage7 ကန့်သတ်ချက်) | ရှိနေရမည်။ `FastpqZeroFillRegression` သတိပေးချက် (စာမျက်နှာ)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |wrapper သည် zero-fill row ကို တိုက်ရိုက် တွန်းအားပေးသည်။ သည်း
`--require-zero-fill-max-ms 0.40` မှ `scripts/fastpq/wrap_benchmark.py` နှင့် ၎င်း
ခုံတန်းလျား JSON သည် zero-fill telemetry မရှိသောအခါ သို့မဟုတ် အပူဆုံးအချိန်တွင် ပျက်လိမ့်မည်။
သုည-ဖြည့်စွက်နမူနာသည် Stage7 ဘတ်ဂျက်ထက် ကျော်လွန်ကာ ဖြန့်ချိသည့်အစုအဝေးများကို တားဆီးထားသည်။
သက်သေအထောက်အထားမပါဘဲ ပို့ဆောင်ခြင်း။【scripts/fastpq/wrap_benchmark.py:1008】

#### Stage7-1 သတိပေးချက်-ကိုင်တွယ်စစ်ဆေးရန်စာရင်း

အထက်ဖော်ပြပါသတိပေးချက်တိုင်းတွင် အော်ပရေတာများက စုဆောင်းထားသည့် သီးသန့်ခေါ်ဆိုမှုလေ့ကျင့်ခန်းကို ပေးသည်။
ထုတ်ဝေမှုအစုအဝေးတွင် လိုအပ်သည့် တူညီသည့်အရာများ-

1. **`FastpqQueueHeadroomLow` (သတိပေးချက်)။** ချက်ခြင်း Prometheus မေးမြန်းချက်ကို လုပ်ဆောင်ပါ။
   `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` နှင့်
   `fastpq-acceleration` မှ Grafana “Queue headroom” panel ကို ဖမ်းယူပါ။
   ဘုတ်အဖွဲ့။ မေးမြန်းမှုရလဒ်ကို မှတ်တမ်းတင်ပါ။
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   သတိပေးချက် ID နှင့်အတူ တွဲထားသောကြောင့် ထုတ်ဝေမှုအစုအဝေးသည် သတိပေးချက်ဖြစ်ကြောင်း သက်သေထူသည်။
   လူတန်းမအောင်မီ အသိအမှတ်ပြုခဲ့သည်။【 dashboards/grafana/fastpq_acceleration.json:1 】
2. **`FastpqZeroFillRegression` (စာမျက်နှာ)။** စစ်ဆေးပါ။
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` နှင့် မက်ထရစ်ဖြစ်လျှင်
   ဆူညံစွာ၊ `scripts/fastpq/wrap_benchmark.py` ကို နောက်ဆုံးခုံတန်းလျား JSON တွင် ပြန်ဖွင့်ပါ။
   `zero_fill_hotspots` ဘလောက်ကို ပြန်လည်ဆန်းသစ်ရန်။ promQL အထွက်ကို ပူးတွဲပါ၊
   ဖန်သားပြင်ဓာတ်ပုံများနှင့် ခုံတန်းလျားဖိုင်ကို ဖြန့်ချိသည့်လမ်းညွှန်တွင် ပြန်လည်စတင်ပါ။ ဒါက ဖန်တီးတယ်။
   ထွက်ရှိစဉ်အတွင်း `ci/check_fastpq_rollout.sh` မျှော်လင့်ထားသည့် တူညီသောအထောက်အထား
   အတည်ပြုချက်။【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (စာမျက်နှာ)။** အဲဒါကို အတည်ပြုပါ။
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` သည် 5% ကိုကျော်လွန်သည်
   ကြမ်းပြင်၊ ထို့နောက် သက်ဆိုင်ရာ အဆင့်နှိမ့်ချမက်ဆေ့ချ်များအတွက် နမူနာ `irohad` မှတ်တမ်းများ
   (`telemetry::fastpq.execution_mode resolved="cpu"`)။ promQL အမှိုက်ပုံးကို သိမ်းဆည်းပါ။
   `metrics_cpu_fallback.prom`/`rollback_drill.log` တွင် မှတ်တမ်းကောက်နှုတ်ချက် များ ၊
   အစုအဝေးသည် အကျိုးသက်ရောက်မှုနှင့် အော်ပရေတာအသိအမှတ်ပြုမှုကို သရုပ်ပြသည်။
4. **အထောက်အထားထုပ်ပိုးမှု။** သတိပေးချက်တစ်ခုခုရှင်းလင်းပြီးနောက်၊ အဆင့် 7-3 အဆင့်များကို ပြန်ဖွင့်ပါ။
   ဖြန့်ချိသည့် ပလေးစာအုပ် (Grafana တင်ပို့မှု၊ သတိပေးချက် လျှပ်တစ်ပြက်၊ နောက်ပြန်လှည့်မှု လေ့ကျင့်ခန်း) နှင့်
   ၎င်းကို ပြန်လည်မချိတ်ဆက်မီ `ci/check_fastpq_rollout.sh` မှတစ်ဆင့် အစုအဝေးကို ပြန်လည်စစ်ဆေးပါ။
   ထုတ်ဝေခွင့်လက်မှတ်။【docs/source/fastpq_rollout_playbook.md:114】

automation ကိုနှစ်သက်သောအော်ပရေတာများသည် run နိုင်သည်။
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
တန်းစီခြင်းအခန်းခေါင်းခန်း၊ သုည-ဖြည့်စွက် နှင့် CPU အလှည့်အတွက် Prometheus API ကို မေးမြန်းရန်
အထက်ဖော်ပြပါ တိုင်းထွာချက်များ၊ အကူအညီပေးသူက ဖမ်းယူထားသော JSON ကိုရေးသည် (၎င်းနှင့် ရှေ့ဆက်သည်။
မူရင်း promQL) `metrics_headroom.prom`၊ `metrics_zero_fill.prom` သို့ လည်းကောင်း၊
ရွေးချယ်ထားသော ဖြန့်ချိရေးလမ်းညွှန်အောက်တွင် `metrics_cpu_fallback.prom` ထို့ကြောင့် ထိုဖိုင်များ
Manual curl invocations မပါဘဲ အတွဲလိုက် တွဲနိုင်ပါတယ်။ယခု `ci/check_fastpq_rollout.sh` သည် တန်းစီဇယားခေါင်းခန်းကို ပြဌာန်းပြီး သုည-ဖြည့်စွက်ပါသည်။
ဘတ်ဂျက်တိုက်ရိုက်။ ၎င်းသည် ရည်ညွှန်းထားသော `metal` ခုံတန်းလျားတစ်ခုစီကို ပိုင်းခြားထားသည်။
`fastpq_bench_manifest.json` တို့ကို ကြည့်ရှုစစ်ဆေးသည်။
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` နှင့်
`benchmarks.zero_fill_hotspots[]`၊ နှင့် headroom ပြုတ်ကျသောအခါ အတွဲလိုက်ပျက်သွားသည်။
အပေါက်တစ်ခုအောက်တွင် သို့မဟုတ် LDE ဟော့စပေါ့တစ်ခုမှ `mean_ms > 0.40` ကို အစီရင်ခံသည့်အခါ။ ဒီဟာကို စောင့်ရှောက်တယ်။
CI ရှိ Stage7 telemetry guard တွင် လုပ်ဆောင်ခဲ့သည့် manual review နှင့် ကိုက်ညီသည်။
Grafana လျှပ်တစ်ပြက်ရိုက်ချက်နှင့် အထောက်အထားများ ထုတ်ပြန်ခြင်း။【ci/check_fastpq_rollout.sh#L1】
တူညီသော validation ၏တစ်စိတ်တစ်ပိုင်းအနေဖြင့် script ကိုယခုအခါတိုင်းကိုထုပ်ပိုးကြောင်းအခိုင်အမာပြောဆိုသည်။
စံသတ်မှတ်ချက်များသည် အလိုအလျောက်တွေ့ရှိနိုင်သော ဟာ့ဒ်ဝဲအညွှန်းများ ပါ၀င်သည် (`metadata.labels.device_class`
နှင့် `metadata.labels.gpu_kind`)။ အဆိုပါ အညွှန်းများ ပျောက်ဆုံးနေသော အစုအဝေးများသည် ချက်ချင်းပျက်ကွက်၊
ပစ္စည်းများကို ထုတ်လွှတ်ခြင်း၊ Stage7-2 matrix manifests နှင့် runtime ကို အာမခံပါသည်။
ဒက်ရှ်ဘုတ်များအားလုံးသည် တူညီသော စက်ပစ္စည်းအမျိုးအစားအမည်များကို ရည်ညွှန်းပါသည်။

Grafana "နောက်ဆုံးပေါ်စံညွှန်း" အကန့်နှင့် ဆက်စပ်ထုတ်လွှတ်မှုအစုအဝေးသည် ယခုဖော်ပြချက်ကို ကိုးကားပါသည်။
`device_class`၊ သုည-ဖြည့်စွက်ဘတ်ဂျက်၊ နှင့် တန်းစီအသေးစိတ် လျှပ်တစ်ပြက် ရိုက်ချက်ကြောင့် အင်ဂျင်နီယာများကို ဖုန်းခေါ်ဆိုပါ။
နိမိတ်အတွင်း အသုံးပြုသည့် တိကျသော ဖမ်းယူမှုအတန်းအစား ထုတ်လုပ်မှု တယ်လီမီတာကို ဆက်စပ်နိုင်သည်။
off အနာဂတ် matrix ထည့်သွင်းမှုများသည် တူညီသောအညွှန်းများကို အမွေဆက်ခံသည်၊ ဆိုလိုသည်မှာ Stage7-2 စက်ကို ဆိုလိုသည်။
စာရင်းများနှင့် Prometheus ဒိုင်ခွက်များသည် AppleM4 အတွက် တစ်ခုတည်းသော အမည်ပေးအစီအစဉ်ကို မျှဝေသည်၊
M3 Max နှင့် လာမည့် MI300/RTX ဖမ်းယူမှုများ။

### Stage7-1 Fleet telemetry runbook

ပုံမှန်အားဖြင့် GPU လမ်းကြောင်းများကို မဖွင့်မီ ဤစစ်ဆေးရန်စာရင်းကို လိုက်နာပါ။
နှင့် Alertmanager ၏ စည်းမျဉ်းများသည် ထုတ်ဝေမှု ကြိုတင်ပြင်ဆင်မှုအတွင်း ဖမ်းယူထားသော တူညီသောအထောက်အထားများကို ထင်ဟပ်စေသည်-

1. ** အညွှန်းဖမ်းယူမှုနှင့် runtime hosts။** `python3 scripts/fastpq/wrap_benchmark.py`
   `metadata.labels.device_class`၊ `chip_family` နှင့် `gpu_kind` တို့ကို ထုတ်လွှတ်ထားပြီးဖြစ်သည်
   JSON ထုပ်တိုင်းအတွက်။ ထိုတံဆိပ်များနှင့် ထပ်တူကျအောင်ထားပါ။
   `fastpq.{device_class,chip_family,gpu_kind}` (သို့မဟုတ်
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) `iroha_config` အတွင်း
   ဒါကြောင့် runtime metrics တွေကို publish လုပ်ပါ။
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   နှင့် `fastpq_metal_queue_*` တိုင်းထွာများ ပေါ်လာသည့် တူညီသော ခွဲခြားသတ်မှတ်မှုများ၊
   `artifacts/fastpq_benchmarks/matrix/devices/*.txt` တွင် ဇာတ်တွေ တက်လာတဲ့အခါ
   class မှတဆင့် matrix manifest ကို ပြန်ထုတ်ပါ။
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   ထို့ကြောင့် CI နှင့် ဒက်ရှ်ဘုတ်များသည် အပိုတံဆိပ်ကို နားလည်သည်။
2. ** တန်းစီကိန်းများနှင့် မွေးစားခြင်းမက်ထရစ်များကို အတည်ပြုပါ။** `irohad --features fastpq-gpu` ကို ဖွင့်ပါ။
   တိုက်ရိုက်လူစီခြင်းကို အတည်ပြုရန် Metal hosts ပေါ်တွင် တယ်လီမီတာ အဆုံးမှတ်ကို ခြစ်ပါ။
   တိုင်းတာချက်များ တင်ပို့နေသည်-

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```ပထမ command သည် semaphore sampler သည် `busy` ကို ထုတ်လွှတ်ကြောင်း သက်သေပြသည်၊
   `overlap`, `limit`, နှင့် `max_in_flight` စီးရီးနှင့် ဒုတိယရှိမရှိကို ပြသသည်
   device class တစ်ခုစီသည် `backend="metal"` သို့ ဖြေရှင်းနေသည် သို့မဟုတ် ပြန်ကျသွားသည်
   `backend="cpu"`။ ခြစ်ပစ်မှတ်ကို Prometheus/OTel မှတဆင့် ရှေ့သို့ သွယ်တန်းပါ။
   ဒက်ရှ်ဘုတ်ကို တင်သွင်းခြင်းကြောင့် Grafana သည် ရေယာဉ်စုမြင်ကွင်းကို ချက်ချင်းစီစဉ်နိုင်သည်။
3. **ဒက်ရှ်ဘုတ် + သတိပေးချက်အစုံကို ထည့်သွင်းပါ။** တင်သွင်းပါ။
   `dashboards/grafana/fastpq_acceleration.json` သို့ Grafana (ဆက်လက်ထိန်းသိမ်းထားရန်
   Built-in Device Class၊ Chip Family နှင့် GPU Kind template variables) တို့ကို load လုပ်ပါ။
   `dashboards/alerts/fastpq_acceleration_rules.yml` သည် Alertmanager တွင် အတူတကွ
   ၎င်း၏ယူနစ်စမ်းသပ် fixture နှင့်အတူ။ စည်းမျဉ်း pack သည် `promtool` ကြိုးကြိုးကို တင်ပို့သည်။ ပြေး
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   `FastpqMetalDowngrade` နှင့် သက်သေပြရန် စည်းမျဉ်းများ ပြောင်းလဲသည့်အခါတိုင်း
   `FastpqBackendNoneBurst` သည် မှတ်တမ်းတင်ထားသော တံခါးခုံများတွင် မီးတောက်နေဆဲဖြစ်သည်။
4. ** အထောက်အထား အစုအဝေးနှင့်အတူ ဂိတ်မှ ထုတ်ပေးပါသည်။** သိမ်းဆည်းပါ။
   `docs/source/fastpq_rollout_playbook.md` သည် စတင်ထုတ်စဉ်တွင် အဆင်ပြေသည်။
   တင်ပြခြင်းဖြစ်သောကြောင့် အစုအဝေးတိုင်းသည် ထုပ်ပိုးထားသော စံသတ်မှတ်ချက်များ၊ Grafana တင်ပို့မှု၊
   အချက်ပေးအထုပ်၊ တန်းစီတယ်လီမီတာအထောက်အထားနှင့် နောက်ပြန်လှည့်မှုမှတ်တမ်းများ။ CI က ပြဋ္ဌာန်းထားပြီးသား
   စာချုပ်- `make check-fastpq-rollout` (သို့မဟုတ် ခေါ်ဆိုခြင်း။
   `ci/check_fastpq_rollout.sh --bundle <path>`) သည် အစုအဝေးကို အတည်ပြုပြီး ပြန်လည်လုပ်ဆောင်သည်။
   သတိပေးချက် စမ်းသပ်မှုများ၊ တန်းစီနေစဉ် headroom သို့မဟုတ် zero-fill လုပ်သည့်အခါတွင် လက်မှတ်မထိုးပါ။
   ဘတ်ဂျက်များ နောက်ပြန်ဆုတ်သွားသည်။
5. ** Tie သည် ပြန်လည်ပြင်ဆင်ခြင်းသို့ ပြန်လည်ရောက်ရှိစေပါသည်။** Alertmanager စာမျက်နှာများတွင် Grafana ကို အသုံးပြုပါ။
   board နှင့် ကုန်ကြမ်း Prometheus ကောင်တာများ step2 ရှိမရှိကိုအတည်ပြုပါ။
   အဆင့်နှိမ့်ချမှုများသည် တန်းစီနေသော ငတ်မွတ်မှု၊ CPU ကျဆင်းမှုများ၊ သို့မဟုတ် နောက်ကွယ်မှ ပေါက်ကြားခြင်းမှ အရင်းခံသည်။
Runbook တွင်နေထိုင်သည်။
ဤစာရွက်စာတမ်းနှင့် `docs/source/fastpq_rollout_playbook.md`၊ update ကို
သက်ဆိုင်ရာ `fastpq_execution_mode_total` နှင့်အတူ လက်မှတ်ထုတ်ပေးခြင်း၊
`fastpq_metal_queue_ratio` နှင့် `fastpq_metal_queue_depth` တို့ အတူတကွ ကောက်နုတ်ချက်များ
Grafana အကန့်သို့ လင့်ခ်များနှင့် သတိပေးချက် လျှပ်တစ်ပြက် ရိုက်ချက်များဖြင့် သုံးသပ်သူများ မြင်နိုင်စေရန်၊
SLO က အစပျိုးခဲ့တာ အတိအကျပါ။

### WP2-E — အဆင့်ဆင့်သော သတ္တုပရိုဖိုင်းပုံ လျှပ်တစ်ပြက်

`scripts/fastpq/src/bin/metal_profile.rs` သည် ထုပ်ပိုးထားသော သတ္တုဖမ်းယူမှုများကို အကျဉ်းချုပ်ဖော်ပြသည်။
ထို့ကြောင့် sub-900ms ပစ်မှတ်ကို အချိန်နှင့်အမျှ ခြေရာခံနိုင်သည် (ပြေးသည်။
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`)။
Markdown အကူအညီအသစ်
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
အောက်ဖော်ပြပါ အဆင့်ဇယားများကို ထုတ်ပေးသည် (၎င်းသည် စာသားနှင့်အတူ Markdown ကို ပရင့်ထုတ်သည်။
အနှစ်ချုပ်၊ ထို့ကြောင့် WP2-E လက်မှတ်များသည် အထောက်အထားစကားလုံးများကို ထည့်သွင်းနိုင်သည်)။ ဖမ်းမိမှုနှစ်ခုကို ခြေရာခံထားသည်။
အခုချက်ချင်း-

> ** WP2-E ကိရိယာအသစ်-** `fastpq_metal_bench --gpu-probe ...` သည် ယခု ထုတ်လွှတ်သည်
> ထောက်လှမ်းမှု လျှပ်တစ်ပြက်ရိုက်ချက် (တောင်းဆိုထားသည်/ဖြေရှင်းထားသော လုပ်ဆောင်မှုမုဒ်၊ `FASTPQ_GPU`
> အစားထိုးမှုများ၊ ရှာဖွေတွေ့ရှိထားသော နောက်ခံဖိုင်နှင့် စာရင်းကောက်ထားသော သတ္တုကိရိယာများ/မှတ်ပုံတင်အမှတ်များ)
> မည်သည့် kernels မလည်ပတ်မီ။ အတင်း GPU လည်ပတ်နေချိန်တိုင်း ဤမှတ်တမ်းကို ဖမ်းယူပါ။
> CPU လမ်းကြောင်းသို့ ပြန်ကျသွားသောကြောင့် hosts မြင်သည့် အထောက်အထားအစုအဝေးမှတ်တမ်းများ
> `MTLCopyAllDevices` ကာလအတွင်း သုညပြန်ပို့ပြီး မည်သည့်အရာများ အစားထိုးမှုများ သက်ရောက်မှုရှိခဲ့သည်။
> စံသတ်မှတ်ချက်။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Stage capture helper:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> ယခုအခါ FFT၊ LDE နှင့် Poseidon တို့အတွက် `fastpq_metal_bench` ကို တစ်ဦးချင်းစီ မောင်းနှင်ပြီး၊
> အကြမ်းဖျင်း JSON အထွက်များကို အဆင့်အလိုက် လမ်းညွှန်များအောက်တွင် သိမ်းဆည်းကာ တစ်ခုတည်းကို ထုတ်လွှတ်သည်။
> `stage_profile_summary.json` အတွဲလိုက်သည် CPU/GPU အချိန်၊ တန်းစီအတိမ်အနက်ကို မှတ်တမ်းတင်သည်
> တယ်လီမီတာ၊ ကော်လံ-အဆင့်သတ်မှတ်မှု ကိန်းဂဏန်းများ၊ ကာနယ်ပရိုဖိုင်များနှင့် ဆက်စပ်သော လမ်းကြောင်း
> ပစ္စည်းများ။ အစုခွဲတစ်ခုကို ပစ်မှတ်ထားရန် `--stage fft --stage lde --stage poseidon` ကို ကျော်ဖြတ်ပါ၊
> `--trace-template "Metal System Trace"` သတ်သတ်မှတ်မှတ် xctrace ပုံစံခွက်ကို ရွေးရန်၊
> နှင့် `--trace-dir` မှ `.trace` အစုအဝေးများကို မျှဝေထားသောတည်နေရာသို့ လမ်းကြောင်းပေးပါ။ ပူးတွဲပါ
> အကျဉ်းချုပ် JSON နှင့် WP2-E ပြဿနာတိုင်းအတွက် ထုတ်လုပ်လိုက်သော ခြေရာခံဖိုင်များကို ထို့ကြောင့် သုံးသပ်သူများ
> တန်းစီနေထိုင်မှု (`metal_dispatch_queue.*`)၊ ထပ်နေသောအချိုးများနှင့် ကွဲပြားနိုင်သည်
> အများအပြားကို manually spelunking မပါဘဲ run ဖြတ်ကျော်ဖမ်းယူထားသောလွှတ်တင်ဂျီသြမေတြီ
> `fastpq_metal_bench` တောင်းခံမှုများ။【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Queue/staging evidence helper (2026-05-09):** ယခု `scripts/fastpq/profile_queue.py`
> တစ်ခု သို့မဟုတ် တစ်ခုထက်ပိုသော `fastpq_metal_bench` JSON က ဖမ်းယူပြီး Markdown ဇယားနှစ်ခုလုံးကို ထုတ်လွှတ်သည် ။
> စက်ဖတ်နိုင်သော အနှစ်ချုပ် (`--markdown-out/--json-out`) ထို့ကြောင့် တန်းစီခြင်း၏ အတိမ်အနက်၊ ထပ်နေသော အချိုးများနှင့်၊
> host-side staging telemetry သည် WP2-E artefact တိုင်းနှင့်အတူ စီးနိုင်သည်။ ပြေးသည်။
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` သည် အောက်ဖော်ပြပါဇယားကို ထုတ်လုပ်ပြီး မော်ကွန်းတင်ထားသော Metal ဖမ်းယူမှုများကို အစီရင်ခံနေဆဲဖြစ်ကြောင်း အလံပြခဲ့သည်
> `dispatch_count = 0` နှင့် `column_staging.batches = 0`—WP2-E.1 သည် Metal အထိ ဖွင့်ထားဆဲဖြစ်သည်
> ကိရိယာတန်ဆာပလာကို တယ်လီမီတာစနစ်ဖြင့် ပြန်လည်တည်ဆောက်ထားသည်။ ထုတ်လုပ်ထားသော JSON/Markdown အနုပညာပစ္စည်းများ တိုက်ရိုက်ထုတ်လွှင့်သည်။
> စာရင်းစစ်အတွက် `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` အောက်။
> ယခု (2026-05-19) အကူအညီပေးသူက Poseidon ပိုက်လိုင်း တယ်လီမီတာ (`pipe_depth`၊
> `batches`၊ `chunk_columns`၊ နှင့် `fallbacks`) Markdown ဇယားနှင့် JSON အနှစ်ချုပ် နှစ်ခုစလုံးအတွင်း၊
> ထို့ကြောင့် WP2-E.4/6 ပြန်လည်သုံးသပ်သူများသည် GPU သည် ပိုက်လိုင်းလမ်းကြောင်းတွင်ရှိနေသည်ရှိမရှိနှင့် တစ်ခုခုရှိမရှိ သက်သေပြနိုင်သည်
> အကြမ်းဖမ်းခြင်းကို မဖွင့်ဘဲ မှားယွင်းမှုများ ဖြစ်ပေါ်ခဲ့သည်။【scripts/fastpq/profile_queue.py:1】> **ဇာတ်ခုံပရိုဖိုင်အကျဉ်းချုပ် (2026-05-30):** `scripts/fastpq/stage_profile_report.py` စားသုံးသည်
> `stage_profile_summary.json` အစုအဝေးကို `cargo xtask fastpq-stage-profile` မှ ထုတ်လွှတ်ပြီး
> Markdown နှင့် JSON အနှစ်ချုပ် နှစ်ခုလုံးကို တင်ဆက်ပေးသောကြောင့် WP2-E သုံးသပ်သူများသည် အထောက်အထားများကို လက်မှတ်များတွင် ကူးယူနိုင်သည်
> အချိန်ဇယားကို ကိုယ်တိုင်ရေးသွင်းခြင်းမပြုဘဲ။ ဖိတ်ခေါ်ပါတယ်။
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> GPU/CPU ဆိုသည်မှာ၊ အရှိန်မြှင့် deltas၊ ခြေရာခံ လွှမ်းခြုံမှုတို့ကို ဖော်ပြသည့် အဆုံးအဖြတ်ပေးသည့်ဇယားများကို ထုတ်လုပ်ရန် >
> အဆင့်အလိုက် တယ်လီမီတာ ကွာဟချက်။ JSON အထွက်သည် ဇယားကို ထင်ဟပ်စေပြီး အဆင့်အလိုက် ပြဿနာတက်ဂ်များကို မှတ်တမ်းတင်သည်။
> (`trace missing`၊ `queue telemetry missing` စသည်ဖြင့်) ထို့ကြောင့် အုပ်ချုပ်မှု အလိုအလျောက်စနစ်သည် လက်ခံအား ကွဲပြားနိုင်သည်
> WP2-E.1 တွင် ရည်ညွှန်းထားသော WP2-E.6 မှ လုပ်ဆောင်သည်။
> ** တန်ဆာပလာ/ ကိရိယာ ထပ်နေသော အကာအကွယ် (2026-06-04):** `scripts/fastpq/profile_queue.py` သည် ယခု မှတ်ချက်ပေးသည်
> FFT/LDE/Poseidon စောင့်ဆိုင်းမှုအချိုးများသည် အဆင့်အလိုက် ပြားလိုက်/စောင့်ဆိုင်းခြင်း မီလီစက္ကန့် စုစုပေါင်းများနှင့် ယှဉ်တွဲကာ ထုတ်ပေးသည်
`--max-wait-ratio <threshold>` ညံ့ဖျင်းသော ထပ်နေမှုများကို တွေ့ရှိသည့်အခါတိုင်း > ပြဿနာ။ သုံးပါ။
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> တိကျပြတ်သားသောစောင့်ဆိုင်းမှုအချိုးများဖြင့် Markdown ဇယားနှင့် JSON အတွဲနှစ်ခုလုံးကိုဖမ်းယူရန် > ထို့ကြောင့် WP2-E.5 လက်မှတ်များ
> double-buffering ဝင်းဒိုးသည် GPU အား ဖြည့်သွင်းထားခြင်း ရှိမရှိ ပြသနိုင်သည်။ plain-text console သည် output လည်းဖြစ်သည်။
> ဖုန်းခေါ်ဆိုမှုစုံစမ်းစစ်ဆေးမှုများကို ပိုမိုလွယ်ကူစေရန်အတွက် အဆင့်အလိုက် အချိုးများကို စာရင်းပြုစုထားသည်။
> **Telemetry guard + run status (2026-06-09):** `fastpq_metal_bench` သည် ယခု `run_status` ဘလောက်ကို ထုတ်လွှတ်သည်
> (နောက်ခံအညွှန်း၊ ပေးပို့မှုအရေအတွက်၊ အကြောင်းရင်းများ) နှင့် `--require-telemetry` အလံအသစ်သည် လည်ပတ်မှုမအောင်မြင်ပါ။
> GPU ချိန်ကိုက်ချိန်များ သို့မဟုတ် တန်းစီခြင်း/စတိတ်တယ်လီမီတာ ပျောက်ဆုံးသည့်အခါတိုင်း။ `profile_queue.py` သည် အပြေးကို တင်ဆက်သည်။
> သီးသန့်ကော်လံတစ်ခုအနေဖြင့် အခြေအနေကို `ok` တွင် ဖော်ပြထားခြင်းမဟုတ်သော ဖော်ပြချက်များကို လည်းကောင်း၊
> `launch_geometry_sweep.py` သည် တူညီသောအခြေအနေကို သတိပေးချက်များ/အမျိုးအစားခွဲခြားခြင်းအဖြစ် သတ်မှတ်ပေးသောကြောင့် matrices များမရနိုင်ပါ။
> CPU သို့ တိတ်တဆိတ် ပြန်ကျသွားသော သို့မဟုတ် တန်းစီခြင်း ကိရိယာတန်ဆာပလာကို ကျော်သွားသော ဖမ်းယူမှုများကို ကြာရှည်စွာ ဝန်ခံပါ။
> **Poseidon/LDE အလိုအလျောက်ချိန်ညှိခြင်း (2026-06-12):** `metal_config::poseidon_batch_multiplier()` ယခု ချိန်ခွင်
> သတ္တုလုပ်ငန်းသုံးအစုံ အရိပ်အမြွက်များနှင့် `lde_tile_stage_target()` သည် သီးခြား GPU များပေါ်တွင် အကွက်အတိမ်အနက်ကို မြှင့်တင်ပေးပါသည်။
> အသုံးပြုထားသော မြှောက်ကိန်းနှင့် အကွက်ကန့်သတ်ချက်ကို `metal_heuristics` ပိတ်ဆို့ခြင်းတွင် ထည့်သွင်းထားသည်။
> `fastpq_metal_bench` ရုပ်ထွက်နှင့် `scripts/fastpq/metal_capture_summary.py` ဖြင့် ပြန်ဆိုထားသောကြောင့် WP2-E
> အကြမ်းထည်များကို JSON မှတဆင့် မတူးဘဲ ရိုက်ကူးမှုတစ်ခုစီတွင် အသုံးပြုသည့် ပိုက်လိုင်းအဖုများကို အတိအကျ မှတ်တမ်းတင်ထားသည်။ 【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq/metal_capture_summary】

| တံဆိပ် | Dispatch | အလုပ်များ | ထပ်နေ | Max Depth | FFT ပြား | FFT စောင့် | FFT စောင့်ပါ % | LDE ပြား | LDE ခဏစောင့် | LDE ခဏစောင့်ပါ % | Poseidon ပြား | Poseidon စည်သူ | Poseidon ခဏစောင့်ပါ % | ပိုက်စူး | ပိုက်ဖောင်း | ပိုက် ပြုတ်ကျ |
|---|---|---|---|---|--|---|---|---|---|---|---|---------------|---|
| fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k လျှပ်တစ်ပြက်ရိုက်ချက် (ကြိုတင်ရေးထားသည်)

`fastpq_metal_bench_20k_latest.json`| ဇာတ်ခုံ | ကော်လံ | ထည့်သွင်းရန် len | GPU ဆိုသည်မှာ (ms) | CPU ဆိုသည်မှာ (ms) | GPU မျှဝေ | အရှိန်မြှင့် | Δ CPU (ms) |
| ---| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130.986ms (115.761–167.755) | 112.616ms (95.335–132.929) | 2.4% | 0.860× | −18.370 |
| IFFT | 16 | 32768 | 129.296ms (111.127–142.955) | 158.144ms (126.847–237.887) | 2.4% | 1.223× | +28.848 |
| LDE | 16 | 262144 | 1570.656ms (1544.397–1584.502) | 1752.523ms (1548.807–2191.930) | 29.2% | 1.116× | +181.867 |
| Poseidon | 16 | 524288 | 3548.329ms (3519.881–3576.041) | 3642.706ms (3539.055–3758.279) | 66.0% | 1.027× | +94.377 |

အဓိက သတိပြုစရာများ-1. GPU စုစုပေါင်းသည် 5.379s ဖြစ်ပြီး၊ **4.48s ကျော်** 900ms ပန်းတိုင်ဖြစ်သည်။ Poseidon
   hashing သည် ဒုတိယတွင် LDE kernel ဖြင့် runtime (≈66%) ကိုလွှမ်းမိုးထားသည်။
   နေရာ (≈29%)၊ ထို့ကြောင့် WP2-E သည် Poseidon ပိုက်လိုင်းအတိမ်အနက်နှင့် နှစ်ခုလုံးကို တိုက်ခိုက်ရန် လိုအပ်သည်။
   CPU အမှားအယွင်းများ မပျောက်မီ LDE memory residency/tiling အစီအစဉ်။
2. FFT သည် IFFT သည် scalar ထက် >1.22× ဖြစ်သော်ငြား ဆုတ်ယုတ်မှု (0.86×) ဖြစ်နေဆဲ
   လမ်းကြောင်း။ ပစ်လွှတ်ခြင်း-ဂျီသြမေတြီ ပွတ်ဆွဲမှုတစ်ခု လိုအပ်ပါသည်။
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) နားလည်ရန်၊
   FFT နေထိုင်မှုကို မထိခိုက်စေဘဲ ကယ်တင်နိုင်မလား။
   IFFT အချိန်များ။ `scripts/fastpq/launch_geometry_sweep.py` အကူအညီပေးသူက ယခု မောင်းသွားပါပြီ။
   ဤစမ်းသပ်မှုများကို အဆုံးမှ အဆုံးထိ- ကော်မာ-ခြားထားသော အစားထိုးမှုများကို ကျော်ဖြတ်ပါ (ဥပမာ၊
   `--fft-columns 16,32 --queue-fanout 1,2` နှင့်
   `--poseidon-lanes auto,256`) နှင့် ၎င်းကို ခေါ်ဆိုမည်ဖြစ်သည်။
   ပေါင်းစပ်တိုင်းအတွက် `fastpq_metal_bench`၊ JSON payloads များကို အောက်တွင် သိမ်းဆည်းပါ။
   `artifacts/fastpq_geometry/<timestamp>/` နှင့် `summary.json` အတွဲလိုက်ကို ဆက်ထားပါ
   ပြေးသူတိုင်း၏ တန်းစီအချိုးအစား၊ FFT/LDE စတင်ရွေးချယ်မှုများ၊ GPU နှင့် CPU အချိန်များကို ဖော်ပြခြင်း၊
   နှင့် လက်ခံသူ မက်တာဒေတာ (အိမ်ရှင်အမည်/တံဆိပ်၊ ပလပ်ဖောင်းသုံးဆ၊ ရှာဖွေတွေ့ရှိသည့် ကိရိယာ
   အတန်းအစား၊ GPU ရောင်းချသူ/မော်ဒယ်) ထို့ကြောင့် စက်ဖြတ်ကျော် နှိုင်းယှဉ်မှုများသည် အဆုံးအဖြတ်ရှိသည်။
   သက်သေ။ အကူအညီပေးသူက `reason_summary.json` ရဲ့ဘေးမှာ ရေးထားတယ်။
   ပုံသေအားဖြင့် အကျဉ်းချုပ်ကို လှိမ့်ရန်၊ ဂျီသြမေတြီမက်ထရစ်ကဲ့သို့ အမျိုးအစားတူသော အမျိုးအစားခွဲနည်းကို အသုံးပြုသည်။
   CPU ချို့ယွင်းချက်များနှင့် တယ်လီမီတာ ပျောက်နေပါသည်။ တဂ်ရန် `--host-label staging-m3` ကိုသုံးပါ။
   မျှဝေထားသော ဓာတ်ခွဲခန်းများမှ ရိုက်ကူးမှုများ။
   အဖော် `scripts/fastpq/geometry_matrix.py` ကိရိယာသည် ယခုအခါ တစ်ခု သို့မဟုတ် တစ်ခုကို စားသုံးမိနေပြီဖြစ်သည်။
   နောက်ထပ် အနှစ်ချုပ်အတွဲများ (`--summary hostA/summary.json --summary hostB/summary.json`)
   နှင့် ပစ်လွှတ်မှု ပုံသဏ္ဍာန်တိုင်းကို *stable* အဖြစ် တံဆိပ်တပ်သည့် Markdown/JSON ဇယားများကို ထုတ်လွှတ်သည်
   (FFT/LDE/Poseidon GPU အချိန်များကို ဖမ်းယူထားသည်) သို့မဟုတ် *မတည်မငြိမ်* (အချိန်ကုန်ခြင်း၊ CPU ဆုတ်ယုတ်ခြင်း၊
   သတ္တုမဟုတ်သော နောက်ခံအစွန်၊ သို့မဟုတ် တယ်လီမီတာ ပျောက်ဆုံးနေသည်) လက်ခံသူကော်လံများနှင့်အတူ။ ဟိ
   ယခုဇယားများတွင် ဖြေရှင်းထားသော `execution_mode`/`gpu_backend` အပေါင်း a ပါ၀င်သည်
   `Reason` ကော်လံတွင် CPU အမှားအယွင်းများနှင့် ပျောက်ဆုံးနေသော GPU အချိန်များကို သိသာထင်ရှားစွာ မြင်တွေ့နိုင်သည်။
   Timing blocks တွေရှိနေချိန်မှာတောင် Stage7 matrices တွေ၊ အကျဉ်းချုပ် စာကြောင်းရေတွက်သည်။
   တည်ငြိမ်မှု vs စုစုပေါင်းအပြေးများ။ `--operation fft|lde|poseidon_hash_columns` ကို ဖြတ်ပါ။
   တံမြက်လှည်းသည် အဆင့်တစ်ခုတည်းကို ခွဲထုတ်ရန် လိုအပ်သည့်အခါ (ဥပမာ၊ ပရိုဖိုင်သို့
   Poseidon သီးခြားစီ) နှင့် `--extra-args` ကို ခုံတန်းလျားအလိုက် အလံများအတွက် အခမဲ့ထားပါ။
   အထောက် အကူပြုသူ မှန်သမျှ လက်ခံသည်။
   အမိန့်ပေးရှေ့ဆက် (`cargo run … fastpq_metal_bench`) နှင့် စိတ်ကြိုက်ရွေးချယ်နိုင်သည်။
   `--halt-on-error` / `--timeout-seconds` အကာအရံများသည် စွမ်းဆောင်ရည် အင်ဂျင်နီယာများ တတ်နိုင်သည်
   မတူညီသော စက်များတွင် တံမြက်လှည်းကို မျိုးပွားစေပြီး နှိုင်းယှဉ်စုဆောင်းခြင်း၊
   Stage7 အတွက် စက်ပစ္စည်းပေါင်းစုံ အထောက်အထားအစုအဝေးများ။
3. `metal_dispatch_queue` တွင် `dispatch_count = 0` အစီရင်ခံသည်၊ ထို့ကြောင့် တန်းစီစောင့်ဆိုင်းမှု၊
   GPU kernels လည်ပတ်နေသော်လည်း telemetry ပျောက်နေပါသည်။ ယခု Metal runtime ကိုအသုံးပြုသည်။
   တန်းစီခြင်း/ကော်လံ-ဇာတ်ခုံများအတွက် ခြံစည်းရိုးများရယူခြင်း/ထုတ်လွှတ်ခြင်း သို့မှသာ အလုပ်သမားလိုင်းများ
   ကိရိယာတန်ဆာပလာအလံများကို သတိပြုပါ၊ ဂျီသြမေတြီမက်ထရစ်အစီရင်ခံစာက ထွက်လာသည်။
   FFT/LDE/Poseidon GPU အချိန်သတ်မှတ်ချက်များ မရှိတော့သည့်အခါတိုင်း မတည်မငြိမ်ဖြစ်စေမည့် ပုံသဏ္ဍာန်များ။ စောင့်ရှောက်ပါ။
   ဝေဖန်သုံးသပ်သူများ မြင်နိုင်စေရန် Markdown/JSON matrix ကို WP2-E လက်မှတ်များထံ ပူးတွဲခြင်း။
   တန်းစီနေသည့် တယ်လီမီတာကို ရနိုင်သည်နှင့် တစ်ပြိုင်နက် မည်သည့်ပေါင်းစပ်မှုများ မအောင်မြင်သေးပါ။`run_status` အစောင့်နှင့် `--require-telemetry` အလံသည် ယခုဖမ်းယူမှု မအောင်မြင်ပါ။
   GPU အချိန်ဇယားများ ပျောက်ဆုံးနေသည့်အခါတိုင်း သို့မဟုတ် တန်းစီခြင်း/ အဆင့်သတ်မှတ်ခြင်း တယ်လီမီတာ မရှိသည့်အခါတိုင်း၊ ထို့ကြောင့်
   dispatch_count=0 လည်ပတ်မှုများသည် WP2-E အစုအဝေးများသို့ သတိမပြုမိနိုင်တော့ပါ။
   ယခု `fastpq_metal_bench` သည် `--require-gpu` ကို ဖော်ထုတ်ပေးပြီး၊
   `launch_geometry_sweep.py` သည် ၎င်းကို မူရင်းအတိုင်း ဖွင့်ပေးသည် (ဖယ်ထုတ်ထားသည်။
   `--allow-cpu-fallback`) ထို့ကြောင့် CPU အမှားအယွင်းများနှင့် သတ္တုရှာဖွေခြင်း မအောင်မြင်မှုများ ပျက်သွားသည်
   GPU မဟုတ်သော တယ်လီမီတာဖြင့် Stage7 matrices များကို ညစ်ညမ်းစေမည့်အစား ချက်ချင်းပင်။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Zero-fill metrics များသည် ယခင်က အလားတူအကြောင်းပြချက်ကြောင့် ပျောက်ကွယ်သွားခဲ့သည်။ ကာရံပြင်ဆင်ခြင်း။
   လက်ခံကိရိယာတန်ဆာပလာများကို တိုက်ရိုက်ထုတ်လွှင့်နေစေသောကြောင့် နောက်တစ်ကြိမ်ရိုက်ကူးမှုတွင် ပါဝင်သင့်သည်။
   `zero_fill` ပေါင်းစပ်ချိန်ကိုက်ခြင်းမရှိဘဲ ပိတ်ဆို့ခြင်း။

#### `FASTPQ_GPU=gpu` ဖြင့် 20k လျှပ်တစ်ပြက်ရိုက်ချက်

`fastpq_metal_bench_20k_refresh.json`

| ဇာတ်ခုံ | ကော်လံ | ထည့်သွင်းရန် len | GPU ဆိုသည်မှာ (ms) | CPU ဆိုသည်မှာ (ms) | GPU မျှဝေ | အရှိန်မြှင့် | Δ CPU (ms) |
| ---| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79.951ms (65.645–93.193) | 83.289ms (59.956–107.585) | 0.3% | 1.042× | +3.338 |
| IFFT | 16 | 32768 | 78.605ms (69.986–83.726) | 93.898ms (80.656–119.625) | 0.3% | 1.195× | +15.293 |
| LDE | 16 | 262144 | 657.673ms (619.219–712.367) | 669.537ms (619.716–723.285) | 2.1% | 1.018× | +11.864 |
| Poseidon | 16 | 524288 | 30004.898ms (27284.117–32945.253) | 29087.532ms (24969.810–33020.517) | 97.4% | 0.969× | −917.366 |

လေ့လာတွေ့ရှိချက်များ-

1. `FASTPQ_GPU=gpu` ဖြင့်ပင်၊ ဤဖမ်းယူမှုသည် CPU ဆုတ်ယုတ်မှုကို ထင်ဟပ်နေဆဲဖြစ်သည်-
   `metal_dispatch_queue` ဖြင့် ထပ်ခါတလဲလဲနှုန်း ~ 30s သည် သုညတွင် ကပ်နေသည်။ ဟို
   override ကို သတ်မှတ်ထားသော်လည်း လက်ခံသူသည် သတ္တုစက်ပစ္စည်းကို ရှာမတွေ့နိုင်ပါ၊ CLI သည် ယခု ထွက်သွားပြီဖြစ်သည်။
   မည်သည့် kernels ကိုမဆိုမလုပ်ဆောင်မီ တောင်းဆိုထားသော/ဖြေရှင်းထားသောမုဒ် နှင့် ပရင့်ထုတ်ခြင်းမပြုမီ
   အင်ဂျင်နီယာများသည် ထောက်လှမ်းခြင်း၊ ရပိုင်ခွင့်များ သို့မဟုတ် ဖြစ်သည်ရှိမရှိကို အင်ဂျင်နီယာများက ပြောပြနိုင်စေရန်အတွက် backend အညွှန်း
   metallib ရှာဖွေမှုအား အဆင့်နှိမ့်ချစေခဲ့သည်။ `fastpq_metal_bench --gpu-probe ကို ဖွင့်ပါ။
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` စာရင်းကောက်မှတ်တမ်းကို ဖမ်းယူရန်နှင့်
   ပရိုဖိုင်းကို ပြန်မလည်ပတ်မီ အရင်းခံထောက်လှမ်းမှုပြဿနာကို ဖြေရှင်းပါ။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. Zero-fill telemetry သည် အမှန်တကယ်နမူနာ (18.66ms ထက် 32MiB) ကို သက်သေပြနေပါသည်။
   ခြံစည်းရိုး ပြုပြင်ခြင်း အလုပ်ဖြစ်သော်လည်း GPU မပို့မချင်း တန်းစီမြစ်ဝကျွန်းပေါ်များ ပျက်နေပါသည်။
   အောင်မြင်ပါစေ။
3. backend သည် အဆင့်နှိမ့်နေသောကြောင့် Stage7 တယ်လီမီတာတံခါးသည် ငြိမ်နေပါသည်။
   ပိတ်ဆို့ထားသည်- တန်းစီနေသော headroom အထောက်အထားများနှင့် poseidon ထပ်နေသည့်အတွက် စစ်မှန်သော GPU လိုအပ်သည်။
   ပြေး

ဤဖမ်းယူမှုများသည် ယခုအခါ WP2-E ၏နောက်ကွယ်တွင် ရပ်တည်နေပါသည်။ နောက်တစ်ခု လုပ်ဆောင်ချက်များ- ပရိုဖိုင်ကို စုဆောင်းပါ။
မီးပုံဇယားများနှင့် တန်းစီမှတ်တမ်းများ (နောက်ကွယ်မှ GPU ပေါ်တွင် လုပ်ဆောင်သည်နှင့် တပြိုင်နက်)၊
FFT ကို ပြန်လည်မကြည့်ရှုမီ Poseidon/LDE သည် ပိတ်ဆို့မှုများဖြစ်ပြီး နောက်ကွယ်မှ ကျောထောက်နောက်ခံကို ပြန်ဖွင့်ပါ
ထို့ကြောင့် Stage7 telemetry တွင် တကယ့် GPU ဒေတာရှိသည်။

### အားသာချက်များ
- တိုးမြင့်သောအဆင့်၊ ခြေရာကောက်-ပထမဒီဇိုင်း၊ ဖောက်ထွင်းမြင်ရသော STARK စတန်းခ်။### ဦးစားပေး လုပ်ဆောင်ချက် များ
1. ထုပ်ပိုးခြင်း/အမှာစာ ကိရိယာများကို အကောင်အထည်ဖော်ပြီး AIR spec ကို အပ်ဒိတ်လုပ်ပါ။
2. Poseidon2 သည် `3f2b7fe` ကို အပြီးသတ်ပြီး ဥပမာ SMT/lookup vector များကို ထုတ်ဝေပါ။
3. အလုပ်နမူနာများ (`lookup_grand_product.md`၊ `smt_update.md`) ကို ထိန်းသိမ်းပါ။
4. နောက်ဆက်တွဲ A ကို ထည့်သွင်းဖော်ပြပါ ခိုင်လုံသောဆင်းသက်လာခြင်းနှင့် CI ပယ်ချခြင်းနည်းစနစ်ကို ထည့်သွင်းပါ။

### ဖြေရှင်းထားသော ဒီဇိုင်းဆုံးဖြတ်ချက်များ
- P1 တွင် ZK ကိုပိတ်ထားသည် (မှန်ကန်မှု-သာ)၊ အနာဂတ်အဆင့်တွင် ပြန်လည်ကြည့်ရှုပါ။
- အုပ်ချုပ်မှုပြည်နယ်မှ ဆင်းသက်လာသော ခွင့်ပြုချက်ဇယားအမြစ်၊ အစုလိုက်အစည်းများသည် ဇယားကို ဖတ်ရန်သီးသန့်အဖြစ် သတ်မှတ်ပြီး ရှာဖွေမှုမှတစ်ဆင့် အဖွဲ့ဝင်ခြင်းကို သက်သေပြပါ။
- မရှိသောသော့သက်သေအထောက်အထားများသည် သုညအရွက်နှင့် အိမ်နီးချင်းသက်သေကို canonical encoding ဖြင့်အသုံးပြုသည်။
- အဓိပ္ပါယ်ဖွင့်ဆိုချက်များကို ဖျက်ပါ = အရွက်တန်ဖိုးကို canonical keyspace အတွင်း သုညဟု သတ်မှတ်သည်။

ဤစာတမ်းကို ကျမ်းကိုးကိုးကားအဖြစ် အသုံးပြုပါ။ ပျံ့လွင့်မှုကို ရှောင်ရှားရန် ၎င်းကို အရင်းအမြစ်ကုဒ်များ၊ ကိရိယာများနှင့် နောက်ဆက်တွဲများနှင့်အတူ အပ်ဒိတ်လုပ်ပါ။

## နောက်ဆက်တွဲ A— အသံထွက်ခြင်း

ဤနောက်ဆက်တွဲတွင် “Soundness & SLOs” ဇယားကို မည်သို့ထုတ်လုပ်ကြောင်းနှင့် အစောပိုင်းတွင်ဖော်ပြထားသော ≥128-bit ကြမ်းပြင်ကို CI မည်ကဲ့သို့ ကျင့်သုံးကြောင်း ရှင်းပြထားသည်။

### အမှတ်အသား
- `N_trace = 2^k` — ပါဝါနှစ်ခုသို့ စီခွဲပြီး အကွက်များထည့်ပြီးနောက် ခြေရာခံအရှည်။
- `b` — ပေါက်ကွဲမှုအချက် (`N_eval = N_trace × b`)။
- `r` — FRI arity (တရားဝင်သတ်မှတ်မှုများအတွက် 8 သို့မဟုတ် 16)။
- `ℓ` — FRI အရေအတွက် လျှော့ချခြင်း (`layers` ကော်လံ)။
- `q` — အထောက်အထားတစ်ခုအတွက် အတည်ပြုမေးမြန်းချက်များ (`queries` ကော်လံ)။
- `ρ` — ကော်လံစီစဉ်သူမှ အစီရင်ခံတင်ပြထားသော ထိရောက်သောကုဒ်နှုန်း- `ρ = max_i(degree_i / domain_i)` ပထမ FRI အကြိမ်တွင် ရှင်သန်နိုင်သည့် ကန့်သတ်ချက်များအပေါ်။

Goldilocks အခြေခံအကွက်တွင် `|F| = 2^64 - 2^32 + 1` ပါရှိသောကြောင့် Fiat-Shamir တိုက်မိမှုများကို `q / 2^64` ဖြင့် ကန့်သတ်ထားသည်။ ကြိတ်ခြင်းဖြင့် ချိန်ညှိမှုပရိုဖိုင်အတွက် `2^{-g}` နှင့် `g = 23` နှင့် `g = 21` နှင့် ချိန်ညှိမှုပရိုဖိုင်အတွက် `g = 21` ပါဝင်သည်။【crates/fastpq_isi/src/params.rs:65】

### သရုပ်ဖော်နှောင်ထားသည်။

စဉ်ဆက်မပြတ်နှုန်း DEEP-FRI ဖြင့် စာရင်းအင်းဆိုင်ရာ ချို့ယွင်းမှုဖြစ်နိုင်ခြေကို ကျေနပ်စေသည်။

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

အလွှာတစ်ခုစီသည် တူညီသောအချက် `r` ဖြင့် polynomial degree နှင့် domain width ကိုလျှော့ချပေးသောကြောင့် `ρ` ကို အမြဲမပြတ်ထိန်းထားသောကြောင့်ဖြစ်သည်။ ဇယား၏ `est bits` ကော်လံသည် `⌊-log₂ p_fri⌋` အစီရင်ခံသည်။ Fiat-Shamir နှင့် ကြိတ်ခြင်းများသည် အပိုဘေးကင်းသောအနားသတ်များအဖြစ် ဆောင်ရွက်သည်။

### Planner output နှင့် အလုပ်လုပ်ပြီး တွက်ချက်ခြင်း။

ကိုယ်စားလှယ် အစုလိုက် အထွက်နှုန်းများအတွက် Stage1 ကော်လံ အစီအစဉ်ကို လုပ်ဆောင်နေသည်-

| ကန့်သတ်ချက် | `N_trace` | `b` | `N_eval` | `ρ` (စီစဉ်သူ) | ထိရောက်သောဒီဂရီ (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | ---------| ---| --------| ------------- | -------------------------------- | ---| ---| ------------------|
| လက်ကျန် 20k သုတ် | `2^15` | 8 | 262144 | 0.077026 | 20192 | 5 | 52 | 190bits |
| ပမာဏ 65k သုတ် | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132bits |
| Latency 131k သုတ် | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142bits |ဥပမာ (ဟန်ချက်ညီသော 20k အသုတ်)-
1. `N_trace = 2^15`၊ ဒါကြောင့် `N_eval = 2^15 × 8 = 2^18`။
2. စီစဉ်သူ ကိရိယာတန်ဆာပလာ အစီရင်ခံစာသည် `ρ = 0.077026`၊ ထို့ကြောင့် `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`။
3. `-log₂ p_fri = 190 bits`၊ ဇယားထည့်သွင်းချက်နှင့် ကိုက်ညီသည်။
4. Fiat–Shamir တိုက်မိမှုများသည် `2^{-58.3}` အများစုကို ပေါင်းထည့်ကာ ကြိတ်ခြင်း (`g = 23`) သည် နောက်ထပ် `2^{-23}` ကို နုတ်ပြီး စုစုပေါင်း အသံထွက်နှုန်းကို 160bits အထက်တွင် ထိန်းသိမ်းထားသည်။

### CI ငြင်းပယ်ခြင်း-နမူနာကြိုးကြိုး

CI လည်ပတ်မှုတိုင်းသည် ခွဲခြမ်းစိတ်ဖြာမှုနှောင်ကြိုး၏ ±0.6bits အတွင်းတွင် ရှိနေကြောင်း သေချာစေရန် Monte Carlo ကြိုးကြိုးကို လုပ်ဆောင်သည်-
1. Canonical parameter အစုံကိုဆွဲပြီး `TransitionBatch` ကို ကိုက်ညီသောအတန်းအရေအတွက်ဖြင့် ပေါင်းစပ်ပါ။
2. သဲလွန်စကိုတည်ဆောက်ပါ၊ ကျပန်းရွေးချယ်ထားသောကန့်သတ်ချက်ကိုလှန်ပါ (ဥပမာ၊ ရှာဖွေမှုကြီးကျယ်သောထုတ်ကုန် သို့မဟုတ် SMT ပေါက်ဖော်ကိုနှောက်ယှက်ခြင်း) နှင့် သက်သေပြရန်ကြိုးစားပါ။
3. စစ်မှန်ကြောင်းပြန်ဖွင့်ပြီး Fiat-Shamir စိန်ခေါ်မှုများကို ပြန်လည်နမူနာယူပါ (ကြိတ်ခြင်းပါ၀င်သည်) နှင့် ခိုးယူထားသောအထောက်အထားကို ငြင်းပယ်ခြင်းရှိမရှိ မှတ်တမ်းတင်ပါ။
4. ကန့်သတ်ချက်တစ်ခုလျှင် မျိုးစေ့ 16384 အတွက် ပြန်လုပ်ကာ သတိပြုမိသော ငြင်းပယ်မှုနှုန်း၏ အောက်ဘက်ဘောင်၏ 99% Clopper-Pearson ကို bits အဖြစ်သို့ ပြောင်းပါ။

တိုင်းတာထားသော အောက်ပိုင်းဘောင်သည် 128bits အောက်သို့ ချော်သွားပါက အလုပ် ချက်ချင်းပျက်သွားသည်၊ ထို့ကြောင့် အစီအစဉ်ဆွဲသူ၊ ခေါက်ကွင်း သို့မဟုတ် စာသားဝိုင်ယာကြိုးများကို ပေါင်းစည်းခြင်းမပြုမီ ဖမ်းမိပါသည်။

## နောက်ဆက်တွဲ B — Domain-root ဆင်းသက်လာခြင်း

Stage0 သည် Poseidon မှရရှိသော ကိန်းသေများသို့ ခြေရာကောက်နှင့် အကဲဖြတ်သည့် ဂျင်နရေတာများကို တွဲထိုးထားသောကြောင့် အကောင်အထည်ဖော်မှုအားလုံးသည် တူညီသောအုပ်စုခွဲများကို မျှဝေပါသည်။

### လုပ်ထုံးလုပ်နည်း
1. **မျိုးစေ့ရွေးချယ်မှု။** UTF-8 တဂ် `fastpq:v1:domain_roots` ကို FASTPQ တွင် အခြားနေရာများတွင် အသုံးပြုသည့် Poseidon2 ရေမြှုပ်ထဲသို့ UTF-8 တဂ်ကို စုပ်ယူပါ။ ထည့်သွင်းမှုများသည် `pack_bytes` မှ `[len, limbs…]` ကုဒ်နံပါတ်ကို ပြန်သုံးကာ အခြေခံမီးစက် `g_base = 7` ကို ထုတ်ပေးသည်။【crates/fastpq_prover/src/packing.rs:44】【scripts】fastpq/id_gen/bin/rs.
2. **Trace generator.** `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` ကိုတွက်ချက်ပြီး `trace_root^{2^{trace_log_size}} = 1` အား တစ်ဝက်ပါဝါသည် 1 မဟုတ်သော်လည်း စစ်ဆေးပါ။
3. **LDE မီးစက်။** `lde_root` ကိုရရှိရန် `lde_log_size` နှင့် တူညီသော အညွှန်းကိန်းကို ထပ်လုပ်ပါ။
4. ** Coset ရွေးချယ်မှု။** Stage0 သည် အခြေခံအုပ်စုခွဲ (`omega_coset = 1`) ကို အသုံးပြုသည်။ အနာဂတ် coset များသည် `fastpq:v1:domain_roots:coset` ကဲ့သို့သော နောက်ထပ်တက်ဂ်တစ်ခုကို စုပ်ယူနိုင်သည်။
5. **Permutation size.** `permutation_size` ကို ပြတ်သားစွာ ဆက်လက်တည်ရှိနေသောကြောင့် အချိန်ဇယားဆွဲသူများသည် နှစ်ခု၏ သွယ်ဝိုက်သောပါဝါများထံမှ စည်းမျဥ်းများကို ဘယ်သောအခါမှ ကောက်မချပါ။

### မျိုးပွားခြင်းနှင့် အတည်ပြုခြင်း။
- ကိရိယာတန်ဆာပလာ- `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` သည် Rust အတိုအထွာများ သို့မဟုတ် Markdown ဇယား (`--format table`၊ `--seed`၊ `--filter` ကိုကြည့်ပါ)။【scripts/fastpq/src/bin/poseidon】။
- စမ်းသပ်မှုများ- `canonical_sets_meet_security_target` သည် ထုတ်ဝေထားသော ကိန်းသေများ (သုညမဟုတ်သော ရင်းမြစ်များ၊ မှုတ်ထုတ်ခြင်း/arity တွဲချိတ်ခြင်း၊ ပြောင်းလဲခြင်း အရွယ်အစား)၊ ထို့ကြောင့် `cargo test -p fastpq_isi` သည် ထုတ်ဝေထားသော ကိန်းသေများနှင့် လိုက်လျောညီထွေဖြစ်အောင် ထိန်းပေးသည်။【crates/fastpq_isi/:13/params】
- အမှန်တရား၏အရင်းအမြစ်- ပါရာမီတာ ပက်ခ်များကို မိတ်ဆက်သည့်အခါတိုင်း Stage0 ဇယားနှင့် `fastpq_isi/src/params.rs` ကို အတူတကွ အပ်ဒိတ်လုပ်ပါ။

## နောက်ဆက်တွဲ C — ကတိကဝတ် ပိုက်လိုင်းအသေးစိတ်### လွှ Poseidon တစိုက်မတ်မတ် စီးဆင်းခြင်း။
Stage2 သည် သက်သေနှင့် အတည်ပြုသူမှ မျှဝေထားသော အဆုံးအဖြတ်ပေးသည့် ခြေရာကောက်ကြောင်း ကတိကဝတ်ကို သတ်မှတ်သည်-
1. **ပုံမှန်အကူးအပြောင်းများ။** `trace::build_trace` သည် အသုတ်တစ်ခုစီကို စီထားကာ ၎င်းကို `N_trace = 2^{⌈log₂ rows⌉}` သို့ pads လုပ်ပြီး အောက်ဖော်ပြပါအတိုင်း ကော်လံ vector များကို ထုတ်လွှတ်ပါသည်။【crates/fastpq_prover/src/trace.rs:123】
2. **Hash ကော်လံများ** `trace::column_hashes` သည် `fastpq:v1:trace:column:<name>` တွင်ပါသော သီးခြား Poseidon2 ရေမြှုပ်များမှတစ်ဆင့် ကော်လံများကို တိုက်ရိုက်ထုတ်လွှင့်သည်။ `fastpq-prover-preview` အင်္ဂါရပ်သည် အသက်ဝင်သောအခါ တူညီသောဖြတ်သန်းမှုသည် နောက်ကွယ်မှလိုအပ်သော IFFT/LDE ကိန်းဂဏန်းများကို ပြန်လည်အသုံးပြုသည်၊ ထို့ကြောင့် အပိုမက်ထရစ်မိတ္တူများကို ခွဲဝေပေးမည်မဟုတ်ပါ။【crates/fastpq_prover/src/trace.rs:474】
3. ** Merkle သစ်ပင်တစ်ပင်သို့ ကြွပါ။** `trace::merkle_root` သည် `fastpq:v1:trace:node` ပါသော `fastpq:v1:trace:node` ပါသော Poseidon node များဖြင့် ကော်လံအချေများကို ခေါက်ကာ အထူးကိစ္စများရှောင်ရှားရန် အထူးကိစ္စများရှောင်ရှားရန် ထူးထူးခြားခြား ပန်ကာထွက်သည့်အခါတိုင်း နောက်ဆုံးအရွက်ကို ပွားခြင်း။【crates/fastpq_prover/:rs6/trace】
4. **အချေအတင်ကို အပြီးသတ်ပါ။** `digest::trace_commitment` သည် ဒိုမိန်းတက်ဂ် (`fastpq:v1:trace_commitment`)၊ ပါရာမီတာအမည်၊ ဘောင်ခတ်ထားသော အတိုင်းအတာ၊ ကော်လံအချေအတင်များနှင့် Merkle root သည် တူညီသော `[len, limbs…]` ကုဒ်နံပါတ်ကို အသုံးပြု၍ ဒိုမိန်းတဂ် (`fastpq:v1:trace_commitment`) ဖြင့် ရှေ့နောက်တွင် ပါရာမီတာ 25 ကို S. `Proof::trace_commitment`။ 【crates/fastpq_prover/src/digest.rs:25】

Fiat-Shamir စိန်ခေါ်မှုများကိုနမူနာမကောက်ယူမီ တူညီသောအချေအတင်ကို ပြန်လည်တွက်ချက်ပေးသည်၊ ထို့ကြောင့် မကိုက်ညီမှုများသည် အဖွင့်များမစတင်မီ အထောက်အထားများကို ပျက်ပြယ်စေပါသည်။

### Poseidon fallback ထိန်းချုပ်မှုများ- ယခုသက်သေပြချက်သည် ရည်မှန်းထားသော Poseidon ပိုက်လိုင်းကို အစားထိုးခြင်း (`zk.fastpq.poseidon_mode`၊ env `FASTPQ_POSEIDON_MODE`၊ CLI `--fastpq-poseidon-mode`) ကို ဖော်ထုတ်ပြသပေးသောကြောင့် အော်ပရေတာများသည် CPU Poseidon hashing ကို မအောင်မြင်သော စက်များတွင် GPU FFT/LDE နှင့် ရောနှောနိုင်သည်။ ပံ့ပိုးထားသောတန်ဖိုးများသည် သတ်မှတ်မုဒ်မှသတ်မှတ်မထားသောအခါ ကမ္ဘာလုံးဆိုင်ရာမုဒ်သို့ ပုံသေဖြစ်စေသော ပံ့ပိုးပေးထားသောတန်ဖိုးများ (`auto`၊ `cpu`၊ `gpu`) ကို ထင်ဟပ်စေသည်။ runtime thread သည် ဤတန်ဖိုးကို lane config (`FastpqPoseidonMode`) မှတဆင့် ပေးပို့ပြီး prover (`Prover::canonical_with_modes`) သို့ ပြန့်ပွားသွားသောကြောင့် overrides များသည် အဆုံးအဖြတ်ပေးပြီး config တွင် စစ်ဆေးနိုင်သည် အမှိုက်ပုံများ။ 【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- Telemetry သည် ဖြေရှင်းပြီးသား ပိုက်လိုင်းမုဒ်ကို `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` ကောင်တာအသစ် (နှင့် OTLP အမွှာ `fastpq.poseidon_pipeline_resolutions_total`) မှတဆင့် တင်ပို့သည်။ ထို့ကြောင့် `sorafs`/အော်ပရေတာ ဒက်ရှ်ဘုတ်များသည် GPU ပေါင်းစပ်ထားသော/ပိုက်လိုင်းဖြင့် ဟတ်ခြင်းလုပ်ဆောင်နေချိန်တွင် အတင်းအကြပ် CPU ဆုတ်ယုတ်ခြင်း (`path="cpu_forced"`) သို့မဟုတ် runtime အဆင့်နှိမ့်ချခြင်း (`path="cpu_fallback"`) ကို လုပ်ဆောင်နေချိန်တွင် အတည်ပြုနိုင်သည်။ CLI probe သည် `irohad` တွင် အလိုအလျောက်ထည့်သွင်းပေးသည်၊ ထို့ကြောင့် အစုအဝေးများကို ထုတ်လွှတ်ပြီး တိုက်ရိုက် တယ်လီမီတာမှ တူညီသောအထောက်အထားစီးကြောင်းကို မျှဝေပါသည်။ 【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- ရောနှော-မုဒ် အထောက်အထားများကို လက်ရှိမွေးစားခြင်းဂိတ်မှတစ်ဆင့် အမှတ်အသားပြုသည့် ဂိတ်တိုင်းတွင် တံဆိပ်ခတ်ထားပါသည်- သက်သေသည် အသုတ်တစ်ခုစီအတွက် ဖြေရှင်းထားသောမုဒ် + လမ်းကြောင်းတံဆိပ်ကို ထုတ်လွှတ်ကာ `fastpq_poseidon_pipeline_total` ကောင်တာသည် အထောက်အထားတစ်ခုရောက်တိုင်း execution-mode ကောင်တာဘေးတွင် တိုးလာပါသည်။ ၎င်းသည် WP2-E.6 ကို မြင်သာအောင်ပြုလုပ်ကာ ပိုမိုကောင်းမွန်အောင်လုပ်ဆောင်နေချိန်တွင် အဆုံးအဖြတ်အဆင့်နှိမ့်ချမှုအတွက် သန့်ရှင်းသောခလုတ်ကို ပံ့ပိုးပေးခြင်းဖြင့် ကျေနပ်စေသည်။ 【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- ယခု `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` သည် Prometheus ခြစ်ရာများ (သတ္တု သို့မဟုတ် CUDA) ကို ပိုင်းခြားပြီး `poseidon_metrics` အကျဉ်းချုပ်ကို ထုပ်ပိုးထားသည့် အစုအဝေးတိုင်းအတွင်းတွင် ထည့်သွင်းထားသည်။ အကူအညီပေးသူက `metadata.labels.device_class` ဖြင့် ကောင်တာအတန်းများကို စစ်ထုတ်ပြီး ကိုက်ညီသော `fastpq_execution_mode_total` နမူနာများကို ဖမ်းယူကာ `fastpq_poseidon_pipeline_total` တွင် ထည့်သွင်းမှုများ ပျောက်ဆုံးနေချိန်တွင် WP2-E.6 အစုအဝေးများကို ကြော်ငြာပြန်ထုတ်နိုင်သော CUDA/Metal အထောက်အထားများအစား အမြဲပို့ပေးပါသည်။ မှတ်စုများ။【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Deterministic mixed-mode policy (WP2-E.6)1. **GPU ပြတ်တောက်မှုကို ထောက်လှမ်းပါ။** Stage7 ဖမ်းယူခြင်း သို့မဟုတ် တိုက်ရိုက်လွှင့်သော Grafana လျှပ်တစ်ပြက်ရိုက်ချက်သည် Poseidon latency ကိုပြသပြီး စုစုပေါင်း သက်သေပြချိန်> 900ms ရှိသော်လည်း FFT/LDE သည် ပစ်မှတ်အောက်၌ ရှိနေစဉ်တွင် မည်သည့်စက်ပစ္စည်းအမျိုးအစားကိုမဆို အလံပြပါ။ အော်ပရေတာများသည် ဖမ်းယူမှုမက်ထရစ် (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) ကို မှတ်သားပြီး `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` ရပ်တန့်သွားချိန်တွင် `fastpq_execution_mode_total{backend="metal"}` သည် GPU FFT/LDE ကို ဆက်လက်မှတ်တမ်းတင်နေချိန်တွင် အော်ပရေတာများက ခေါ်ဆိုမှုစာမျက်နှာကို စာမျက်နှာပေါ်တွင် ရေးတင်ထားသည်။ ပေးပို့မှုများ။【scripts/fastpq/wrap_benchmark.py:1】【 dashboards/grafana/fastpq_acceleration.json:1】
2. **CPU Poseidon သို့ ပြန်လှန်၍ သက်ရောက်မှုရှိသော hosts များအတွက်သာ။** `zk.fastpq.poseidon_mode = "cpu"` (သို့မဟုတ် `FASTPQ_POSEIDON_MODE=cpu`) ကို ရေတပ်အညွှန်းများနှင့်အတူ လက်ခံဆောင်ရွက်ပေးသော ဒေသဖွဲ့စည်းမှုတွင် `zk.fastpq.execution_mode = "gpu"` ကိုထားရှိခြင်းဖြင့် accelerator ကို ဆက်လက်အသုံးပြုပါ။ စတင်ရောင်းချခြင်းလက်မှတ်တွင် config ကွဲပြားမှုကို မှတ်တမ်းတင်ပြီး `poseidon_fallback.patch` အဖြစ် အစုအစည်းတွင် တစ်ခုချင်းပြန်လက်ခံသည့်အရာအား ထပ်လောင်းထည့်ခြင်းဖြင့် ပြန်လည်သုံးသပ်သူများသည် အပြောင်းအလဲကို တိကျပြတ်သားစွာ ပြန်ဖွင့်နိုင်ပါသည်။
3. ** အဆင့်နှိမ့်မှုကို သက်သေပြပါ။** node ကို ပြန်လည်စတင်ပြီးနောက် Poseidon ကောင်တာကို ချက်ချင်းခြစ်ပါ။
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   အမှိုက်ပုံသည် `path="cpu_forced"` ကို GPU လုပ်ဆောင်ချက်ကောင်တာဖြင့် လော့ခ်ချသည့်အဆင့်တွင် ကြီးထွားလာကြောင်း ပြသရပါမည်။ ရှိပြီးသား `metrics_cpu_fallback.prom` လျှပ်တစ်ပြက်ရိုက်ချက်ဘေးတွင် `metrics_poseidon.prom` အဖြစ် သိမ်းဆည်းပြီး လိုက်ဖက်သော `telemetry::fastpq.poseidon` မှတ်တမ်းလိုင်းများကို `poseidon_fallback.log` တွင် ရိုက်ကူးပါ။
4. **Monitor & exit.** optimization အလုပ်ကို ဆက်လက်လုပ်ဆောင်နေချိန်တွင် `fastpq_poseidon_pipeline_total{path="cpu_forced"}` တွင် သတိပေးချက် ဆက်လက်ထားရှိပါ။ ဖာထေးမှုတစ်ခုသည် စမ်းသပ်မှုတွင် 900ms အောက်တွင်ရှိနေသည်နှင့်တစ်ပြိုင်နက် config ကို `auto` သို့ပြန်လှည့်ကာ ခြစ်ခြင်းကိုပြန်လည်လုပ်ဆောင်ပါ (`path="gpu"` ကိုထပ်မံပြသသည်) နှင့် ပေါင်းစပ်မုဒ်အစမ်းလေ့ကျင့်မှုကိုပိတ်ရန် အတွဲတွင် ရှေ့/နောက် မက်ထရစ်များကို ပူးတွဲပါ။

** တယ်လီမီတာ စာချုပ်။**

| အချက်ပြ | PromQL / Source | ရည်ရွယ်ချက် |
|--------|-----------------|---------|
| Poseidon မုဒ်ကောင်တာ | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | CPU hashing သည် ရည်ရွယ်ချက်ရှိရှိနှင့် အလံပြထားသော စက်ပစ္စည်းအတန်းအစားသို့ ကန့်သတ်ထားကြောင်း အတည်ပြုသည်။ |
| စီရင်ချက်မုဒ် ကောင်တာ | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Poseidon အဆင့်နှိမ့်ချနေချိန်တွင်ပင် FFT/LDE သည် GPU တွင် ဆက်လက်လည်ပတ်နေသေးကြောင်း သက်သေပြသည်။ |
| မှတ်တမ်းအထောက်အထား | `telemetry::fastpq.poseidon` ဖိုင်များကို `poseidon_fallback.log` တွင် ရိုက်ကူးထားသည် | အကြောင်းပြချက် `cpu_forced` ဖြင့် CPU hashing ကို လက်ခံဆောင်ရွက်ပေးသူမှ ဖြေရှင်းခဲ့သော အထောက်အထားတစ်ခု ပေးသည်။ |

ဖြန့်ချိသည့်အစုအဝေးတွင် ယခု `metrics_poseidon.prom`၊ config diff နှင့် ရောစပ်မုဒ်တက်ကြွနေသည့်အခါတိုင်း မှတ်တမ်းကောက်နုတ်ချက် ပါဝင်ရမည်ဖြစ်ရာ အုပ်ချုပ်ရေးသည် FFT/LDE တယ်လီမက်ထရီနှင့်အတူ အဆုံးအဖြတ်ပေးသော ဆုတ်ယုတ်မှုမူဝါဒကို စစ်ဆေးနိုင်သည်။ `ci/check_fastpq_rollout.sh` သည် တန်းစီခြင်း/သုည-ဖြည့်စွက်ကန့်သတ်ချက်များကို ပြဌာန်းထားပြီးဖြစ်သည်။ နောက်ဆက်တွဲဂိတ်သည် ရောစပ်ထားသော မုဒ်ကို လွှတ်တင်လိုက်သည်နှင့် အလိုအလျောက်စနစ်ဖြင့် Poseidon ကောင်တာကို စစ်ဆေးမည်ဖြစ်သည်။

Stage7 ဖမ်းယူသည့်ကိရိယာသည် CUDA ကိုကိုင်တွယ်ပြီးဖြစ်သည်- `fastpq_cuda_bench` အစုအဝေးတိုင်းကို `--poseidon-metrics` (ခြစ်ထုတ်ထားသော `metrics_poseidon.prom` ကိုညွှန်ပြသည်) နှင့် ထုပ်ပိုးထားပြီး ယခုထွက်ရှိမှုသည် CU တွင်အသုံးပြုသော တူညီသောပိုက်လိုင်းကောင်တာများ/ဖြေရှင်းချက်အကျဉ်းချုပ်ဖြစ်သောကြောင့် CU တွင်အသုံးပြုသော စည်းမျဥ်းစည်းကမ်းအတိုင်း အုပ်ချုပ်မှုမပြုလုပ်ဘဲ တူညီပါသည်။ ကိရိယာတန်ဆာပလာ။【scripts/fastpq/wrap_benchmark.py:1】### ကော်လံအော်ဒါ
hashing ပိုက်လိုင်းသည် ဤသတ်မှတ်စနစ်တွင် ကော်လံများကို စားသုံးသည်-
1. ရွေးချယ်မှုအလံများ- `s_active`၊ `s_transfer`၊ `s_mint`၊ `s_burn`၊ `s_role_grant`၊ `s_role_revoke`၊ Prometheus `s_perm`။
2. ထုပ်ပိုးထားသော ကိုယ်လက်အင်္ဂါကော်လံများ (အစအနအလျားသို့ သုည-အဖုံးတစ်ခုစီ)- `key_limb_{i}`၊ `value_old_limb_{i}`၊ `value_new_limb_{i}`၊ `asset_id_limb_{i}`။
3. အရန်ကြေးခွံများ- `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, Prometheus,018X `slot`။
4. အဆင့်တိုင်းအတွက် Sparse Merkle သက်သေ `ℓ ∈ [0, SMT_HEIGHT)`- `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`။

`trace::column_hashes` သည် ကော်လံများကို ဤအစီအစဥ်အတိုင်း အတိအကျ လမ်းလျှောက်ပေးသည်၊ ထို့ကြောင့် နေရာယူထားသော နောက်ခံအစွန်နှင့် Stage2 STARK အကောင်အထည်ဖော်မှုသည် ဖြန့်ချိမှုများတစ်လျှောက် ခြေရာခံတည်ငြိမ်နေပါသည်။【crates/fastpq_prover/src/trace.rs:474】

### စာသားမှတ်တမ်း ဒိုမိန်း တဂ်များ
Stage2 သည် စိန်ခေါ်မှုမျိုးဆက်ကို အဆုံးအဖြတ်ပေးနိုင်ရန် အောက်ပါ Fiat–Shamir ကတ်တလောက်ကို ပြင်ဆင်သည်-

| Tag | ရည်ရွယ်ချက် |
| ---| -------|
| `fastpq:v1:init` | ပရိုတိုကောဗားရှင်း၊ ကန့်သတ်ချက်နှင့် `PublicIO` ကို စုပ်ယူပါ။ |
| `fastpq:v1:roots` | ခြေရာကောက်ပြီး Merkle အမြစ်များကို ရှာဖွေပါ။ |
| `fastpq:v1:gamma` | ရှာဖွေမှုကြီးကျယ်သော ထုတ်ကုန်စိန်ခေါ်မှုကို နမူနာယူပါ။ |
| `fastpq:v1:alpha:<i>` | နမူနာဖွဲ့စည်းမှု- polynomial စိန်ခေါ်မှုများ (`i = 0, 1`)။ |
| `fastpq:v1:lookup:product` | အကဲဖြတ်ရှာဖွေမှု ခမ်းနားသော ထုတ်ကုန်ကို စုပ်ယူပါ။ |
| `fastpq:v1:beta:<round>` | FRI အဝိုင်းတစ်ခုစီအတွက် ခေါက်စိန်ခေါ်မှုကို နမူနာယူပါ။ |
| `fastpq:v1:fri_layer:<round>` | FRI အလွှာတစ်ခုစီအတွက် Merkle အမြစ်ကို ထည့်သွင်းပါ။ |
| `fastpq:v1:fri:final` | မေးခွန်းများမဖွင့်မီ နောက်ဆုံး FRI အလွှာကို မှတ်တမ်းတင်ပါ။ |
| `fastpq:v1:query_index:0` | စစ်မှန်သော စစ်ဆေးမေးမြန်းမှု အညွှန်းကိန်းများကို အဆုံးအဖြတ်ရယူသည်။ |