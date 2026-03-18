---
lang: my
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Kotodama နမူနာများ ခြုံငုံသုံးသပ်ချက်

ဤစာမျက်နှာသည် Kotodama နမူနာများနှင့် IVM syscalls နှင့် pointer-ABI အငြင်းအခုံများကို မည်သို့မြေပုံဆွဲပြထားသည်။ ကိုလည်းကြည့်ပါ-
- လုပ်ဆောင်နိုင်သောရင်းမြစ်များအတွက် `examples/`
- canonical syscall ABI အတွက် `docs/source/ivm_syscalls.md`
- ဘာသာစကားသတ်မှတ်ချက်အပြည့်အစုံအတွက် `kotodama_grammar.md`

## မင်္ဂလာပါ + အကောင့်အသေးစိတ်

အရင်းအမြစ်- `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

မြေပုံဆွဲခြင်း (pointer-ABI)-
- `authority()` → `SCALL 0xA4` (အိမ်ရှင်က `&AccountId` ကို `r10` သို့ ရေးသည်)
- `set_account_detail(a, k, v)` → `r10=&AccountId`၊ `r11=&Name`၊ `r12=&Json`၊ ထို့နောက် `SCALL 0x1A`

## ပစ္စည်းလွှဲပြောင်းခြင်း။

အရင်းအမြစ်- `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
      account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```

မြေပုံဆွဲခြင်း (pointer-ABI)-
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`၊ ထို့နောက် `SCALL 0x24`

## NFT ဖန်တီး + လွှဲပြောင်း

အရင်းအမြစ်- `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let recipient = account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

မြေပုံဆွဲခြင်း (pointer-ABI)-
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Pointer Norito Helpers

Pointer-valued တာရှည်ခံသောအခြေအနေသည် ရိုက်ထည့်ထားသော TLV များကို နှင့် မှ ပြောင်းရန် လိုအပ်သည်။
ဆက်လက်တည်ရှိနေသော `NoritoBytes` စာအိတ်။ ယခု Kotodama သည် ဤအကူအညီများကို ကြိုးပေးသည်။
တည်ဆောက်သူများသည် pointer defaults နှင့် map ကိုသုံးနိုင်စေရန် compiler မှတဆင့်တိုက်ရိုက်
Manual FFI ကော်မပါဘဲ ရှာဖွေခြင်း-

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

နှိမ့်ချခြင်း-

- ရိုက်ထည့်ထားသော TLV ကိုထုတ်ဝေပြီးနောက် ညွှန်ပြသည့်ပုံသေများသည် `POINTER_TO_NORITO` ကို ထုတ်လွှတ်သည်၊ ထို့ကြောင့်၊
  လက်ခံသူသည် သိုလှောင်မှုအတွက် canonical `NoritoBytes` payload ကို လက်ခံရရှိသည် ။
- Reads သည် `POINTER_FROM_NORITO` ဖြင့် ပြောင်းပြန်လည်ပတ်မှုကို လုပ်ဆောင်ပြီး ၎င်းကိုထောက်ပံ့ပေးသည်။
  `r11` တွင် မျှော်လင့်ထားသော pointer type id
- လမ်းကြောင်းနှစ်ခုစလုံးသည် ပကတိ TLV များကို INPUT ဒေသသို့ အလိုအလျောက် ဖြန့်ချိပေးပါသည်။
  string literals နှင့် runtime pointers များကို ပွင့်လင်းမြင်သာစွာ ရောနှောရန် စာချုပ်များ။

၎င်းကို runtime regression အတွက် `crates/ivm/tests/kotodama_pointer_args.rs` ကိုကြည့်ပါ။
`MockWorldStateView` ကို အသွားအပြန် လေ့ကျင့်သည်။

## Deterministic Map Iteration (ဒီဇိုင်း)၊

တစ်ခုစီအတွက် အဆုံးအဖြတ်ပေးသည့်မြေပုံသည် ဘောင်တစ်ခု လိုအပ်သည်။ Multi-entry ထပ်ကာထပ်ကာသည် ပြည်နယ်မြေပုံတစ်ခု လိုအပ်ပါသည်။ compiler သည် `.take(n)` သို့မဟုတ် အများဆုံးကြေငြာထားသောအရှည်ကိုလက်ခံသည်။

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

ဝေါဟာရ-
- Iteration set သည် loop entry တွင် လျှပ်တစ်ပြက်ရိုက်ချက်တစ်ခုဖြစ်သည်။ အမှာစာသည် သော့၏ Norito bytes ဖြင့် အဘိဓာန်ဖြစ်သည်။
- `E_ITER_MUTATION` ဖြင့် loop trap ရှိ `M` သို့ ဖွဲ့စည်းပုံပြောင်းလဲမှုများ။
- ဘောင်မပါဘဲ compiler သည် `E_UNBOUNDED_ITERATION` ကို ထုတ်လွှတ်သည်။

## Compiler/Host အတွင်းပိုင်းများ (Rrust၊ Kotodama အရင်းအမြစ်မဟုတ်ပါ)

အောက်ပါအတိုအထွာများသည် toolchain ၏ Rust ဘက်တွင်နေထိုင်သည်။ ၎င်းတို့သည် compiler helpers နှင့် VM lowing mechanics များကို သရုပ်ဖော်ထားပြီး Kotodama `.ko` အရင်းအမြစ် ** မဟုတ်ပါ။

## Wide Opcode Chunked Frame အပ်ဒိတ်များ

Kotodama ၏ကျယ်ပြန့်သော opcode ကူညီပေးသူများသည် IVM မှအသုံးပြုသော 8-bit operand အပြင်အဆင်ကို ပစ်မှတ်ထားပါသည်
ကျယ်ပြန့်သောကုဒ်ပြောင်းခြင်း။ 128-ဘစ်တန်ဖိုးများကို ရွေ့လျားစေသည့် တတိယမြောက် အော်ပရေတာကို ပြန်လည်အသုံးပြုသည့် ဝန်နှင့်စတိုးဆိုင်များ
မြင့်မားသော register အတွက် slot ဖြစ်သောကြောင့် base register သည် final ကိုကိုင်ထားပြီးဖြစ်သည်။
လိပ်စာ။ ဝန်/စတိုးကို မထုတ်ပြန်မီ `ADDI` ဖြင့် အခြေခံကို ချိန်ညှိပါ-

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```အတုံးလိုက်ဘောင်မွမ်းမံမှုများသည် မှတ်ပုံတင်ခြင်းကိုသေချာစေရန် 16-byte အဆင့်များဖြင့် အခြေခံကို ရှေ့သို့တိုးစေသည်။
`STORE128` မှ ကျူးလွန်ထားသော အတွဲသည် လိုအပ်သော ချိန်ညှိမှု နယ်နိမိတ်တွင် ဆင်းသက်သည်။ အတူတူပါပဲ။
ပုံစံသည် `LOAD128` နှင့် သက်ဆိုင်သည်။ အလိုရှိသော ခြေလှမ်းဖြင့် `ADDI` ကို ထုတ်ပေးပါသည်။
load တစ်ခုစီသည် high destination register ကို တတိယ operand slot သို့ ချည်နှောင်ထားသည်။
VM နှင့် ကိုက်ညီသော `VMError::MisalignedAccess` နှင့် မှားယွင်းနေသော လိပ်စာများကို ထောင်ချောက်များ
`crates/ivm/tests/wide_memory128.rs` တွင်ကျင့်သုံးသောအပြုအမူ။

