---
lang: my
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T12:24:34.985909+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% FastPQ Transfer Gadget ဒီဇိုင်း

# ခြုံငုံသုံးသပ်ချက်

လက်ရှိ FASTPQ အစီအစဉ်ရေးဆွဲသူသည် `TransferAsset` ညွှန်ကြားချက်တွင် ပါ၀င်သည့် ပဏာမလုပ်ဆောင်မှုတိုင်းကို မှတ်တမ်းတင်ထားပြီး၊ ဆိုလိုသည်မှာ လွှဲပြောင်းမှုတစ်ခုစီသည် လက်ကျန်ဂဏန်းသင်္ချာ၊ hash rounds နှင့် SMT အပ်ဒိတ်များအတွက် သီးခြားစီပေးဆောင်သည်။ လွှဲပြောင်းမှုတစ်ခုလျှင် ခြေရာခံတန်းစီများကို လျှော့ချရန်အတွက် လက်ခံသူသည် ကာနိုနစ်ဆိုင်ရာ အခြေအနေအကူးအပြောင်းကို ဆက်လက်လုပ်ဆောင်နေချိန်တွင် အနိမ့်ဆုံးဂဏန်းသင်္ချာ/ကတိကဝတ်စစ်ဆေးမှုများကိုသာ စစ်ဆေးသည့် သီးခြားဂက်ဂျက်တစ်ခုကို မိတ်ဆက်ပေးထားပါသည်။

- **Scope**- လက်ရှိ Kotodama/IVM `TransferAsset` syscall မျက်နှာပြင်မှတဆင့် ထုတ်လွှတ်သော တစ်ခုတည်းသော လွှဲပြောင်းမှုများနှင့် အသေးစားအတွဲများ။
- **ပန်းတိုင်**- ရှာဖွေမှုဇယားများကို မျှဝေပြီး တစ်ခုချင်းလွှဲပြောင်းသည့်ဂဏန်းသင်္ချာကို ကျစ်လစ်သိပ်သည်းစွာ ကန့်သတ်ပိတ်ဆို့ထားသော ဘလောက်တစ်ခုအဖြစ် ပြိုကျစေခြင်းဖြင့် ပမာဏမြင့်မားသောလွှဲပြောင်းမှုများအတွက် FFT/LDE ကော်လံခြေရာကို ဖြတ်လိုက်ပါ။

#ဗိသုကာ

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## စာသားမှတ်တမ်းပုံစံ