ဤ 128-bit အကူအညီများကို ထုတ်လွှတ်သော ပရိုဂရမ်များသည် vector စွမ်းရည်ကို ကြော်ငြာရပါမည်။
Kotodama compiler သည် `VECTOR` mode ကို အချိန်တိုင်း အလိုအလျောက် ဖွင့်ပေးသည်
`LOAD128`/`STORE128` ပေါ်လာသည်၊ VM သည် ထောင်ချောက်များဖြင့် ဖမ်းသည်။
ပရိုဂရမ်တစ်ခုက ၎င်းတို့ကို လုပ်ဆောင်ရန် ကြိုးပမ်းပါက `VMError::VectorExtensionDisabled`
ဒီနည်းနည်းသတ်မှတ်မထားဘူး။

## ကျယ်ပြန့်သော အခြေအနေတွင်ရှိသော ဘဏ်ခွဲများကို လျှော့ချခြင်း။

Kotodama သည် `if`/`else` သို့မဟုတ် ternary အကိုင်းအခက်ကို ကျယ်ပြန့် bytecode အဖြစ် လျှော့ချသောအခါ၊
ပုံသေ `BNE cond, zero, +2` အတွဲလိုက်ပြီးနောက် `JAL` ညွှန်ကြားချက်တစ်စုံဖြင့်

1. တိုတောင်းသော `BNE` သည် အခြေအနေဆိုင်ရာဌာနခွဲအား 8-bit ချက်ခြင်းလမ်းကြောအတွင်း သိမ်းဆည်းသည်
   `JAL` ဖြတ်ကျော်ပြီး ခုန်ချလိုက်ပါ။
2. ပထမ `JAL` သည် `else` ဘလောက်ကို ပစ်မှတ်ထားပါသည် (အခြေအနေဖြစ်သည့်အခါ လုပ်ဆောင်သည်
   အမှား)။
3. ဒုတိယ `JAL` သည် `then` ဘလောက်သို့ ခုန်သွားသည် (အခြေအနေဖြစ်သည့်အခါ ယူသည်
   အမှန်)။

ဤပုံစံသည် ပိုကြီးသော အော့ဖ်ဆက်များကို ကုဒ်ကုဒ်လုပ်ရန် မည်သည့်အခါမျှ အခြေအနေစစ်ဆေးမှုကို အာမခံပါသည်။
`then` အတွက် မထင်သလို ကြီးမားသော ကိုယ်ထည်များကို ပံ့ပိုးနေချိန်တွင် ±127 စကားလုံးများထက်
ကျယ်ပြန့်သော `JAL` အကူအညီပေးခြင်းဖြင့် `else` လုပ်ကွက်များ။ ကြည့်ပါ။
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` တို့အတွက်
sequence ကိုပိတ်သော regression test ။

### ဥပမာ နှိမ့်ချခြင်း။

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

အောက်ဖော်ပြပါ ကျယ်ပြန့်သော ညွှန်ကြားချက်အရိုးစုသို့ စုစည်းပါ (နံပါတ်များနှင့် မှတ်ပုံတင်ပါ။
အကြွင်းမဲ့ အော့ဖ်ဆက်များသည် enclosing လုပ်ဆောင်မှုအပေါ် မူတည်သည်)။

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

နောက်ဆက်တွဲ ညွှန်ကြားချက်များသည် ကိန်းသေများကို အကောင်အထည်ပေါ်စေပြီး ပြန်တန်ဖိုးကို ရေးပါ။
`BNE` သည် ပထမ `JAL` ထက်ခုန်တက်သောကြောင့်၊ အခြေအနေအရ offset သည် အမြဲတမ်းဖြစ်သည်
`+2` စကားလုံးများ၊ ဘလောက်ကောင်များ ချဲ့ထွင်နေချိန်၌ပင် အကိုင်းအခက်ကို အကွာအဝေးအတွင်း ထားရှိပါ။