လက်ခံသူသည် syscall ခေါ်ဆိုမှုတစ်ခုလျှင် `TransferTranscript` ကို ထုတ်လွှတ်သည်-

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` သည် ပြန်လည်ကစားခြင်းကို ကာကွယ်ရန်အတွက် ငွေပေးငွေယူ entrypoint hash နှင့် transcript ကို ချိတ်ဆက်ထားသည်။
- `authority_digest` သည် အမျိုးအစားခွဲထားသော လက်မှတ်ထိုးသူများ/အထမြောက်ခြင်းဒေတာအတွက် လက်ခံဆောင်ရွက်ပေးသူ၏ hash ဖြစ်သည် gadget သည် သာတူညီမျှမှုကို စစ်ဆေးသော်လည်း လက်မှတ်အတည်ပြုခြင်းကို ထပ်မလုပ်ပါ။ လက်ခံဆောင်ရွက်ပေးသူ Norito သည် `AccountId` (canonical multisig controller ကိုထည့်သွင်းထားပြီးဖြစ်သည်) နှင့် `b"iroha:fastpq:v1:authority|" || encoded_account` ကို Blake2b-256 ဖြင့် ဆိုင်းငံ့ထားပြီး ရလဒ် I180NI0100 ကို သိမ်းဆည်းထားသည်။
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || ပိုင်ဆိုင်မှု || ပမာဏ || batch_hash); gadget သည် host နှင့်တူညီသော digest ကိုပြန်လည်တွက်ချက်ကြောင်းသေချာစေသည်။ preimage bytes များကို မျှဝေထားသော Poseidon2 helper မှတဆင့် ၎င်းတို့ကို မဖြတ်သန်းမီ Norito ဗလာ Norito ကုဒ်နံပါတ်ကို အသုံးပြု၍ `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` အဖြစ် တည်ဆောက်ထားပါသည်။ ဤဒြပ်စင်သည် မြစ်ဝကျွန်းပေါ်ဒေသတစ်ခုတည်းအတွက် တည်ရှိပြီး မြစ်ဝကျွန်းပေါ်ဒေသအစုံအလင်အတွက် ချန်လှပ်ထားသည်။

အကွက်အားလုံးကို Norito မှတစ်ဆင့် အမှတ်စဉ်ပြုလုပ်ထားသောကြောင့် ရှိပြီးသား အဆုံးအဖြတ်ပေးခြင်းဆိုင်ရာ အာမခံချက်များကို ထိန်းထားနိုင်သည်။
`from_path` နှင့် `to_path` နှစ်ခုလုံးကို Norito blobs အဖြစ် ထုတ်လွှတ်သည်။
`TransferMerkleProofV1` အစီအစဉ်- `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`။
Prover သည် ဗားရှင်းတဂ်အား ပြဋ္ဌာန်းထားစဉ် အနာဂတ်ဗားရှင်းများသည် schema ကို တိုးချဲ့နိုင်သည်။
decoding မတိုင်ခင်။ `TransitionBatch` မက်တာဒေတာသည် Norito-ကုဒ်ဝှက်ထားသော စာသားမှတ်တမ်းကို မြှုပ်နှံထားသည်
`transfer_transcripts` သော့အောက်ရှိ vector သည် သက်သေကို ကုဒ်လုပ်နိုင်သည်။
ကွင်းပြင်ပမေးခွန်းများကို မလုပ်ဆောင်ဘဲ၊ အများသူငှာ သွင်းအားစုများ (`dsid`၊ `slot`၊ အမြစ်များ၊
`perm_root`, `tx_set_hash`) တို့သည် `FastpqTransitionBatch.public_inputs`၊
စာရင်းသွင်းမှု hash/transcript count အတွက် metadata ကိုချန်ထားပါ။ အိမ်ရှင် ပိုက်ဆက်သည်အထိ
မြေများ၊ သုသူသည် သော့/လက်ကျန်အတွဲများမှ သက်သေများကို ပေါင်းစပ်၍ အတန်းလိုက်၊
စာသားမှတ်တမ်းသည် ရွေးချယ်နိုင်သောအကွက်များကို ချန်လှပ်ထားသော်လည်း အဆုံးအဖြတ်ပေးသော SMT လမ်းကြောင်းကို အမြဲထည့်သွင်းပါ။

## Gadget အပြင်အဆင်

1. **လက်ကျန်ဂဏန်းသင်္ချာ ပိတ်ဆို့ခြင်း**
   - ထည့်သွင်းမှုများ- `from_balance_before`၊ `amount`၊ `to_balance_before`။
   - စစ်ဆေးမှုများ
     - `from_balance_before >= amount` (မျှဝေထားသော RNS ပြိုကွဲခြင်းနှင့်အတူ အပိုင်းအခြား gadget)။
     - `from_balance_after = from_balance_before - amount`။
     - `to_balance_after = to_balance_before + amount`။
   - စိတ်ကြိုက်တံခါးတစ်ခုတွင် ထည့်သွင်းထားသောကြောင့် ညီမျှခြင်းသုံးခုစလုံးသည် အတန်းအုပ်စုတစ်စုကို စားသုံးသည်။2. **Poseidon ကတိကဝတ် Block**
   - အခြား gadget များတွင် အသုံးပြုထားပြီးဖြစ်သော Poseidon ရှာဖွေမှုဇယားကို အသုံးပြု၍ `poseidon_preimage_digest` ကို ပြန်လည်တွက်ချက်သည်။ သဲလွန်စတွင် တစ်ကြိမ် လွှဲပြောင်းခြင်း Poseidon လှည့်ခြင်း မရှိပါ။

3. ** Merkle Path Block**
   - ရှိပြီးသား Kaigi SMT gadget ကို "တွဲထားသောအပ်ဒိတ်" မုဒ်ဖြင့် တိုးချဲ့ပါ။ အရွက်နှစ်ရွက် (ပေးပို့သူ၊ လက်ခံသူ) သည် ပွားနေသော အတန်းများကို လျှော့ချပြီး တူညီသော ကော်လံကို မျှဝေပါသည်။

4. ** အာဏာပိုင် ဒိုင်ဂျစ် စစ်ဆေးမှု**
   - အိမ်ရှင်မှပံ့ပိုးပေးထားသော အချေအတင်နှင့် သက်သေတန်ဖိုးကြား ရိုးရှင်းသော တန်းတူညီမျှမှု ကန့်သတ်ချက်။ လက်မှတ်များသည် ၎င်းတို့၏ သီးသန့် gadget တွင် ရှိနေပါသည်။

5. **Batch Loop**
   - ပရိုဂရမ်များသည် `transfer_asset` တည်ဆောက်သူများနှင့် နောက်မှ `transfer_v1_batch_end()` တို့ကို ကွင်းဆက်မတိုင်မီ `transfer_v1_batch_begin()` သို့ခေါ်ဆိုပါ။ နယ်ပယ်သည် တက်ကြွနေချိန်တွင် လက်ခံဆောင်ရွက်ပေးသူသည် လွှဲပြောင်းမှုတစ်ခုစီကို ကြားခံလုပ်ဆောင်ပြီး ၎င်းတို့ကို `TransferAssetBatch` တစ်ခုတည်းအဖြစ် Poseidon/SMT ဆက်စပ်မှုကို တစ်သုတ်လျှင် တစ်ကြိမ် ပြန်လည်အသုံးပြုသည်။ နောက်ထပ်မြစ်ဝကျွန်းပေါ်ဒေသတစ်ခုစီသည် ဂဏန်းသင်္ချာနှင့် အရွက်စစ်ဆေးမှုနှစ်ခုကိုသာ ပေါင်းထည့်သည်။ စာသားမှတ်တမ်း ဒီကုဒ်ဒါသည် ယခုအခါ မြစ်ဝကျွန်းပေါ်ဒေသအစုံအလင်ကို လက်ခံပြီး ၎င်းတို့ကို `TransferGadgetInput::deltas` အဖြစ် ပေါ်လွင်စေသောကြောင့် စီစဉ်သူသည် Norito ကို ပြန်လည်မဖတ်ဘဲ သက်သေများကို ခေါက်နိုင်သည်။ Norito ပေးဆောင်ရန် အဆင်ပြေသော စာချုပ်များ (ဥပမာ၊ CLI/SDKs) များသည် `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` ကိုခေါ်ဆိုခြင်းဖြင့်၊ syscall တစ်ခုတွင် အပြည့်အဝ encoded batch ကို လက်ခံဆောင်ရွက်ပေးသူအား ပေးအပ်သည့် `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` ကိုခေါ်ဆိုခြင်းဖြင့် နယ်ပယ်တစ်ခုလုံးကို ကျော်သွားနိုင်ပါသည်။

# လက်ခံသူနှင့် သက်သေ အပြောင်းအလဲများ| အလွှာ | အပြောင်းအလဲများ |
|---------|---------|
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) သို့ထည့်ပါ ထို့ကြောင့် ပရိုဂရမ်များသည် `transfer_v1` အလယ်အလတ် ISIs၊ 604X နှင့် ISI6040 တို့ကို အလယ်အလတ် ISIs များ ထုတ်လွှတ်ခြင်းမရှိဘဲ ကွင်းပိတ်ပြုလုပ်နိုင်သည်။ ကြိုတင်ကုဒ်လုပ်ထားသော အတွဲများအတွက် (`0x2B`)။ |
| `ivm::host` & စမ်းသပ်မှုများ | Core/Default host များသည် `transfer_v1` ကို scope တက်ကြွနေချိန်တွင်၊ မျက်နှာပြင် `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` နှင့် mock WSV host buffers များသည် buffers entry များကို မလုပ်ဆောင်မီ regression tests သည် အဆုံးအဖြတ်လက်ကျန်ကို အာမခံနိုင်သည် အပ်ဒိတ်များ။【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | ပြည်နယ်အကူးအပြောင်းပြီးနောက် `TransferTranscript` ကိုထုတ်ပါ၊ `FastpqTransitionBatch` မှတ်တမ်းများကို `StateBlock::capture_exec_witness` ကာလအတွင်း တိကျပြတ်သားသော `public_inputs` ဖြင့် `StateBlock::capture_exec_witness` ဖြင့် တည်ဆောက်ပြီး FASTPQ သက်သေပြလမ်းကို လည်ပတ်စေခြင်းဖြင့် Torii နှင့် Stage နှစ်ခုစလုံးကို backend လုပ်နိုင်သည် `TransitionBatch` သွင်းအားစုများ `TransferAssetBatch` သည် ဆက်တိုက်လွှဲပြောင်းမှုများကို တစ်ခုတည်းသောမှတ်တမ်းအဖြစ် အုပ်စုဖွဲ့ကာ Multi-delta အတွဲများအတွက် poseidon digest ကို ချန်လှပ်ထားသောကြောင့် gadget သည် entries များကို တိကျပြတ်သားစွာ ထပ်တလဲလဲပြုလုပ်နိုင်ပါသည်။ |
| `fastpq_prover` | ယခုအခါ `gadgets::transfer` သည် မြစ်ဝကျွန်းပေါ်သား အများအပြား၏ စာသားမှတ်တမ်းများ (လက်ကျန်ဂဏန်းသင်္ချာ + Poseidon ချေဖျက်မှု) ကို အတည်ပြုပြီး အစီအစဉ်ရေးဆွဲသူ (`crates/fastpq_prover/src/gadgets/transfer.rs`) အတွက် ဖွဲ့စည်းထားသော သက်သေများ (နေရာယူထားသည့် SMT blobs များအပါအဝင်) ကို မျက်နှာပြင်များခင်းပေးပါသည်။ `trace::build_trace` သည် အဆိုပါ စာသားများကို အသုတ် metadata မှ ကုဒ်ဖျက်သည်၊ `transfer_transcripts` payload ပျောက်ဆုံးနေသော လွှဲပြောင်းမှုအသုတ်များကို ငြင်းပယ်သည်၊ တရားဝင်သက်သေများကို `Trace::transfer_witnesses` သို့ ပူးတွဲပေးကာ `TracePolynomialData::transfer_plan()` သည် အစီအစဉ်ကို သက်တမ်းပြည့်သည်အထိ ဆက်လက်လုပ်ဆောင်သွားမည်ဖြစ်သည်။ (`crates/fastpq_prover/src/trace.rs`)။ အတန်းရေတွက်မှုဆုတ်ယုတ်မှုကြိုးသည် ယခု `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) မှတစ်ဆင့် ပေးပို့သည်) သည် 65536 တန်းအထိ padded အတန်းအထိ အကျုံးဝင်ပြီး တွဲချိတ်ထားသော SMT ဝိုင်ယာကြိုးများသည် TF-3 အသုတ်-အထောက်ကူပြုမှတ်တိုင်နောက်ကွယ်တွင် ကျန်ရှိနေချိန်တွင် (နေရာချထားသူများ) သဲလွန်စများအထိ တည်ငြိမ်နေပါသည်။ |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` အကူအညီပေးသူကို `transfer_v1_batch_begin`၊ ဆက်တိုက် `transfer_asset` ခေါ်ဆိုမှုများနှင့် `transfer_v1_batch_end` သို့ နှိမ့်ချပါ။ tuple အငြင်းအခုံတစ်ခုစီသည် `(AccountId, AccountId, AssetDefinitionId, int)` ပုံသဏ္ဍာန်အတိုင်းဖြစ်ရမည်။ တစ်ခုတည်းသော လွှဲပြောင်းမှုများသည် ရှိပြီးသား တည်ဆောက်သူကို ထိန်းသိမ်းထားသည်။ |

ဥပမာ Kotodama အသုံးပြုမှု-

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` သည် `Transfer::asset_numeric` တစ်ဦးချင်းစီ၏ ခွင့်ပြုချက်နှင့် ဂဏန်းသင်္ချာစစ်ဆေးမှုများကို လုပ်ဆောင်သော်လည်း `TransferTranscript` တစ်ခုတည်းအတွင်း မြစ်ဝကျွန်းပေါ်ဒေသအားလုံးကို မှတ်တမ်းတင်ပါသည်။ မြစ်ဝကျွန်းပေါ်ဒေသတစ်ခုချင်းစီ၏ ကတိကဝတ်များ မရောက်မချင်း posidon အချေအတင်များကို အများအပြား-မြစ်ဝကျွန်းပေါ် စာသားမှတ်တမ်းများက ဖယ်ပေးသည်။ Kotodama တည်ဆောက်သူသည် ယခု အစ/အဆုံး syscalls များကို အလိုအလျောက် ထုတ်လွှတ်သောကြောင့် စာချုပ်များသည် လက်-ကုဒ်နံပါတ် Norito payloads များမပါဘဲ အစုလိုက်လွှဲပြောင်းမှုများကို အသုံးချနိုင်သည်။

## Row-count Regression Harness

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) သည် FASTPQ အကူးအပြောင်းအသုတ်များကို ချိန်ညှိနိုင်သော ရွေးချယ်နိုင်သော အရေအတွက်များဖြင့် ပေါင်းစပ်ပြီး ရလဒ် `row_usage` အနှစ်ချုပ်ကို အစီရင်ခံတင်ပြသည် (`total_rows`၊ တစ်ခုချင်းစီအလိုက် အချိုးများ) မှတ်တမ်းများနှင့်အတူ 65536 တန်း မျက်နှာကျက်အတွက် စံသတ်မှတ်ချက်များကို ဖမ်းယူပါ-

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```ထုတ်လွှတ်သော JSON သည် `iroha_cli audit witness` မှ ယခုထုတ်လွှတ်သော FASTPQ အတွဲအစည်းကို ပုံသေဖြင့်ထင်ဟပ်စေသည် (၎င်းတို့ကို ဖိနှိပ်ရန် `--no-fastpq-batches` ကို ကျော်ဖြတ်သည်) ထို့ကြောင့် `scripts/fastpq/check_row_usage.py` နှင့် CI ဂိတ်သည် ကြိုတင်လုပ်ဆောင်ထားသောလျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် ကိုက်ညီသောပြောင်းလဲမှုများကို ကွဲပြားစေနိုင်သည်။

# ဖြန့်ချိမှုအစီအစဉ်

1. **TF-1 (Transcript Plubbing)**- ✅ `StateTransaction::record_transfer_transcripts` သည် ယခုအခါ Norito မှတ်တမ်းများကို `TransferAsset`/batch၊ `sumeragi::witness::record_fastpq_transcript` တွင် ကမ္ဘာလုံးဆိုင်ရာ သက်သေအဖြစ် သိမ်းဆည်းထားပြီး `sumeragi::witness::record_fastpq_transcript` တွင် ၎င်းတို့ကို ကမ္ဘာလုံးဆိုင်ရာ သက်သေအဖြစ် သိမ်းဆည်းထားပြီး I100400181 အော်ပရေတာများအတွက် တိကျပြတ်သားသော `public_inputs` ပါသော `fastpq_batches` (ပါးလျလိုလျှင် `--no-fastpq-batches` ကိုသုံးပါ ထွက်လာသည်)။
2. **TF-2 (Gadget အကောင်အထည်ဖော်မှု)**- ✅ `gadgets::transfer` သည် ယခု delta စာသားမှတ်တမ်းများ (လက်ကျန်ဂဏန်းသင်္ချာ + Poseidon digest) ကို တရားဝင်အတည်ပြုပေးသည်)၊ တန်ဆာပလာများက ၎င်းတို့ကို ချန်လှပ်ထားသည့်အခါ SMT အထောက်အထားများကို ပေါင်းစပ်ပေါင်းစပ်ကာ ပေါင်းစပ်ဖွဲ့စည်းထားသောသက်သေများကို Norito မှတစ်ဆင့် ဖော်ထုတ်ပေးပါသည်။ သက်သေများမှ SMT ကော်လံများကိုဖြည့်နေစဉ် `Trace::transfer_witnesses` တွင် သက်သေများ။ `fastpq_row_bench` သည် 65536 အတန်းဆုတ်ယုတ်မှုကြိုးကို ဖမ်းယူထားသောကြောင့် အစီအစဉ်ရေးဆွဲသူများသည် Norito ကို ပြန်မကစားဘဲ အတန်းအသုံးပြုမှုကို ခြေရာခံသည် ဝန်ဆောင်ခများ။ 【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Batch helper)**- host-level sequential application နှင့် gadget loop အပါအဝင် batch syscall + Kotodama builder ကိုဖွင့်ပါ။
4. **TF-4 (Telemetry & docs)**- အပ်ဒိတ် `fastpq_plan.md`၊ `fastpq_migration_guide.md`၊ နှင့် ဒက်ရှ်ဘုတ် schemas များ

# မေးခွန်းများဖွင့်ပါ။

- **ဒိုမိန်းကန့်သတ်ချက်များ**- 2¹⁴ အတန်းထက်ကျော်လွန်သော ခြေရာများအတွက် လက်ရှိ FFT အစီအစဉ်ရေးဆွဲသူ အထိတ်တလန့်ဖြစ်ခြင်း။ TF-2 သည် ဒိုမိန်းအရွယ်အစားကို မြှင့်တင်သင့်သည် သို့မဟုတ် လျှော့ချထားသော စံသတ်မှတ်မှတ်ကို မှတ်တမ်းတင်သင့်သည်။
- **ပစ္စည်းမျိုးစုံ အစုလိုက်**- ကနဦး gadget သည် မြစ်ဝကျွန်းပေါ်ဒေသအလိုက် တူညီသော ပိုင်ဆိုင်မှု ID ကို ယူဆသည်။ အကယ်၍ ကျွန်ုပ်တို့သည် ကွဲပြားသောအစီအစဥ်များကို လိုအပ်ပါက၊ ပိုင်ဆိုင်မှုအပြန်အလှန်ပြန်ကစားခြင်းကို တားဆီးရန် အကြိမ်တိုင်းတွင် Poseidon သက်သေတွင် ပိုင်ဆိုင်မှုပါဝင်ကြောင်း သေချာရပါမည်။
- **Authority digest ပြန်လည်အသုံးပြုခြင်း**- syscall တစ်ခုလျှင် signer lists များကို ပြန်လည်တွက်ချက်ခြင်းမှ ရှောင်ကြဉ်ရန် အခြားခွင့်ပြုထားသော လုပ်ငန်းဆောင်တာများအတွက် တူညီသော digest ကို ရေရှည်အသုံးပြုနိုင်ပါသည်။


ဤစာတမ်းသည် ဒီဇိုင်းဆုံးဖြတ်ချက်များကို ခြေရာခံသည်။ မှတ်တိုင်များ ဆင်းသက်သည့်အခါ လမ်းပြမြေပုံနှင့် ကိုက်ညီအောင်ထားပါ။